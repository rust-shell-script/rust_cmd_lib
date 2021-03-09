use crate::{tls_get, tls_init, tls_set, CmdResult, FunResult};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Error, ErrorKind, Write};
use std::process::{Child, Command, ExitStatus, Stdio};

pub type CmdArgs = Vec<String>;
pub type CmdEnvs = HashMap<String, String>;
type FnFun = fn(CmdArgs, CmdEnvs) -> FunResult;

tls_init!(CMD_MAP, HashMap<&'static str, FnFun>, HashMap::new());

#[doc(hidden)]
pub fn export_cmd(cmd: &'static str, func: FnFun) {
    tls_set!(CMD_MAP, |map| map.insert(cmd, func));
}

/// set debug mode or not, false by default
pub fn set_debug(enable: bool) {
    std::env::set_var("CMD_LIB_DEBUG", if enable { "1" } else { "0" });
}

/// set pipefail or not, true by default
pub fn set_pipefail(enable: bool) {
    std::env::set_var("CMD_LIB_PIPEFAIL", if enable { "1" } else { "0" });
}

#[doc(hidden)]
#[derive(Default)]
pub struct GroupCmds {
    cmds: Vec<(Cmds, Option<Cmds>)>, // (cmd, orCmd) pairs
}

impl GroupCmds {
    pub fn add(mut self, cmds: Cmds, or_cmds: Option<Cmds>) -> Self {
        self.cmds.push((cmds, or_cmds));
        self
    }

    pub fn run_cmd(self) -> CmdResult {
        for cmd in self.cmds.into_iter() {
            if let Err(err) = cmd.0.run_cmd() {
                if let Some(or_cmds) = cmd.1 {
                    or_cmds.run_cmd()?;
                } else {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub fn run_fun(self) -> FunResult {
        let mut ret = String::new();
        for cmd in self.cmds.into_iter() {
            let ret0 = cmd.0.run_fun();
            match ret0 {
                Err(e) => {
                    if let Some(or_cmds) = cmd.1 {
                        ret = or_cmds.run_fun()?;
                    } else {
                        return Err(e);
                    }
                }
                Ok(r) => ret = r,
            };
        }
        Ok(ret)
    }

    pub fn spawn(mut self) -> std::io::Result<WaitCmd> {
        assert_eq!(self.cmds.len(), 1);
        self.cmds.pop().unwrap().0.spawn()
    }

    pub fn spawn_with_output(mut self) -> std::io::Result<WaitFun> {
        assert_eq!(self.cmds.len(), 1);
        self.cmds.pop().unwrap().0.spawn_with_output()
    }
}

enum ProcHandle {
    ProcChild(Child),
    ProcStr(Option<String>),
}

pub struct WaitCmd(Vec<ProcHandle>, Vec<String>);
impl WaitCmd {
    pub fn wait_result(&mut self) -> CmdResult {
        // wait last process result
        let (handle, cmd) = (self.0.pop().unwrap(), self.1.pop().unwrap());
        match handle {
            ProcHandle::ProcChild(mut child) => {
                let status = child.wait()?;
                if !status.success() {
                    return Err(Cmds::to_io_error(
                        &format!("{} exited with error", cmd),
                        status,
                    ));
                }
            }
            ProcHandle::ProcStr(mut ss) => {
                if let Some(s) = ss.take() {
                    print!("{}", s);
                }
            }
        }

        // wait previous processes
        while !self.0.is_empty() {
            let (handle, cmd) = (self.0.pop().unwrap(), self.1.pop().unwrap());
            Cmds::wait_child(handle, &cmd)?;
        }
        Ok(())
    }
}

pub struct WaitFun(Vec<ProcHandle>, Vec<String>);
impl WaitFun {
    pub fn wait_result(&mut self) -> FunResult {
        let mut ret = String::new();
        // wait last process result
        let (handle, cmd) = (self.0.pop().unwrap(), self.1.pop().unwrap());
        match handle {
            ProcHandle::ProcChild(child) => {
                let output = child.wait_with_output()?;
                if !output.status.success() {
                    return Err(Cmds::to_io_error(
                        &format!("{} exited with error", cmd),
                        output.status,
                    ));
                } else {
                    ret = String::from_utf8_lossy(&output.stdout).to_string();
                }
            }
            ProcHandle::ProcStr(mut ss) => {
                if let Some(s) = ss.take() {
                    ret = s;
                }
            }
        }

        // wait previous processes
        while !self.0.is_empty() {
            let (handle, cmd) = (self.0.pop().unwrap(), self.1.pop().unwrap());
            Cmds::wait_child(handle, &cmd)?;
        }

        if ret.ends_with('\n') {
            ret.pop();
        }
        Ok(ret)
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct Cmds {
    pipes: Vec<Command>,
    children: Vec<ProcHandle>,

    cmd_args: Vec<Cmd>,
    current_dir: String,
}

impl Cmds {
    pub fn pipe(mut self, mut cmd: Cmd) -> Self {
        if !self.pipes.is_empty() {
            self.pipes.last_mut().unwrap().stdout(Stdio::piped());
        }

        let mut pipe_cmd = cmd.gen_command();
        for (k, v) in cmd.get_envs() {
            pipe_cmd.env(k, v);
        }
        if !self.current_dir.is_empty() {
            pipe_cmd.current_dir(self.current_dir.clone());
        }
        self.pipes.push(pipe_cmd);
        self.cmd_args.push(cmd);
        self
    }

    fn get_full_cmd(&self) -> String {
        let mut ret = String::new();
        for cmd_arg in self.cmd_args.iter() {
            if !ret.is_empty() {
                ret += " | ";
            }
            ret += &cmd_arg.get_args().join(" ");
        }
        ret
    }

    fn spawn(mut self) -> std::io::Result<WaitCmd> {
        if let Ok(debug) = std::env::var("CMD_LIB_DEBUG") {
            if debug == "1" {
                eprintln!("Running \"{}\" ...", self.get_full_cmd());
            }
        }

        // spawning all the sub-processes
        for (i, cmd) in self.pipes.iter_mut().enumerate() {
            if i != 0 {
                let mut stdin_setup_done = false;
                if let ProcHandle::ProcChild(child) = &mut self.children[i - 1] {
                    if let Some(output) = child.stdout.take() {
                        cmd.stdin(output);
                        stdin_setup_done = true;
                    }
                }
                if !stdin_setup_done {
                    cmd.stdin(Stdio::piped());
                }
            }

            // check commands defined in CMD_MAP
            let args = self.cmd_args[i].get_args().clone();
            let envs = self.cmd_args[i].get_envs().clone();
            let command = &args[0].as_str();
            let in_cmd_map = tls_get!(CMD_MAP).contains_key(command);
            if command == &"cd" {
                Self::run_cd_cmd(args, &mut self.current_dir)?;
                self.children.push(ProcHandle::ProcStr(None));
            } else if in_cmd_map {
                let output = tls_get!(CMD_MAP)[command](args, envs)?;
                self.children.push(ProcHandle::ProcStr(Some(output)));
            } else {
                let mut child = cmd.spawn()?;
                if i != 0 {
                    if let Some(mut input) = child.stdin.take() {
                        if let ProcHandle::ProcStr(ss) = &mut self.children[i - 1] {
                            if let Some(s) = ss.take() {
                                input.write_all(s.as_bytes())?;
                            }
                        }
                    }
                }
                self.children.push(ProcHandle::ProcChild(child));
            }
        }

        Ok(WaitCmd(
            self.children,
            self.cmd_args
                .iter()
                .map(|c| c.get_args().join(" "))
                .collect(),
        ))
    }

    fn spawn_with_output(mut self) -> std::io::Result<WaitFun> {
        self.pipes.last_mut().unwrap().stdout(Stdio::piped());
        let children = self.spawn()?;
        Ok(WaitFun(children.0, children.1))
    }

    fn wait_child(child_handle: ProcHandle, cmd: &str) -> CmdResult {
        if let ProcHandle::ProcChild(mut child) = child_handle {
            let status = child.wait()?;
            if !status.success() {
                let mut pipefail = true;
                if let Ok(pipefail_str) = std::env::var("CMD_LIB_PIPEFAIL") {
                    pipefail = pipefail_str != "0";
                }
                if pipefail {
                    return Err(Self::to_io_error(
                        &format!("{} exited with error", cmd),
                        status,
                    ));
                }
            }
        }
        Ok(())
    }

    fn run_cd_cmd(args: Vec<String>, current_dir: &mut String) -> CmdResult {
        if args.len() == 1 {
            return Err(Error::new(ErrorKind::Other, "cd: missing directory"));
        } else if args.len() > 2 {
            let err_msg = format!("cd: too many arguments: {}", args.join(" "));
            return Err(Error::new(ErrorKind::Other, err_msg));
        }

        let dir = &args[1];
        if !std::path::Path::new(&dir).exists() {
            let err_msg = format!("cd: {}: No such file or directory", dir);
            eprintln!("{}", err_msg);
            return Err(Error::new(ErrorKind::Other, err_msg));
        }

        *current_dir = dir.clone();
        Ok(())
    }

    pub fn run_cmd(self) -> CmdResult {
        self.spawn()?.wait_result()
    }

    pub fn run_fun(self) -> FunResult {
        self.spawn_with_output()?.wait_result()
    }

    fn to_io_error(command: &str, status: ExitStatus) -> Error {
        if let Some(code) = status.code() {
            Error::new(
                ErrorKind::Other,
                format!("{}; status code: {}", command, code),
            )
        } else {
            Error::new(
                ErrorKind::Other,
                format!("{}; terminated by {}", command, status),
            )
        }
    }
}

#[doc(hidden)]
pub enum Redirect {
    FileToStdin(String),
    StdoutToStderr,
    StderrToStdout,
    StdoutToFile(String, bool),
    StderrToFile(String, bool),
}

#[doc(hidden)]
#[derive(Default)]
pub struct Cmd {
    args: Vec<String>,
    envs: HashMap<String, String>,
    redirects: Vec<Redirect>,
}

impl Cmd {
    pub fn add_arg(mut self, arg: String) -> Self {
        if self.args.is_empty() {
            let v: Vec<&str> = arg.split('=').collect();
            if v.len() == 2 && v[0].chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                self.envs.insert(v[0].to_owned(), v[1].to_owned());
                return self;
            }
        }
        self.args.push(arg);
        self
    }

    pub fn add_args(mut self, args: Vec<String>) -> Self {
        for arg in args {
            self = self.add_arg(arg);
        }
        self
    }

    pub fn get_args(&self) -> &Vec<String> {
        &self.args
    }

    pub fn get_envs(&self) -> &HashMap<String, String> {
        &self.envs
    }

    pub fn add_redirect(mut self, redirect: Redirect) -> Self {
        self.redirects.push(redirect);
        self
    }

    pub fn gen_command(&mut self) -> Command {
        let cmd_args: Vec<String> = self.get_args().to_vec();
        let mut cmd = Command::new(&cmd_args[0]);
        cmd.args(&cmd_args[1..]);
        self.setup_redirects(&mut cmd);
        cmd
    }

    fn setup_redirects(&self, cmd: &mut Command) {
        fn open_file(path: &str, append: bool) -> std::fs::File {
            OpenOptions::new()
                .create(true)
                .truncate(!append)
                .write(true)
                .append(append)
                .open(path)
                .unwrap()
        }

        for redirect in self.redirects.iter() {
            match redirect {
                Redirect::FileToStdin(path) => {
                    if path == "/dev/null" {
                        cmd.stdin(Stdio::null());
                    } else {
                        let file = OpenOptions::new().read(true).open(path).unwrap();
                        cmd.stdin(file);
                    }
                }
                Redirect::StdoutToStderr => {
                    let file = OpenOptions::new().write(true).open("/dev/stderr").unwrap();
                    cmd.stdout(file);
                }
                Redirect::StderrToStdout => {
                    let file = OpenOptions::new().write(true).open("/dev/stdout").unwrap();
                    cmd.stderr(file);
                }
                Redirect::StdoutToFile(path, append) => {
                    if path == "/dev/null" {
                        cmd.stdout(Stdio::null());
                    } else {
                        cmd.stdout(open_file(path, *append));
                    }
                }
                Redirect::StderrToFile(path, append) => {
                    if path == "/dev/null" {
                        cmd.stderr(Stdio::null());
                    } else {
                        cmd.stderr(open_file(path, *append));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_piped_cmds() {
        assert!(Cmds::default()
            .pipe(Cmd::default().add_args(vec!["echo".into(), "rust".into()]))
            .pipe(Cmd::default().add_args(vec!["wc".into()]))
            .run_cmd()
            .is_ok());
    }

    #[test]
    fn test_run_piped_funs() {
        assert_eq!(
            Cmds::default()
                .pipe(Cmd::default().add_args(vec!["echo".into(), "rust".into()]))
                .run_fun()
                .unwrap(),
            "rust"
        );

        assert_eq!(
            Cmds::default()
                .pipe(Cmd::default().add_args(vec!["echo".into(), "rust".into()]))
                .pipe(Cmd::default().add_args(vec!["wc".into(), "-c".into()]))
                .run_fun()
                .unwrap()
                .trim(),
            "5"
        );
    }

    #[test]
    fn test_stdout_redirect() {
        let tmp_file = "/tmp/file_echo_rust";
        let mut write_cmd = Cmd::default().add_args(vec!["echo".into(), "rust".into()]);
        write_cmd = write_cmd.add_redirect(Redirect::StdoutToFile(tmp_file.to_string(), false));
        assert!(Cmds::default().pipe(write_cmd).run_cmd().is_ok());

        let read_cmd = Cmd::default().add_args(vec!["cat".into(), tmp_file.into()]);
        assert_eq!(Cmds::default().pipe(read_cmd).run_fun().unwrap(), "rust");

        let cleanup_cmd = Cmd::default().add_args(vec!["rm".into(), tmp_file.into()]);
        assert!(Cmds::default().pipe(cleanup_cmd).run_cmd().is_ok());
    }
}
