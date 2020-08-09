use std::process::{Child, Command, ExitStatus, Stdio};
use std::io::{Error, ErrorKind};
use std::collections::{HashSet, HashMap};
use std::cell::RefCell;
use crate::{CmdResult, FunResult};

pub struct GroupCmds {
     cmds: Vec<(PipedCmds, Option<PipedCmds>)>,  // (cmd, orCmd) pairs
     cmds_env: Env,
}

impl GroupCmds {
    pub fn new() -> Self {
        Self {
            cmds: vec![],
            cmds_env: Env::new(),
        }
    }

    pub fn add(&mut self, cmds: PipedCmds, or_cmds: Option<PipedCmds>) -> &mut Self {
        self.cmds.push((cmds, or_cmds));
        self
    }

    pub fn run_cmd(&mut self) -> CmdResult {
        for cmd in self.cmds.iter_mut() {
            if let Err(err) = cmd.0.run_cmd(&mut self.cmds_env) {
                if let Some(or_cmds) = &mut cmd.1 {
                    or_cmds.run_cmd(&mut self.cmds_env)?;
                } else {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub fn run_fun(&mut self) -> FunResult {
        let mut ret = String::new();
        for cmd in self.cmds.iter_mut() {
            let ret0 = cmd.0.run_fun(&mut self.cmds_env);
            match ret0 {
                Err(e) => {
                    if let Some(or_cmds) = &mut cmd.1 {
                        ret = or_cmds.run_fun(&mut self.cmds_env)?;
                    } else {
                        return Err(e);
                    }
                },
                Ok(r) => ret = r,
            };
        }
        Ok(ret)
    }
}

pub struct BuiltinCmds {
    cmds: Vec<String>,
}

impl BuiltinCmds {
    pub fn from(cmds: &Vec<String>) -> Self {
        Self {
            cmds: cmds.to_vec(),
        }
    }

    pub fn is_builtin(cmd: &str) -> bool {
        let mut builtins = HashSet::new();
        builtins.insert("cd");
        builtins.insert("true");
        builtins.contains(cmd)
    }

    pub fn run_cmd(&mut self, cmds_env: &mut Env) -> CmdResult {
        match self.cmds[0].as_str() {
            "true" => self.run_true_cmd(cmds_env),
            "cd" => self.run_cd_cmd(cmds_env),
            _ => panic!("invalid builtin cmd: {}", self.cmds[0]),
        }
    }

    fn run_true_cmd(&mut self, _cmds_env: &mut Env) -> CmdResult {
        if self.cmds.len() != 1 {
            let err = Error::new(
                ErrorKind::Other,
                format!("true: too many arguments: {}", self.cmds.join(" ")),
            );
            return Err(err);
        }
        Ok(())
    }

    fn run_cd_cmd(&mut self, cmds_env: &mut Env) -> CmdResult {
        let mut dir = self.cmds[1].clone();
        if self.cmds.len() != 2 {
            let err = Error::new(
                ErrorKind::Other,
                format!("cd: too many arguments: {}", self.cmds.join(" ")),
            );
            return Err(err);
        }
        // if it is relative path, always convert it to absolute one
        if !dir.starts_with("/") {
            ENV_VARS.with(|vars| {
                if let Some(cmd_lib_pwd) = vars.borrow().get("PWD") {
                    dir = format!("{}/{}", cmd_lib_pwd, dir);
                } else {
                    dir = format!("{}/{}", std::env::current_dir().unwrap().to_str().unwrap(), dir);
                }
            });
        }
        if !std::path::Path::new(&dir).exists() {
            let err_msg = format!("cd: {}: No such file or directory", dir);
            eprintln!("{}", err_msg);
            let err = Error::new(
                ErrorKind::Other,
                err_msg,
            );
            return Err(err);
        }
        cmds_env.set_var("PWD".to_string(), dir);
        Ok(())
    }
}


pub struct PipedCmds {
    pipes: Vec<Command>,
    children: Vec<Child>,

    cmd_args: Vec<Vec<String>>,
    full_cmd: String,
}

impl PipedCmds {
    pub fn new() -> Self {
        Self {
            pipes: vec![],
            children: vec![],
            cmd_args: vec![],
            full_cmd: String::new(),
        }
    }

    pub fn from(start_cmd_argv: Vec<String>) -> Self {
        let mut start_cmd = Command::new(&start_cmd_argv[0]);
        start_cmd.args(&start_cmd_argv[1..]);
        Self {
            pipes: vec![start_cmd],
            children: vec![],
            full_cmd: start_cmd_argv.join(" ").to_string(),
            cmd_args: vec![start_cmd_argv],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pipes.is_empty()
    }

    pub fn pipe(&mut self, pipe_cmd_argv: Vec<String>) -> &mut Self {
        if !self.pipes.is_empty() {
            let last_i = self.pipes.len() - 1;
            self.pipes[last_i].stdout(Stdio::piped());
        }

        let mut pipe_cmd = Command::new(&pipe_cmd_argv[0]);
        pipe_cmd.args(&pipe_cmd_argv[1..]);
        self.pipes.push(pipe_cmd);

        if !self.full_cmd.is_empty() {
            self.full_cmd += " | ";
        }
        self.full_cmd += &pipe_cmd_argv.join(" ");
        self.cmd_args.push(pipe_cmd_argv);
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

    pub fn run_cmd(&mut self, cmds_env: &mut Env) -> CmdResult {
        // check builtin commands
        if BuiltinCmds::is_builtin(&self.cmd_args[0][0]) {
            return BuiltinCmds::from(&self.cmd_args[0]).run_cmd(cmds_env);
        }

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

    pub fn run_fun(&mut self, _cmds_env: &mut Env) -> FunResult {
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
        let mut cmds_env = Env::new();
        assert!(PipedCmds::from(vec!["echo".to_string(), "rust".to_string()])
                .pipe(vec!["wc".to_string()])
                .run_cmd(&mut cmds_env)
                .is_ok());
    }

    #[test]
    fn test_run_piped_funs() {
        let mut cmds_env = Env::new();
        assert_eq!(PipedCmds::from(vec!["echo".to_string(), "rust".to_string()])
                   .run_fun(&mut cmds_env)
                   .unwrap(),
                   "rust");

        assert_eq!(PipedCmds::from(vec!["echo".to_string(), "rust".to_string()])
                   .pipe(vec!["wc".to_string(), "-c".to_string()])
                   .run_fun(&mut cmds_env)
                   .unwrap()
                   .trim(), "5");
    }
}
