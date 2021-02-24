use std::process::{Child, Command, ExitStatus, Stdio};
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::{Error, ErrorKind};
use std::fs::{File, OpenOptions};
use std::collections::HashMap;
use std::sync::Mutex;
use std::env;
use lazy_static::lazy_static;
use crate::proc_env::Env;
use crate::proc_env::ENV_VARS;
use crate::{CmdResult, FunResult};

pub type CmdArgs = Vec<String>;
type FnFun = fn(CmdArgs) -> FunResult;

fn cd_cmd(args: CmdArgs) -> FunResult {
    if args.len() == 1 {
        return Err(Error::new(ErrorKind::Other, "cd: missing directory"));
    } else if args.len() > 2 {
        let err_msg = format!("cd: too many arguments: {}", args.join(" "));
        return Err(Error::new(ErrorKind::Other, err_msg));
    }

    env::set_current_dir(&args[1])?;
    Ok("".into())
}

fn echo_cmd(args: CmdArgs) -> FunResult {
    Ok(args[1..].join(" "))
}

fn true_cmd(_args: CmdArgs) -> FunResult {
    Ok("".into())
}

lazy_static! {
    static ref CMD_MAP: Mutex<HashMap<&'static str, FnFun>> = {
        // needs explicit type, or it won't compile
        let mut m: HashMap<&'static str, FnFun> = HashMap::new();
        m.insert("cd", cd_cmd);
        m.insert("echo", echo_cmd);
        m.insert("true", true_cmd);
        Mutex::new(m)
    };
}

#[doc(hidden)]
pub fn export_cmd(cmd: &'static str, func: FnFun) {
    CMD_MAP.lock().unwrap().insert(cmd, func);
}

pub fn set_debug(enable: bool) {
    env::set_var("CMD_LIB_DEBUG", if enable { "1" } else { "0" });
}

fn to_cmd_result(res: FunResult) -> CmdResult {
    match res {
        Ok(v) => {
            print!("{}{}", v, if v.is_empty() {""} else {"\n"});
            Ok(())
        },
        Err(e) => Err(e)
    }
}

pub struct GroupCmds {
     cmds: Vec<(Cmds, Option<Cmds>)>,  // (cmd, orCmd) pairs
     cmds_env: Env,
}

impl GroupCmds {
    pub fn new() -> Self {
        Self {
            cmds: vec![],
            cmds_env: Env::new(),
        }
    }

