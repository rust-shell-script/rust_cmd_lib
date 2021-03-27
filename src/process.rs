use crate::{builtin_true, CmdResult, FunResult};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fmt;
use std::fs::{File, OpenOptions};
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
        let mut m: HashMap<&'static str, FnFun> = HashMap::new();
        m.insert("", builtin_true);
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
    group_cmds: Vec<(Cmds, Option<Cmds>)>, // (cmd, orCmd) pairs
    current_dir: String,
}

impl GroupCmds {
    pub fn add(mut self, cmds: Cmds, or_cmds: Option<Cmds>) -> Self {
        self.group_cmds.push((cmds, or_cmds));
        self
    }

    pub fn run_cmd(&mut self) -> CmdResult {
        for cmds in self.group_cmds.iter_mut() {
            if let Err(err) = cmds.0.run_cmd(&mut self.current_dir) {
                if let Some(or_cmds) = &mut cmds.1 {
                    or_cmds.run_cmd(&mut self.current_dir)?;
                } else {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub fn run_fun(&mut self) -> FunResult {
        let mut last_cmd = self.group_cmds.pop().unwrap();
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
        assert_eq!(self.group_cmds.len(), 1);
        let mut cmds = self.group_cmds.pop().unwrap().0;
        cmds.spawn(&mut self.current_dir, false)
    }

    pub fn spawn_with_output(mut self) -> std::io::Result<WaitFun> {
        assert_eq!(self.group_cmds.len(), 1);
        let mut cmds = self.group_cmds.pop().unwrap().0;
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
                    let status_result = child.wait();
                    match status_result {
                        Err(e) => {
                            let _ = Cmds::wait_children(&mut self.0, &mut self.1);
                            return Err(e);
                        }
                        Ok(status) => {
                            if !status.success() {
                                let _ = Cmds::wait_children(&mut self.0, &mut self.1);
                                return Err(Cmds::status_to_io_error(
                                    status,
                                    &format!("{} exited with error", cmd),
                                ));
                            }
                        }
                    }
                }
            }
            ProcHandle::ProcBuf(mut ss) => {
                if let Some(s) = ss.take() {
                    let result = std::io::stdout().write_all(&s);
                    if let Err(e) = result {
                        let _ = Cmds::wait_children(&mut self.0, &mut self.1);
                        return Err(e);
                    }
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

    pub fn wait_raw_result(&mut self) -> std::io::Result<Vec<u8>> {
        let (mut handle, cmd) = (self.0.pop().unwrap(), self.1.pop().unwrap());
        let wait_last = Self::wait_output(&mut handle, &cmd);
        match wait_last {
            Err(e) => {
                let _ = Cmds::wait_children(&mut self.0, &mut self.1);
                Err(e)
            }
            Ok(output) => {
                Cmds::wait_children(&mut self.0, &mut self.1)?;
                Ok(output)
            }
        }
    }

    pub fn wait_result(&mut self) -> FunResult {
        // wait last process result
        let (mut handle, cmd) = (self.0.pop().unwrap(), self.1.pop().unwrap());
        let wait_last = Self::wait_output(&mut handle, &cmd);
        match wait_last {
            Err(e) => {
                let _ = Cmds::wait_children(&mut self.0, &mut self.1);
                Err(e)
            }
            Ok(output) => {
                let mut ret = String::from_utf8_lossy(&output).to_string();
                if ret.ends_with('\n') {
                    ret.pop();
                }
                Cmds::wait_children(&mut self.0, &mut self.1)?;
                Ok(ret)
            }
        }
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct Cmds {
    cmds: Vec<Cmd>,
}

impl Cmds {
    pub fn pipe(mut self, mut cmd: Cmd) -> Self {
        let mut pipe_cmd_opt = cmd.gen_command();
        for (k, v) in cmd.get_envs() {
            if let Some(pipe_cmd) = pipe_cmd_opt.as_mut() {
                pipe_cmd.env(k, v);
            }
        }
        cmd.set_std_cmd(pipe_cmd_opt);
        self.cmds.push(cmd);
        self
    }

    fn get_full_cmd(&self) -> String {
        self.cmds
            .iter()
            .map(|cmd| cmd.debug_str())
            .collect::<Vec<String>>()
            .join(" | ")
    }

    fn spawn(&mut self, current_dir: &mut String, for_fun: bool) -> std::io::Result<WaitCmd> {
        if std::env::var("CMD_LIB_DEBUG") == Ok("1".into()) {
            eprintln!("Running {} ...", self.get_full_cmd());
        }

        // set up redirects
        for cmd in self.cmds.iter_mut() {
            cmd.setup_redirects()?;
        }

        let len = self.cmds.len();
        let mut children: Vec<ProcHandle> = Vec::new();
        // spawning all the sub-processes
        for i in 0..len {
            let cur_cmd = &mut self.cmds[i];
            let mut cmd_opt = cur_cmd.get_std_cmd();
            let args = cur_cmd.get_args().clone();
            let envs = cur_cmd.get_envs().clone();
            if i != 0 && !cur_cmd.in_cmd_map {
                let mut stdin_setup_done = false;
                if let ProcHandle::ProcChild(Some(child)) = &mut children[i - 1] {
                    if let Some(output) = child.stdout.take() {
                        if let Some(cmd) = cmd_opt.as_mut() {
                            cmd.stdin(output);
                        }
                        stdin_setup_done = true;
                    }
                }
                if !stdin_setup_done {
                    if let Some(cmd) = cmd_opt.as_mut() {
                        cmd.stdin(Stdio::piped());
                    }
                }
            }

            if let Some(cmd) = cmd_opt.as_mut() {
                if !current_dir.is_empty() {
                    cmd.current_dir(current_dir.clone());
                }
            }

            let arg0 = cur_cmd.get_arg0();
            if arg0 == "cd" {
                Self::run_cd_cmd(&args, current_dir)?;
                children.push(ProcHandle::ProcBuf(None));
            } else if cur_cmd.in_cmd_map {
                let mut io = CmdStdio::default();
                if i == 0 {
                    if let Some(path) = cur_cmd.get_stdin_redirect() {
                        io.inbuf = std::fs::read(path)?;
                    }
                } else {
                    io.inbuf = WaitFun::wait_output(
                        &mut children[i - 1],
                        &self.cmds[i - 1].get_args().join(" ").clone(),
                    )?;
                }
                let internal_cmd = CMD_MAP.lock().unwrap()[arg0.as_str()];
                internal_cmd(args, envs, &mut io)?;
                if let Some((path, append)) = self.cmds[i].get_stderr_redirect() {
                    Cmd::open_file(path, *append)?.write_all(&io.errbuf)?;
                } else {
                    std::io::stderr().write_all(&io.errbuf)?;
                }
                if let Some((path, append)) = self.cmds[i].get_stdout_redirect() {
                    Cmd::open_file(path, *append)?.write_all(&io.outbuf)?;
                    children.push(ProcHandle::ProcBuf(None));
                } else {
                    children.push(ProcHandle::ProcBuf(Some(io.outbuf)));
                }
            } else {
                if i == len - 1 && !for_fun && self.cmds[i].get_stdout_redirect().is_none() {
                    if let Some(cmd) = cmd_opt.as_mut() {
                        cmd.stdout(Stdio::inherit());
                    }
                }
                let mut child = cmd_opt.as_mut().unwrap().spawn()?;
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
            self.cmds.iter().map(|c| c.get_args().join(" ")).collect(),
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
        self.spawn(current_dir, false)?.wait_result()
    }

    fn run_fun(&mut self, current_dir: &mut String) -> FunResult {
        self.spawn_with_output(current_dir)?.wait_result()
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
pub struct Cmd {
    // for parsing
    arg0: String,
    in_cmd_map: bool,
    args: Vec<String>,
    envs: HashMap<String, String>,
    redirects: Vec<Redirect>,
    stdin_redirect: Option<String>,
    stdout_redirect: Option<(String, bool)>,
    stderr_redirect: Option<(String, bool)>,

    // for running
    std_cmd: Option<Command>,
}

impl Default for Cmd {
    fn default() -> Self {
        Cmd {
            arg0: "".into(),
            in_cmd_map: true,
            args: vec![],
            envs: HashMap::new(),
            redirects: vec![],
            stdin_redirect: None,
            stdout_redirect: None,
            stderr_redirect: None,
            std_cmd: None,
        }
    }
}

impl Cmd {
    pub fn add_arg(mut self, arg: String) -> Self {
        if self.args.is_empty() {
            self.in_cmd_map = CMD_MAP.lock().unwrap().contains_key(arg.as_str());
            self.arg0 = arg.clone();

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

    fn get_arg0(&self) -> String {
        self.arg0.clone()
    }

    fn get_args(&self) -> &Vec<String> {
        &self.args
    }

    fn get_envs(&self) -> &HashMap<String, String> {
        &self.envs
    }

    fn debug_str(&self) -> String {
        let mut ret = String::new();
        ret += &format!("{:?}", self.args);
        let mut extra = String::new();
        if !self.envs.is_empty() {
            extra += &format!("envs: {:?}", self.envs);
        }
        if !self.redirects.is_empty() {
            if !extra.is_empty() {
                extra += ", ";
            }
            extra += &format!("redirects: {:?}", self.redirects);
        }
        if !extra.is_empty() {
            ret += &format!(" ({})", extra);
        }
        ret
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

    fn gen_command(&mut self) -> Option<Command> {
        let in_cmd_map =
            self.args.is_empty() || CMD_MAP.lock().unwrap().contains_key(self.args[0].as_str());
        if in_cmd_map {
            None
        } else {
            let cmds: Vec<String> = self.get_args().to_vec();
            let mut cmd = Command::new(&cmds[0]);
            cmd.args(&cmds[1..]);
            cmd.stdout(Stdio::piped());
            Some(cmd)
        }
    }

    fn get_std_cmd(&mut self) -> Option<Command> {
        self.std_cmd.take()
    }

    fn set_std_cmd(&mut self, cmd_opt: Option<Command>) {
        self.std_cmd = cmd_opt;
    }

    fn open_file(path: &str, append: bool) -> std::io::Result<File> {
        OpenOptions::new()
            .create(true)
            .truncate(!append)
            .write(true)
            .append(append)
            .open(path)
    }

    fn setup_redirects(&mut self) -> CmdResult {
        let mut stdout_file = "/dev/stdout";
        let mut stderr_file = "/dev/stderr";
        for redirect in self.redirects.iter() {
            match redirect {
                Redirect::FileToStdin(path) => {
                    if let Some(cmd) = self.std_cmd.as_mut() {
                        if path == "/dev/null" {
                            cmd.stdin(Stdio::null());
                        } else {
                            let file = OpenOptions::new().read(true).open(path)?;
                            cmd.stdin(file);
                        }
                    }
                    self.stdin_redirect = Some(path.into());
                }
                Redirect::StdoutToStderr => {
                    if let Some(cmd) = self.std_cmd.as_mut() {
                        cmd.stdout(Self::open_file(stderr_file, true)?);
                    }
                    self.stdout_redirect = Some(("/dev/stderr".into(), false));
                }
                Redirect::StderrToStdout => {
                    if let Some(cmd) = self.std_cmd.as_mut() {
                        cmd.stderr(Self::open_file(stdout_file, true)?);
                    }
                    self.stderr_redirect = Some(("/dev/stdout".into(), false));
                }
                Redirect::StdoutToFile(path, append) => {
                    if let Some(cmd) = self.std_cmd.as_mut() {
                        if path == "/dev/null" {
                            cmd.stdout(Stdio::null());
                        } else {
                            cmd.stdout(Self::open_file(path, *append)?);
                            stdout_file = path;
                        }
                    }
                    self.stdout_redirect = Some((path.into(), *append));
                }
                Redirect::StderrToFile(path, append) => {
                    if let Some(cmd) = self.std_cmd.as_mut() {
                        if path == "/dev/null" {
                            cmd.stderr(Stdio::null());
                        } else {
                            cmd.stderr(Self::open_file(path, *append)?);
                            stderr_file = path;
                        }
                    }
                    self.stderr_redirect = Some((path.into(), *append));
                }
            }
        }
        Ok(())
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
