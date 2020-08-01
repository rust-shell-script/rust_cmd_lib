use std::process::{Child, Command, ExitStatus, Stdio};
use std::io::{Error, ErrorKind, Result};
use std::collections::HashMap;
use std::cell::RefCell;
use crate::{CmdResult, FunResult};

//
// Low level process API, wrapper on std::process module
//
pub struct Process {
    full_cmd: Vec<Vec<String>>,
}

impl Process {
    pub fn new(start_cmd: Vec<String>) -> Self {
        Self {
            full_cmd: vec![start_cmd],
        }
    }

    pub fn pipe(&mut self, pipe_cmd: Vec<String>) -> &mut Self {
        self.full_cmd.push(pipe_cmd);
        self
    }

    pub fn wait<T: ProcessResult>(&mut self) -> T {
        T::get_result(self)
    }
}

#[doc(hidden)]
pub trait ProcessResult {
    fn get_result(process: &mut Process) -> Self;
}

impl ProcessResult for FunResult {
    fn get_result(process: &mut Process) -> Self {
        let (last_proc, full_cmd_str) = run_full_cmd(process, true)?;
        let output = last_proc.wait_with_output()?;
        if !output.status.success() {
            Err(to_io_error(&full_cmd_str, output.status))
        } else {
            let mut ans = String::from_utf8_lossy(&output.stdout).to_string();
            if ans.ends_with('\n') {
                ans.pop();
            }
            Ok(ans)
        }
    }
}

impl ProcessResult for CmdResult {
    fn get_result(process: &mut Process) -> Self {
        let (mut last_proc, full_cmd_str) = run_full_cmd(process, false)?;
        let status = last_proc.wait()?;
        if !status.success() {
            Err(to_io_error(&full_cmd_str, status))
        } else {
            Ok(())
        }
    }
}

fn to_io_error(command: &str, status: ExitStatus) -> Error {
    if let Some(code) = status.code() {
        Error::new(ErrorKind::Other, format!("{} exit with {}", command, code))
    } else {
        Error::new(ErrorKind::Other, "Unknown error")
    }
}

fn format_full_cmd(full_cmd: &Vec<Vec<String>>) -> String {
    let mut full_cmd_str = String::from(full_cmd[0].join(" "));
    for cmd in full_cmd.iter().skip(1) {
        full_cmd_str += " | ";
        full_cmd_str += &cmd.join(" ");
    }
    full_cmd_str
}

fn run_full_cmd(process: &mut Process, pipe_last: bool) -> Result<(Child, String)> {
    let mut full_cmd_str = format_full_cmd(&process.full_cmd);
    let first_cmd = &process.full_cmd[0];
    let mut cmd = Command::new(&first_cmd[0]);

    ENV_VARS.with(|vars| {
        if let Some(dir) = vars.borrow().get("PWD") {
            full_cmd_str += &format!(" (cd: {})", dir);
            cmd.current_dir(dir);
        }
        let mut debug = String::from("0");
        if let Some(proc_debug) = vars.borrow().get("CMD_LIB_DEBUG") {
            debug = proc_debug.clone();
        } else if let Ok(global_debug) = std::env::var("CMD_LIB_DEBUG") {
            debug = global_debug.clone();
        }
        if debug == "1" {
            eprintln!("Running \"{}\" ...", full_cmd_str);
        }
    });

    let mut last_proc = cmd
        .args(&first_cmd[1..])
        .stdout(if pipe_last || process.full_cmd.len() > 1 {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .spawn()?;
    for (i, cmd) in process.full_cmd.iter().skip(1).enumerate() {
        let new_proc = Command::new(&cmd[0])
            .args(&cmd[1..])
            .stdin(last_proc.stdout.take().unwrap())
            .stdout(if !pipe_last && i == process.full_cmd.len() - 2 {
                Stdio::inherit()
            } else {
                Stdio::piped()
            })
            .spawn()?;
        last_proc.wait().unwrap();
        last_proc = new_proc;
    }

    Ok((last_proc, full_cmd_str))
}

thread_local!{
    pub static ENV_VARS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}
#[doc(hidden)]
pub struct Env {
    vars_saved: HashMap<String, String>,
}

impl Env {
    pub fn new() -> Self {
        Self {
            vars_saved: HashMap::new(),
        }
    }

    pub fn set_var(&mut self, key: String, value: String) {
        ENV_VARS.with(|vars| {
            if let Some(old_value) = vars.borrow().get(&key) {
                self.vars_saved.insert(key.clone(), old_value.to_owned());
            } else {
                self.vars_saved.insert(key.clone(), "".to_owned());
            }
            vars.borrow_mut().insert(key, value);
        });
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        for (key, value) in &self.vars_saved {
            if value != "" {
                ENV_VARS.with(|vars| {
                    vars.borrow_mut().insert(key.to_owned(), value.to_owned());
                });
            } else {
                ENV_VARS.with(|vars| {
                    vars.borrow_mut().remove(key);
                });
            }
        }
    }
}

#[macro_export]
macro_rules! proc_env_set {
    () => {};
    (&$env: expr) => {};
    (&$env: expr, $key:ident = $v:tt $($other:tt)*) => {
        $env.set_var(stringify!($key).to_string(), $v.to_string());
        proc_env_set!(&$env $($other)*);
    };
    ($key:ident = $v:tt $($other:tt)*) => {
        let mut _cmdlib_env = $crate::Env::new();
        _cmdlib_env.set_var(stringify!($key).to_string(), $v.to_string());
        proc_env_set!(&_cmdlib_env $($other)*);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_pwd_set() {
        {
            proc_env_set!(PWD = "/tmp", DEBUG = 1);
            ENV_VARS.with(|vars| {
                assert!(vars.borrow().get("PWD") == Some(&"/tmp".to_string()));
                assert!(vars.borrow().get("DEBUG") == Some(&"1".to_string()));
            });
        }
        ENV_VARS.with(|vars| {
            assert!(vars.borrow().get("PWD").is_none());
        });
    }
}
