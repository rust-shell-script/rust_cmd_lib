use crate::{tls_get, tls_init, tls_set, CmdResult, FunResult};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::process::{Child, Command, ExitStatus, Stdio};
use tempfile::tempfile;

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
    ProcFile(Option<File>),
}

pub struct WaitCmd(Vec<ProcHandle>, Vec<String>);
impl WaitCmd {
    pub fn wait_result(&mut self) -> CmdResult {
        let len = self.0.len();
        for i in (0..len).rev() {
            if i == len - 1 {
                match self.0.pop().unwrap() {
                    ProcHandle::ProcChild(mut child) => {
                        let status = child.wait()?;
                        if !status.success() {
                            return Err(Cmds::to_io_error(
                                &format!("{} exited with error", self.1[i]),
                                status,
                            ));
                        }
                    }
                    ProcHandle::ProcFile(mut ff) => {
                        if let Some(mut f) = ff.take() {
                            let mut s = String::new();
                            f.read_to_string(&mut s)?;
                            println!("{}", s);
                        }
                    }
                }
            } else {
                Cmds::wait_child(self.0.pop().unwrap(), &self.1[i])?;
            }
        }
        Ok(())
    }
}

pub struct WaitFun(Vec<ProcHandle>, Vec<String>);
impl WaitFun {
    pub fn wait_result(&mut self) -> FunResult {
        let mut ret = String::new();
        let len = self.0.len();
        for i in (0..len).rev() {
            if i == len - 1 {
                match self.0.pop().unwrap() {
                    ProcHandle::ProcChild(child) => {
                        let output = child.wait_with_output()?;
                        if !output.status.success() {
                            return Err(Cmds::to_io_error(
                                &format!("{} exited with error", self.1[i]),
                                output.status,
                            ));
                        } else {
                            ret = String::from_utf8_lossy(&output.stdout).to_string();
                        }
                    }
                    ProcHandle::ProcFile(mut ff) => {
                        if let Some(mut f) = ff.take() {
                            let mut s = String::new();
                            f.read_to_string(&mut s)?;
                            ret = s;
                        }
                    }
                }
            } else {
                Cmds::wait_child(self.0.pop().unwrap(), &self.1[i])?;
            }
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
    pub fn from_cmd(mut cmd: Cmd) -> Self {
        Self {
            pipes: vec![cmd.gen_command()],
            children: vec![],
            cmd_args: vec![cmd],
            current_dir: String::new(),
        }
    }

    pub fn pipe(mut self, mut cmd: Cmd) -> Self {
        if !self.pipes.is_empty() {
            let last_i = self.pipes.len() - 1;
            self.pipes[last_i].stdout(Stdio::piped());
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

    fn spawn_with_output(mut self) -> std::io::Result<WaitFun> {
        self.pipes.last_mut().unwrap().stdout(Stdio::piped());
        let children = self.spawn()?;
        Ok(WaitFun(children.0, children.1))
    }

    fn spawn(mut self) -> std::io::Result<WaitCmd> {
        if let Ok(debug) = std::env::var("CMD_LIB_DEBUG") {
            if debug == "1" {
                eprintln!("Running \"{}\" ...", self.get_full_cmd());
            }
        }

        // spawning all the sub-processes
        for (i, mut cmd) in self.pipes.into_iter().enumerate() {
            if i != 0 {
                match &mut self.children[i - 1] {
                    ProcHandle::ProcChild(child) => {
                        if let Some(output) = child.stdout.take() {
                            cmd.stdin(output);
                        }
                    }
                    ProcHandle::ProcFile(ff) => {
                        if let Some(f) = ff.take() {
                            cmd.stdin(f);
                        }
                    }
                }
            }

            // check commands defined in CMD_MAP
            let args = self.cmd_args[i].get_args().clone();
            let envs = self.cmd_args[i].get_envs().clone();
            let command = &args[0].as_str();
            let in_cmd_map = tls_get!(CMD_MAP).contains_key(command);
            if command == &"cd" {
                let dir = Self::run_cd_cmd(args)?;
                self.current_dir = dir;
                self.children.push(ProcHandle::ProcFile(None));
            } else if in_cmd_map {
                let output = tls_get!(CMD_MAP)[command](args, envs)?;
                if output.is_empty() {
                    self.children.push(ProcHandle::ProcFile(None));
                } else {
                    let mut file = tempfile()?;
                    writeln!(file, "{}", output)?;
                    self.children.push(ProcHandle::ProcFile(Some(file)));
                }
            } else {
                let child = cmd.spawn()?;
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

    fn wait_child(child_handle: ProcHandle, cmd: &str) -> CmdResult {
        match child_handle {
            ProcHandle::ProcChild(mut child) => {
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
            _ => {}
        }
        Ok(())
    }

    fn run_cd_cmd(args: Vec<String>) -> FunResult {
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

        Ok(dir.clone())
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
pub enum FdOrFile {
    Fd(i32, bool),          // fd, append?
    File(String, bool),     // file, append?
    OpenedFile(File, bool), // opened file, append?
}

#[doc(hidden)]
#[derive(Default)]
pub struct Cmd {
    args: Vec<String>,
    envs: HashMap<String, String>,
    redirects: Vec<(i32, FdOrFile)>,
}

impl Cmd {
    pub fn add_arg(mut self, arg: String) -> Self {
        if self.is_empty() {
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

    pub fn from_args<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            args: args.into_iter().map(|s| s.as_ref().to_owned()).collect(),
            envs: HashMap::new(),
            redirects: vec![],
        }
    }

    pub fn get_args(&self) -> &Vec<String> {
        &self.args
    }

    pub fn get_envs(&self) -> &HashMap<String, String> {
        &self.envs
    }

    pub fn set_redirect(mut self, fd: i32, target: FdOrFile) -> Self {
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
                    let out = unsafe { Stdio::from_raw_fd(*fd) };
                    match *fd_src {
                        1 => cmd.stdout(out),
                        2 => cmd.stderr(out),
                        _ => panic!("invalid fd: {}", *fd_src),
                    };
                }
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
                            OpenOptions::new().read(true).open(file).unwrap()
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
                        let out = unsafe { Stdio::from_raw_fd(fd) };
                        match *fd_src {
                            0 => cmd.stdin(out),
                            1 => cmd.stdout(out),
                            2 => cmd.stderr(out),
                            _ => panic!("invalid fd: {}", *fd_src),
                        };
                        *target = FdOrFile::OpenedFile(f, *append);
                    }
                }
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
            .run_cmd()
            .is_ok());
    }

    #[test]
    fn test_run_piped_funs() {
        assert_eq!(
            Cmds::from_cmd(Cmd::from_args(vec!["echo", "rust"]))
                .run_fun()
                .unwrap(),
            "rust"
        );

        assert_eq!(
            Cmds::from_cmd(Cmd::from_args(vec!["echo", "rust"]))
                .pipe(Cmd::from_args(vec!["wc", "-c"]))
                .run_fun()
                .unwrap()
                .trim(),
            "5"
        );
    }

    #[test]
    fn test_stdout_redirect() {
        let tmp_file = "/tmp/file_echo_rust";
        let mut write_cmd = Cmd::from_args(vec!["echo", "rust"]);
        write_cmd = write_cmd.set_redirect(1, FdOrFile::File(tmp_file.to_string(), false));
        assert!(Cmds::from_cmd(write_cmd).run_cmd().is_ok());

        let read_cmd = Cmd::from_args(vec!["cat", tmp_file]);
        assert_eq!(Cmds::from_cmd(read_cmd).run_fun().unwrap(), "rust");

        let cleanup_cmd = Cmd::from_args(vec!["rm", tmp_file]);
        assert!(Cmds::from_cmd(cleanup_cmd).run_cmd().is_ok());
    }
}
