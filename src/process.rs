use std::process::{Child, Command, ExitStatus, Stdio};
use std::io::{Error, ErrorKind};
use std::collections::HashMap;
use std::cell::RefCell;
use crate::{CmdResult, FunResult};

#[allow(dead_code)]
pub struct GroupCmds {
     cmds: Vec<(PipedCmds, Option<PipedCmds>)>,  // (cmd, orCmd) pairs
}

#[allow(dead_code)]
impl GroupCmds {
    pub fn new() -> Self {
        Self {
            cmds: vec![],
        }
    }

    pub fn add(&mut self, cmds: PipedCmds, or_cmds: Option<PipedCmds>) -> &mut Self {
        self.cmds.push((cmds, or_cmds));
        self
    }

    pub fn run_cmd(&mut self) -> CmdResult {
        Ok(())
    }

    pub fn run_fun(self) -> FunResult {
        Ok("ok".to_string())
    }
}

pub struct PipedCmds {
    pipes: Vec<Command>,
    children: Vec<Child>,
    full_cmd: String,
}

impl PipedCmds {
    pub fn new(start_cmd_argv: &Vec<String>) -> Self {
        let mut start_cmd = Command::new(&start_cmd_argv[0]);
        start_cmd.args(&start_cmd_argv[1..]);
        Self {
            pipes: vec![start_cmd],
            children: vec![],
            full_cmd: start_cmd_argv.join(" ").to_string(),
        }
    }

    pub fn pipe(&mut self, pipe_cmd_argv: &Vec<String>) -> &mut Self {
        let last_i = self.pipes.len() - 1;
        self.pipes[last_i].stdout(Stdio::piped());

        let mut pipe_cmd = Command::new(&pipe_cmd_argv[0]);
        pipe_cmd.args(&pipe_cmd_argv[1..]);
        self.pipes.push(pipe_cmd);

        self.full_cmd += " | ";
        self.full_cmd += &pipe_cmd_argv.join(" ");
        self
    }

    fn spawn(&mut self) -> CmdResult {
        ENV_VARS.with(|vars| {
            if let Some(dir) = vars.borrow().get("PWD") {
                self.full_cmd += &format!(" (cd: {})", dir);
                self.pipes[0].current_dir(dir);
            }
            let mut debug = String::from("0");
            if let Some(proc_debug) = vars.borrow().get("CMD_LIB_DEBUG") {
                debug = proc_debug.clone();
            } else if let Ok(global_debug) = std::env::var("CMD_LIB_DEBUG") {
                debug = global_debug.clone();
            }
            if debug == "1" {
                eprintln!("Running \"{}\" ...", self.full_cmd);
            }
        });

        for (i, cmd) in self.pipes.iter_mut().enumerate() {
            if i != 0 {
                cmd.stdin(self.children[i - 1].stdout.take().unwrap());
            }
            self.children.push(cmd.spawn()?);
            if i % 2 != 0 {
                self.children[i - 1].wait()?;
            }
        }

        Ok(())
    }

    pub fn run_cmd(&mut self) -> CmdResult {
        let last_i = self.pipes.len() - 1;
        self.pipes[last_i].stdout(Stdio::inherit());

        self.spawn()?;
        let status = self.children.pop().unwrap().wait()?;
        if !status.success() {
            Err(Self::to_io_error(&self.full_cmd, status))
        } else {
            Ok(())
        }
    }

    pub fn run_fun(&mut self) -> FunResult {
        let last_i = self.pipes.len() - 1;
        self.pipes[last_i].stdout(Stdio::piped());

        self.spawn()?;
        let output = self.children.pop().unwrap().wait_with_output()?;
        if !output.status.success() {
            Err(Self::to_io_error(&self.full_cmd, output.status))
        } else {
            let mut ret = String::from_utf8_lossy(&output.stdout).to_string();
            if ret.ends_with('\n') {
                ret.pop();
            }
            Ok(ret)
        }
    }

    fn to_io_error(command: &str, status: ExitStatus) -> Error {
        if let Some(code) = status.code() {
            Error::new(ErrorKind::Other, format!("{} exit with {}", command, code))
        } else {
            Error::new(ErrorKind::Other, "Unknown error")
        }
   }
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

    #[test]
    fn test_run_piped_cmds() {
        assert!(PipedCmds::new(&vec!["echo".to_string(), "rust".to_string()])
                .pipe(&vec!["wc".to_string()])
                .run_cmd()
                .is_ok());
    }

    #[test]
    fn test_run_piped_funs() {
        assert_eq!(PipedCmds::new(&vec!["echo".to_string(), "rust".to_string()])
                   .run_fun()
                   .unwrap(),
                   "rust");

        assert_eq!(PipedCmds::new(&vec!["echo".to_string(), "rust".to_string()])
                   .pipe(&vec!["wc".to_string(), "-c".to_string()])
                   .run_fun()
                   .unwrap()
                   .trim(), "5");
    }
}
