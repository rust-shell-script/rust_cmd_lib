use crate::child::{CmdChild, CmdChildren};
use crate::io::{CmdIn, CmdOut};
use crate::{builtin_true, CmdResult, FunResult};
use faccess::{AccessMode, PathExt};
use lazy_static::lazy_static;
use log::{debug, error};
use os_pipe::{self, PipeReader, PipeWriter};
use std::collections::HashMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;

/// Environment for builtin or custom commands
pub struct CmdEnv {
    stdin: CmdIn,
    stdout: CmdOut,
    stderr: CmdOut,
    args: Vec<String>,
    vars: HashMap<String, String>,
    current_dir: String,
}
impl CmdEnv {
    /// Returns the arguments for this command
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Fetches the environment variable key for this command
    pub fn var(&self, key: &str) -> Option<&String> {
        self.vars.get(key)
    }

    /// Returns the current working directory for this command
    pub fn current_dir(&self) -> &str {
        &self.current_dir
    }

    /// Returns a new handle to the standard input for this command
    pub fn stdin(&mut self) -> impl Read + '_ {
        &mut self.stdin
    }

    /// Returns a new handle to the standard output for this command
    pub fn stdout(&mut self) -> impl Write + '_ {
        &mut self.stdout
    }

    /// Returns a new handle to the standard error for this command
    pub fn stderr(&mut self) -> impl Write + '_ {
        &mut self.stderr
    }
}

type FnFun = fn(&mut CmdEnv) -> CmdResult;

lazy_static! {
    static ref CMD_MAP: Mutex<HashMap<String, FnFun>> = {
        // needs explicit type, or it won't compile
        let mut m: HashMap<String, FnFun> = HashMap::new();
        m.insert("".into(), builtin_true);
        Mutex::new(m)
    };
}

