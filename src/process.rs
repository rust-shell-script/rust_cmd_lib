use crate::{CmdResult, FunResult};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fmt;
use std::fs::OpenOptions;
use std::io::{Error, ErrorKind, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;

pub type CmdArgs = Vec<String>;
pub type CmdEnvs = HashMap<String, String>;
/// IO struct for builtin or custom commands
#[derive(Default, Debug)]
pub struct CmdStdio {
    pub inbuf: Vec<u8>,
    pub outbuf: Vec<u8>,
    pub errbuf: Vec<u8>,
}
type FnFun = fn(CmdArgs, CmdEnvs, &mut CmdStdio) -> CmdResult;

lazy_static! {
    static ref CMD_MAP: Mutex<HashMap<&'static str, FnFun>> = {
        // needs explicit type, or it won't compile
        let m: HashMap<&'static str, FnFun> = HashMap::new();
        Mutex::new(m)
    };
}

#[doc(hidden)]
pub fn export_cmd(cmd: &'static str, func: FnFun) {
    CMD_MAP.lock().unwrap().insert(cmd, func);
}

/// set debug mode or not, false by default
///
/// Setting environment variable CMD_LIB_DEBUG=0|1 has the same effect
pub fn set_debug(enable: bool) {
    std::env::set_var("CMD_LIB_DEBUG", if enable { "1" } else { "0" });
}

/// set pipefail or not, true by default
///
/// Setting environment variable CMD_LIB_PIPEFAIL=0|1 has the same effect
pub fn set_pipefail(enable: bool) {
    std::env::set_var("CMD_LIB_PIPEFAIL", if enable { "1" } else { "0" });
}

#[doc(hidden)]
#[derive(Default)]
pub struct GroupCmds {
    cmds: Vec<(Cmds, Option<Cmds>)>, // (cmd, orCmd) pairs
    current_dir: String,
}

impl GroupCmds {
    pub fn add(mut self, cmds: Cmds, or_cmds: Option<Cmds>) -> Self {
        self.cmds.push((cmds, or_cmds));
        self
    }