    pub fn add(&mut self, cmds: Cmds, or_cmds: Option<Cmds>) -> &mut Self {
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

pub struct Cmds {
    pipes: Vec<Command>,
    children: Vec<Child>,

    cmd_args: Vec<Cmd>,
    full_cmd: String,
}

impl Cmds {
    pub fn new() -> Self {
        Self {
            pipes: vec![],
            children: vec![],
            cmd_args: vec![],
            full_cmd: String::new(),
        }
    }

    pub fn from_cmd(mut cmd: Cmd) -> Self {
        let cmd_args: Vec<String> = cmd.get_args().to_vec();
         Self {
            pipes: vec![cmd.gen_command()],
            children: vec![],
            full_cmd: cmd_args.join(" ").to_string(),
            cmd_args: vec![cmd],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pipes.is_empty()
    }

    pub fn pipe(&mut self, mut cmd: Cmd) -> &mut Self {
        if !self.pipes.is_empty() {
            let last_i = self.pipes.len() - 1;
            self.pipes[last_i].stdout(Stdio::piped());
        }

        let cmd_args: Vec<String> = cmd.get_args().to_vec();
        let pipe_cmd = cmd.gen_command();
        self.pipes.push(pipe_cmd);

        if !self.full_cmd.is_empty() {
            self.full_cmd += " | ";
        }
        self.full_cmd += &cmd_args.join(" ");
        self.cmd_args.push(cmd);
        self
    }

    fn spawn(&mut self) -> CmdResult {
        let mut pipe_error = false;

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

            if let Some("1") = vars.borrow().get("CMD_LIB_PIPE_FAIL").map(|v| v as &str) {
                pipe_error = true;
            } else if let Ok("1") = std::env::var("CMD_LIB_PIPE_FAIL").as_ref().map(|v| v as &str) {
                pipe_error = true;
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
            if pipe_error {
                let status = self.children[i].wait()?;
                if !status.success() {
                    return Err(Self::to_io_error(&format!("{:?}", cmd), status))
                }
            }
        }

        Ok(())
    }

    pub fn run_cmd(&mut self, _cmds_env: &mut Env) -> CmdResult {
        // check builtin commands
        let args = self.cmd_args[0].get_args().clone();
        let cmd = &args[0].as_str();
        let is_builtin = CMD_MAP.lock().unwrap().contains_key(cmd);
        if is_builtin {
            return to_cmd_result(CMD_MAP.lock().unwrap()[cmd](args));
        }

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

pub enum FdOrFile {
    Fd(i32, bool),          // fd, append?
    File(String, bool),     // file, append?
    OpenedFile(File, bool), // opened file, append?
}
impl FdOrFile {
    pub fn is_orig_stdout(&self) -> bool {
        if let FdOrFile::Fd(1, _) = self {
            true
        } else {
            false
        }
    }
}

pub struct Cmd {
    args: Vec<String>,
    redirects: Vec<(i32, FdOrFile)>,
}

impl Cmd {
    pub fn new() -> Self {
        Self {
            args: vec![],
            redirects: vec![],
        }
    }

    pub fn from_args<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            args: args.into_iter()
                .map(|s| s.as_ref().to_owned())
                .collect(),
            redirects: vec![],
        }
    }

    pub fn add_arg(&mut self, arg: String) -> &mut Self {
        self.args.push(arg);
        self
    }

    pub fn get_args(&mut self) -> &mut Vec<String> {
        &mut self.args
    }

    pub fn set_redirect(&mut self, fd: i32, target: FdOrFile) -> &mut Self {
        self.redirects.push((fd, target));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    pub fn gen_command(&mut self) -> Command {
        let cmd_args: Vec<String> = self.get_args().to_vec();
        let mut cmd = Command::new(&cmd_args[0]);
        cmd.args(&cmd_args[1..]);

        for (fd_src, target) in self.redirects.iter_mut() {
            match &target {
                FdOrFile::Fd(fd, _append) => {
                    let out = unsafe {Stdio::from_raw_fd(*fd)};
                    match *fd_src {
                        1 => cmd.stdout(out),
                        2 => cmd.stderr(out),
                        _ => panic!("invalid fd: {}", *fd_src),
                    };
                },
                FdOrFile::File(file, append) => {
                    if file == "/dev/null" {
                        match *fd_src {
                            0 => cmd.stdin(Stdio::null()),
                            1 => cmd.stdout(Stdio::null()),
                            2 => cmd.stderr(Stdio::null()),
                            _ => panic!("invalid fd: {}", *fd_src),
                        };
                    } else {
                        let f = if *fd_src == 0 {
                            OpenOptions::new()
                                .read(true)
                                .open(file)
                                .unwrap()
                        } else {
                            OpenOptions::new()
                                .create(true)
                                .truncate(!append)
                                .write(true)
                                .append(*append)
                                .open(file)
                                .unwrap()
                        };
                        let fd = f.as_raw_fd();
                        let out = unsafe {Stdio::from_raw_fd(fd)};
                        match *fd_src {
                            0 => cmd.stdin(out),
                            1 => cmd.stdout(out),
                            2 => cmd.stderr(out),
                            _ => panic!("invalid fd: {}", *fd_src),
                        };
                        *target = FdOrFile::OpenedFile(f, *append);
                    }
                },
                _ => {
                    panic!("file is already opened");
                }
            };
        }

        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_piped_cmds() {
        assert!(Cmds::from_cmd(Cmd::from_args(vec!["echo", "rust"]))
                .pipe(Cmd::from_args(vec!["wc"]))
                .run_cmd(&mut Env::new())
                .is_ok());
    }

    #[test]
    fn test_run_piped_funs() {
        assert_eq!(Cmds::from_cmd(Cmd::from_args(vec!["echo", "rust"]))
                   .run_fun(&mut Env::new())
                   .unwrap(),
                   "rust");

        assert_eq!(Cmds::from_cmd(Cmd::from_args(vec!["echo", "rust"]))
                   .pipe(Cmd::from_args(vec!["wc", "-c"]))
                   .run_fun(&mut Env::new())
                   .unwrap()
                   .trim(), "5");
    }

    #[test]
    fn test_stdout_redirect() {
        let tmp_file = "/tmp/file_echo_rust";
        let mut write_cmd = Cmd::from_args(vec!["echo", "rust"]);
        write_cmd.set_redirect(1, FdOrFile::File(tmp_file.to_string(), false));
        assert!(Cmds::from_cmd(write_cmd)
                .run_cmd(&mut Env::new())
                .is_ok());

        let read_cmd = Cmd::from_args(vec!["cat", tmp_file]);
        assert_eq!(Cmds::from_cmd(read_cmd)
                   .run_fun(&mut Env::new())
                   .unwrap(),
                   "rust");

        let cleanup_cmd = Cmd::from_args(vec!["rm", tmp_file]);
        assert!(Cmds::from_cmd(cleanup_cmd)
                .run_cmd(&mut Env::new())
                .is_ok());
    }
}