#[doc(hidden)]
pub fn export_cmd(cmd: &'static str, func: FnFun) {
    CMD_MAP.lock().unwrap().insert(cmd.to_string(), func);
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
                    let ret = or_cmds.run_cmd(&mut self.current_dir);
                    if let Err(err) = ret {
                        error!("Running {} failed, Error: {}", or_cmds.get_full_cmds(), err);
                        return Err(err);
                    }
                } else {
                    error!("Running {} failed, Error: {}", cmds.0.get_full_cmds(), err);
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
                let or_ret = or_cmds.run_fun(&mut self.current_dir);
                if let Err(ref err) = or_ret {
                    error!("Running {} failed, Error: {}", or_cmds.get_full_cmds(), err);
                }
                or_ret
            } else {
                let full_cmds = last_cmd.0.get_full_cmds();
                error!("Running {} failed, Error: {}", full_cmds, e);
                Err(e)
            }
        } else {
            ret
        }
    }

    pub fn spawn(mut self) -> Result<CmdChildren> {
        assert_eq!(self.group_cmds.len(), 1);
        let mut cmds = self.group_cmds.pop().unwrap().0;
        let ret = cmds.spawn(&mut self.current_dir);
        if let Err(ref err) = ret {
            error!("Spawning {} failed, Error: {}", cmds.get_full_cmds(), err);
        }
        ret
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct Cmds {
    cmds: Vec<Option<Cmd>>,
    full_cmds: String,
}

impl Cmds {
    pub fn pipe(mut self, cmd: Cmd) -> Self {
        if !self.full_cmds.is_empty() {
            self.full_cmds += " | ";
        }
        self.full_cmds += &cmd.debug_str();
        self.cmds.push(Some(cmd.gen_command()));
        self
    }

    fn get_full_cmds(&self) -> &str {
        &self.full_cmds
    }

    fn spawn(&mut self, current_dir: &mut String) -> Result<CmdChildren> {
        if std::env::var("CMD_LIB_DEBUG") == Ok("1".into()) {
            debug!("Running {} ...", self.get_full_cmds());
        }

        // spawning all the sub-processes
        let mut children: Vec<CmdChild> = Vec::new();
        let len = self.cmds.len();
        let mut last_pipe_in = None;
        for (i, cmd_opt) in self.cmds.iter_mut().enumerate() {
            let mut cmd = cmd_opt.take().unwrap();
            if i != len - 1 {
                // not the last, update redirects
                let (pipe_reader, pipe_writer) = os_pipe::pipe()?;
                cmd.setup_redirects(&mut last_pipe_in, Some(pipe_writer))?;
                last_pipe_in = Some(pipe_reader);
            } else {
                cmd.setup_redirects(&mut last_pipe_in, None)?;
            }
            let child = cmd.spawn(current_dir)?;
            children.push(child);
        }

        Ok(CmdChildren::from(children))
    }

    fn run_cmd(&mut self, current_dir: &mut String) -> CmdResult {
        self.spawn(current_dir)?.wait_cmd_result_nolog()
    }

    fn run_fun(&mut self, current_dir: &mut String) -> FunResult {
        self.spawn(current_dir)?.wait_fun_result_nolog()
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
    in_cmd_map: bool,
    args: Vec<String>,
    vars: HashMap<String, String>,
    redirects: Vec<Redirect>,

    // for running
    std_cmd: Option<Command>,
    stdin_redirect: Option<CmdIn>,
    stdout_redirect: Option<CmdOut>,
    stderr_redirect: Option<CmdOut>,
    stdout_logging: Option<PipeReader>,
    stderr_logging: Option<PipeReader>,
}

impl Default for Cmd {
    fn default() -> Self {
        Cmd {
            in_cmd_map: true,
            args: vec![],
            vars: HashMap::new(),
            redirects: vec![],
            std_cmd: None,
            stdin_redirect: None,
            stdout_redirect: None,
            stderr_redirect: None,
            stdout_logging: None,
            stderr_logging: None,
        }
    }
}

impl Cmd {
    pub fn add_arg(mut self, arg: String) -> Self {
        if self.args.is_empty() {
            let v: Vec<&str> = arg.split('=').collect();
            if v.len() == 2 && v[0].chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                self.vars.insert(v[0].to_owned(), v[1].to_owned());
                return self;
            }
            self.in_cmd_map = CMD_MAP.lock().unwrap().contains_key(arg.as_str());
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

    pub fn add_redirect(mut self, redirect: Redirect) -> Self {
        self.redirects.push(redirect);
        self
    }

    fn arg0(&self) -> String {
        if self.args.is_empty() {
            "".into()
        } else {
            self.args[0].clone()
        }
    }

    fn debug_str(&self) -> String {
        let mut ret = format!("{:?}", self.args);
        let mut extra = String::new();
        if !self.vars.is_empty() {
            extra += &format!("{:?}", self.vars);
        }
        if !self.redirects.is_empty() {
            if !extra.is_empty() {
                extra += ", ";
            }
            extra += &format!("{:?}", self.redirects);
        }
        if !extra.is_empty() {
            ret += &format!("({})", extra);
        }
        ret
    }

    fn gen_command(mut self) -> Self {
        if !self.in_cmd_map {
            let cmds: Vec<String> = self.args.to_vec();
            let mut cmd = Command::new(&cmds[0]);
            cmd.args(&cmds[1..]);
            for (k, v) in self.vars.iter() {
                cmd.env(k, v);
            }
            self.std_cmd = Some(cmd);
        }
        self
    }

    fn spawn(mut self, current_dir: &mut String) -> Result<CmdChild> {
        let arg0 = self.arg0();
        let full_cmd = self.debug_str();
        if arg0 == "cd" {
            self.run_cd_cmd(current_dir)?;
            Ok(CmdChild::SyncFn {
                cmd: full_cmd,
                stdout: None,
                stderr: None,
            })
        } else if self.in_cmd_map {
            let pipe_out = self.stdout_logging.is_none();
            let mut env = CmdEnv {
                args: self.args,
                vars: self.vars,
                current_dir: current_dir.clone(),
                stdin: if let Some(redirect_in) = self.stdin_redirect.take() {
                    redirect_in
                } else {
                    CmdIn::CmdPipe(os_pipe::dup_stdin()?)
                },
                stdout: if let Some(redirect_out) = self.stdout_redirect.take() {
                    redirect_out
                } else {
                    CmdOut::CmdPipe(os_pipe::dup_stdout()?)
                },
                stderr: if let Some(redirect_err) = self.stderr_redirect.take() {
                    redirect_err
                } else {
                    CmdOut::CmdPipe(os_pipe::dup_stderr()?)
                },
            };

            let internal_cmd = CMD_MAP.lock().unwrap()[&arg0];
            if pipe_out {
                let handle = std::thread::spawn(move || internal_cmd(&mut env));
                Ok(CmdChild::ThreadFn {
                    child: handle,
                    stdout: self.stdout_logging,
                    stderr: self.stderr_logging,
                    cmd: full_cmd,
                })
            } else {
                internal_cmd(&mut env)?;
                drop(env);
                Ok(CmdChild::SyncFn {
                    cmd: full_cmd,
                    stdout: self.stdout_logging,
                    stderr: self.stderr_logging,
                })
            }
        } else {
            let mut cmd = self.std_cmd.take().unwrap();

            // setup current_dir
            if !current_dir.is_empty() {
                cmd.current_dir(current_dir.clone());
            }

            // update stdin
            if let Some(redirect_in) = self.stdin_redirect.take() {
                cmd.stdin(redirect_in);
            }

            // update stdout
            if let Some(redirect_out) = self.stdout_redirect.take() {
                cmd.stdout(redirect_out);
            } else {
                cmd.stdout(Stdio::piped());
            }

            // update stderr
            if let Some(redirect_err) = self.stderr_redirect.take() {
                cmd.stderr(redirect_err);
            }

            // spawning process
            let child = cmd.spawn()?;
            Ok(CmdChild::Proc {
                cmd: full_cmd,
                stderr: self.stderr_logging,
                child,
            })
        }
    }

    fn run_cd_cmd(&self, current_dir: &mut String) -> CmdResult {
        if self.args.len() == 1 {
            return Err(Error::new(ErrorKind::Other, "cd: missing directory"));
        } else if self.args.len() > 2 {
            let err_msg = format!("cd: too many arguments: {}", self.debug_str());
            return Err(Error::new(ErrorKind::Other, err_msg));
        }

        let dir = &self.args[1];
        if !std::path::Path::new(&dir).is_dir() {
            let err_msg = format!("cd: {}: No such file or directory", dir);
            error!("{}", err_msg);
            return Err(Error::new(ErrorKind::Other, err_msg));
        }

        if let Err(e) = Path::new(dir).access(AccessMode::EXECUTE) {
            error!("cd {}: {}", dir, e);
            return Err(e);
        }

        *current_dir = dir.clone();
        Ok(())
    }

    fn open_file(path: &str, read_only: bool, append: bool) -> Result<File> {
        if read_only {
            OpenOptions::new().read(true).open(path)
        } else {
            OpenOptions::new()
                .create(true)
                .truncate(!append)
                .write(true)
                .append(append)
                .open(path)
        }
    }

    fn setup_redirects(
        &mut self,
        pipe_in: &mut Option<PipeReader>,
        pipe_out: Option<PipeWriter>,
    ) -> CmdResult {
        // set up error pipe
        let (pipe_reader, pipe_writer) = os_pipe::pipe()?;
        self.stderr_redirect = Some(CmdOut::CmdPipe(pipe_writer));
        self.stderr_logging = Some(pipe_reader);

        if let Some(pipe) = pipe_out {
            self.stdout_redirect = Some(CmdOut::CmdPipe(pipe));
        } else if self.in_cmd_map {
            // set up stdout pipe
            let (pipe_reader, pipe_writer) = os_pipe::pipe()?;
            self.stdout_redirect = Some(CmdOut::CmdPipe(pipe_writer));
            self.stdout_logging = Some(pipe_reader);
        }

        if let Some(pipe) = pipe_in.take() {
            self.stdin_redirect = Some(CmdIn::CmdPipe(pipe));
        }

        for redirect in self.redirects.iter() {
            match redirect {
                Redirect::FileToStdin(path) => {
                    self.stdin_redirect = Some(if path == "/dev/null" {
                        CmdIn::CmdNull
                    } else {
                        CmdIn::CmdFile(Self::open_file(path, true, false)?)
                    });
                }
                Redirect::StdoutToStderr => {
                    if let Some(ref redirect) = self.stderr_redirect {
                        self.stdout_redirect = Some(redirect.try_clone()?);
                    } else {
                        self.stdout_redirect = Some(CmdOut::CmdPipe(os_pipe::dup_stderr()?));
                    }
                }
                Redirect::StderrToStdout => {
                    if let Some(ref redirect) = self.stdout_redirect {
                        self.stderr_redirect = Some(redirect.try_clone()?);
                    } else {
                        self.stderr_redirect = Some(CmdOut::CmdPipe(os_pipe::dup_stdout()?));
                    }
                }
                Redirect::StdoutToFile(path, append) => {
                    self.stdout_redirect = Some(if path == "/dev/null" {
                        CmdOut::CmdNull
                    } else {
                        CmdOut::CmdFile(Self::open_file(path, false, *append)?)
                    });
                }
                Redirect::StderrToFile(path, append) => {
                    self.stderr_redirect = Some(if path == "/dev/null" {
                        CmdOut::CmdNull
                    } else {
                        CmdOut::CmdFile(Self::open_file(path, false, *append)?)
                    });
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