    pub fn run_cmd(&mut self) -> CmdResult {
        for cmd in self.cmds.iter_mut() {
            if let Err(err) = cmd.0.run_cmd(&mut self.current_dir) {
                if let Some(or_cmds) = &mut cmd.1 {
                    or_cmds.run_cmd(&mut self.current_dir)?;
                } else {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub fn run_fun(&mut self) -> FunResult {
        let mut last_cmd = self.cmds.pop().unwrap();
        self.run_cmd()?;
        // run last function command
        let ret = last_cmd.0.run_fun(&mut self.current_dir);
        if let Err(e) = ret {
            if let Some(or_cmds) = &mut last_cmd.1 {
                or_cmds.run_fun(&mut self.current_dir)
            } else {
                Err(e)
            }
        } else {
            ret
        }
    }

    pub fn spawn(mut self) -> std::io::Result<WaitCmd> {
        assert_eq!(self.cmds.len(), 1);
        let mut cmds = self.cmds.pop().unwrap().0;
        cmds.spawn(&mut self.current_dir, false)
    }

    pub fn spawn_with_output(mut self) -> std::io::Result<WaitFun> {
        assert_eq!(self.cmds.len(), 1);
        let mut cmds = self.cmds.pop().unwrap().0;
        cmds.spawn_with_output(&mut self.current_dir)
    }
}

enum ProcHandle {
    ProcChild(Option<Child>), // for normal commands
    ProcBuf(Option<Vec<u8>>), // for builtin/custom commands
}

pub struct WaitCmd(Vec<ProcHandle>, Vec<String>);
impl WaitCmd {
    pub fn wait_result(&mut self) -> CmdResult {
        // wait last process result
        let (handle, cmd) = (self.0.pop().unwrap(), self.1.pop().unwrap());
        match handle {
            ProcHandle::ProcChild(child_opt) => {
                if let Some(mut child) = child_opt {
                    let status = child.wait()?;
                    if !status.success() {
                        return Err(Cmds::status_to_io_error(
                            status,
                            &format!("{} exited with error", cmd),
                        ));
                    }
                }
            }
            ProcHandle::ProcBuf(mut ss) => {
                if let Some(s) = ss.take() {
                    std::io::stdout().write_all(&s)?;
                }
            }
        }
        Cmds::wait_children(&mut self.0, &mut self.1)
    }
}

pub struct WaitFun(Vec<ProcHandle>, Vec<String>);
impl WaitFun {
    fn wait_output(handle: &mut ProcHandle, cmd: &str) -> std::io::Result<Vec<u8>> {
        match handle {
            ProcHandle::ProcChild(child_opt) => {
                if let Some(child) = child_opt.take() {
                    let output = child.wait_with_output()?;
                    if !output.status.success() {
                        return Err(Cmds::status_to_io_error(
                            output.status,
                            &format!("{} exited with error", cmd),
                        ));
                    } else {
                        return Ok(output.stdout);
                    }
                }
            }
            ProcHandle::ProcBuf(ss) => {
                if let Some(s) = ss.take() {
                    return Ok(s);
                }
            }
        }
        Ok(vec![])
    }

    pub fn wait_result(&mut self) -> FunResult {
        // wait last process result
        let (mut handle, cmd) = (self.0.pop().unwrap(), self.1.pop().unwrap());
        let mut ret = String::from_utf8_lossy(&Self::wait_output(&mut handle, &cmd)?).to_string();
        if ret.ends_with('\n') {
            ret.pop();
        }
        Cmds::wait_children(&mut self.0, &mut self.1)?;
        Ok(ret)
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct Cmds {
    pipes: Vec<Command>,
    cmd_args: Vec<Cmd>,
    current_dir: String,
}

impl Cmds {
    pub fn pipe(mut self, mut cmd: Cmd) -> Self {
        let mut pipe_cmd = cmd.gen_command();
        for (k, v) in cmd.get_envs() {
            pipe_cmd.env(k, v);
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
            ret += &format!("{:?}", cmd_arg.get_args());
            let mut extra = String::new();
            if !cmd_arg.get_envs().is_empty() {
                extra += &format!("envs: {:?}", cmd_arg.get_envs());
            }
            if !cmd_arg.get_redirects().is_empty() {
                if !extra.is_empty() {
                    extra += ", ";
                }
                extra += &format!("redirects: {:?}", cmd_arg.get_redirects());
            }
            if !extra.is_empty() {
                ret += &format!(" ({})", extra);
            }
        }
        ret
    }

    fn spawn(&mut self, current_dir: &mut String, for_fun: bool) -> std::io::Result<WaitCmd> {
        if std::env::var("CMD_LIB_DEBUG") == Ok("1".into()) {
            eprintln!("Running {} ...", self.get_full_cmd());
        }

        let len = self.pipes.len();
        let mut children: Vec<ProcHandle> = Vec::new();
        // spawning all the sub-processes
        for (i, cmd) in self.pipes.iter_mut().enumerate() {
            // check commands defined in CMD_MAP
            let args = self.cmd_args[i].get_args().clone();
            let envs = self.cmd_args[i].get_envs().clone();
            let command = &args[0].as_str();
            let in_cmd_map = CMD_MAP.lock().unwrap().contains_key(command);

            if i != 0 && !in_cmd_map {
                let mut stdin_setup_done = false;
                if let ProcHandle::ProcChild(Some(child)) = &mut children[i - 1] {
                    if let Some(output) = child.stdout.take() {
                        cmd.stdin(output);
                        stdin_setup_done = true;
                    }
                }
                if !stdin_setup_done {
                    cmd.stdin(Stdio::piped());
                }
            }

            // inherit outside current_dir setting
            if self.current_dir.is_empty() {
                self.current_dir = current_dir.clone();
            }
            // update current_dir for current process
            if !self.current_dir.is_empty() {
                cmd.current_dir(self.current_dir.clone());
            }

            if command == &"cd" {
                Self::run_cd_cmd(&args, &mut self.current_dir)?;
                *current_dir = self.current_dir.clone();
                children.push(ProcHandle::ProcBuf(None));
            } else if in_cmd_map {
                let mut io = CmdStdio::default();
                if i == 0 {
                    if let Some(path) = self.cmd_args[i].get_stdin_redirect() {
                        io.inbuf = std::fs::read(path)?;
                    }
                } else {
                    io.inbuf = WaitFun::wait_output(
                        &mut children[i - 1],
                        &self.cmd_args[i - 1].get_args().join(" "),
                    )?;
                }
                let internal_cmd = CMD_MAP.lock().unwrap()[command];
                internal_cmd(args, envs, &mut io)?;
                if let Some((path, append)) = self.cmd_args[i].get_stderr_redirect() {
                    Cmd::open_file(path, *append).write_all(&io.errbuf)?;
                } else {
                    std::io::stderr().write_all(&io.errbuf)?;
                }
                if let Some((path, append)) = self.cmd_args[i].get_stdout_redirect() {
                    Cmd::open_file(path, *append).write_all(&io.outbuf)?;
                    children.push(ProcHandle::ProcBuf(None));
                } else {
                    children.push(ProcHandle::ProcBuf(Some(io.outbuf)));
                }
            } else {
                if i == len - 1 && !for_fun && self.cmd_args[i].get_stdout_redirect().is_none() {
                    cmd.stdout(Stdio::inherit());
                }
                let mut child = cmd.spawn()?;
                if i != 0 {
                    if let Some(mut input) = child.stdin.take() {
                        if let ProcHandle::ProcBuf(ss) = &mut children[i - 1] {
                            if let Some(s) = ss.take() {
                                input.write_all(&s)?;
                            }
                        }
                    }
                }
                children.push(ProcHandle::ProcChild(Some(child)));
            }
        }

        Ok(WaitCmd(
            children,
            self.cmd_args
                .iter()
                .map(|c| c.get_args().join(" "))
                .collect(),
        ))
    }

    fn spawn_with_output(&mut self, current_dir: &mut String) -> std::io::Result<WaitFun> {
        let children = self.spawn(current_dir, true)?;
        Ok(WaitFun(children.0, children.1))
    }

    fn wait_children(children: &mut Vec<ProcHandle>, cmds: &mut Vec<String>) -> CmdResult {
        while !children.is_empty() {
            let (child_handle, cmd) = (children.pop().unwrap(), cmds.pop().unwrap());
            if let ProcHandle::ProcChild(Some(mut child)) = child_handle {
                let status = child.wait()?;
                if !status.success() && std::env::var("CMD_LIB_PIPEFAIL") != Ok("0".into()) {
                    return Err(Self::status_to_io_error(
                        status,
                        &format!("{} exited with error", cmd),
                    ));
                }
            }
        }
        Ok(())
    }

    fn run_cd_cmd(args: &[String], current_dir: &mut String) -> CmdResult {
        if args.len() == 1 {
            return Err(Error::new(ErrorKind::Other, "cd: missing directory"));
        } else if args.len() > 2 {
            let err_msg = format!("cd: too many arguments: {}", args.join(" "));
            return Err(Error::new(ErrorKind::Other, err_msg));
        }

        let dir = &args[1];
        if !std::path::Path::new(&dir).is_dir() {
            let err_msg = format!("cd: {}: No such file or directory", dir);
            eprintln!("{}", err_msg);
            return Err(Error::new(ErrorKind::Other, err_msg));
        }

        *current_dir = dir.clone();
        Ok(())
    }

    fn run_cmd(&mut self, current_dir: &mut String) -> CmdResult {
        let mut handle = self.spawn(current_dir, false)?;
        self.pipes.clear(); // to avoid wait deadlock
        handle.wait_result()
    }

    fn run_fun(&mut self, current_dir: &mut String) -> FunResult {
        let mut handle = self.spawn_with_output(current_dir)?;
        self.pipes.clear(); // to avoid wait deadlock
        handle.wait_result()
    }

    fn status_to_io_error(status: ExitStatus, command: &str) -> Error {
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
impl fmt::Debug for Redirect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Redirect::FileToStdin(path) => f.write_str(&format!("< {}", path)),
            Redirect::StdoutToStderr => f.write_str(">&2"),
            Redirect::StderrToStdout => f.write_str("2>&1"),
            Redirect::StdoutToFile(path, append) => {
                if *append {
                    f.write_str(&format!("1>> {}", path))
                } else {
                    f.write_str(&format!("1> {}", path))
                }
            }
            Redirect::StderrToFile(path, append) => {
                if *append {
                    f.write_str(&format!("2>> {}", path))
                } else {
                    f.write_str(&format!("2> {}", path))
                }
            }
        }
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct Cmd {
    args: Vec<String>,
    envs: HashMap<String, String>,
    redirects: Vec<Redirect>,
    stdin_redirect: Option<String>,
    stdout_redirect: Option<(String, bool)>,
    stderr_redirect: Option<(String, bool)>,
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

    fn get_args(&self) -> &Vec<String> {
        &self.args
    }

    fn get_envs(&self) -> &HashMap<String, String> {
        &self.envs
    }

    pub fn add_redirect(mut self, redirect: Redirect) -> Self {
        self.redirects.push(redirect);
        self
    }

    fn get_stdin_redirect(&self) -> &Option<String> {
        &self.stdin_redirect
    }

    fn get_stdout_redirect(&self) -> &Option<(String, bool)> {
        &self.stdout_redirect
    }

    fn get_stderr_redirect(&self) -> &Option<(String, bool)> {
        &self.stderr_redirect
    }

    fn get_redirects(&self) -> &Vec<Redirect> {
        &self.redirects
    }

    fn gen_command(&mut self) -> Command {
        let cmd_args: Vec<String> = self.get_args().to_vec();
        let mut cmd = Command::new(&cmd_args[0]);
        cmd.args(&cmd_args[1..]);
        cmd.stdout(Stdio::piped());
        self.setup_redirects(&mut cmd);
        cmd
    }

    fn open_file(path: &str, append: bool) -> std::fs::File {
        OpenOptions::new()
            .create(true)
            .truncate(!append)
            .write(true)
            .append(append)
            .open(path)
            .unwrap()
    }

    fn setup_redirects(&mut self, cmd: &mut Command) {
        let mut stdout_file = "/dev/stdout";
        let mut stderr_file = "/dev/stderr";
        let in_cmd_map = CMD_MAP.lock().unwrap().contains_key(self.args[0].as_str());
        for redirect in self.redirects.iter() {
            match redirect {
                Redirect::FileToStdin(path) => {
                    if !in_cmd_map {
                        if path == "/dev/null" {
                            cmd.stdin(Stdio::null());
                        } else {
                            let file = OpenOptions::new().read(true).open(path).unwrap();
                            cmd.stdin(file);
                        }
                    }
                    self.stdin_redirect = Some(path.into());
                }
                Redirect::StdoutToStderr => {
                    if !in_cmd_map {
                        cmd.stdout(Self::open_file(stderr_file, true));
                    }
                    self.stdout_redirect = Some(("/dev/stderr".into(), false));
                }
                Redirect::StderrToStdout => {
                    if !in_cmd_map {
                        cmd.stderr(Self::open_file(stdout_file, true));
                    }
                    self.stderr_redirect = Some(("/dev/stdout".into(), false));
                }
                Redirect::StdoutToFile(path, append) => {
                    if !in_cmd_map {
                        if path == "/dev/null" {
                            cmd.stdout(Stdio::null());
                        } else {
                            cmd.stdout(Self::open_file(path, *append));
                            stdout_file = path;
                        }
                    }
                    self.stdout_redirect = Some((path.into(), *append));
                }
                Redirect::StderrToFile(path, append) => {
                    if !in_cmd_map {
                        if path == "/dev/null" {
                            cmd.stderr(Stdio::null());
                        } else {
                            cmd.stderr(Self::open_file(path, *append));
                            stderr_file = path;
                        }
                    }
                    self.stderr_redirect = Some((path.into(), *append));
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
        let mut current_dir = String::new();
        assert!(Cmds::default()
            .pipe(Cmd::default().add_args(vec!["echo".into(), "rust".into()]))
            .pipe(Cmd::default().add_args(vec!["wc".into()]))
            .run_cmd(&mut current_dir)
            .is_ok());
    }

    #[test]
    fn test_run_piped_funs() {
        let mut current_dir = String::new();
        assert_eq!(
            Cmds::default()
                .pipe(Cmd::default().add_args(vec!["echo".into(), "rust".into()]))
                .run_fun(&mut current_dir)
                .unwrap(),
            "rust"
        );

        assert_eq!(
            Cmds::default()
                .pipe(Cmd::default().add_args(vec!["echo".into(), "rust".into()]))
                .pipe(Cmd::default().add_args(vec!["wc".into(), "-c".into()]))
                .run_fun(&mut current_dir)
                .unwrap()
                .trim(),
            "5"
        );
    }

    #[test]
    fn test_stdout_redirect() {
        let mut current_dir = String::new();
        let tmp_file = "/tmp/file_echo_rust";
        let mut write_cmd = Cmd::default().add_args(vec!["echo".into(), "rust".into()]);
        write_cmd = write_cmd.add_redirect(Redirect::StdoutToFile(tmp_file.to_string(), false));
        assert!(Cmds::default()
            .pipe(write_cmd)
            .run_cmd(&mut current_dir)
            .is_ok());

        let read_cmd = Cmd::default().add_args(vec!["cat".into(), tmp_file.into()]);
        assert_eq!(
            Cmds::default()
                .pipe(read_cmd)
                .run_fun(&mut current_dir)
                .unwrap(),
            "rust"
        );

        let cleanup_cmd = Cmd::default().add_args(vec!["rm".into(), tmp_file.into()]);
        assert!(Cmds::default()
            .pipe(cleanup_cmd)
            .run_cmd(&mut current_dir)
            .is_ok());
    }
}
