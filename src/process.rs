use crate::builtins::*;
use crate::child::{CmdChild, CmdChildHandle, CmdChildren, FunChildren};
use crate::io::{CmdIn, CmdOut};
use crate::{debug, warn};
use crate::{CmdResult, FunResult};
use faccess::{AccessMode, PathExt};
use lazy_static::lazy_static;
use os_pipe::{self, PipeReader, PipeWriter};
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Result};
use std::marker::PhantomData;
use std::mem::take;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{LazyLock, Mutex};
use std::thread;

const CD_CMD: &str = "cd";
const IGNORE_CMD: &str = "ignore";

/// Environment for builtin or custom commands.
pub struct CmdEnv {
    stdin: CmdIn,
    stdout: CmdOut,
    stderr: CmdOut,
    args: Vec<String>,
    vars: HashMap<String, String>,
    current_dir: PathBuf,
}
impl CmdEnv {
    /// Returns the name of this command.
    pub fn get_cmd_name(&self) -> &str {
        &self.args[0]
    }

    /// Returns the arguments for this command.
    pub fn get_args(&self) -> &[String] {
        &self.args[1..]
    }

    /// Fetches the environment variable key for this command.
    pub fn var(&self, key: &str) -> Option<&String> {
        self.vars.get(key)
    }

    /// Returns the current working directory for this command.
    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    /// Returns a new handle to the standard input for this command.
    pub fn stdin(&mut self) -> &mut CmdIn {
        &mut self.stdin
    }

    /// Returns a new handle to the standard output for this command.
    pub fn stdout(&mut self) -> &mut CmdOut {
        &mut self.stdout
    }

    /// Returns a new handle to the standard error for this command.
    pub fn stderr(&mut self) -> &mut CmdOut {
        &mut self.stderr
    }
}

type FnFun = fn(&mut CmdEnv) -> CmdResult;

lazy_static! {
    static ref CMD_MAP: Mutex<HashMap<OsString, FnFun>> = {
        // needs explicit type, or it won't compile
        let mut m: HashMap<OsString, FnFun> = HashMap::new();
        m.insert("echo".into(), builtin_echo);
        m.insert("trace".into(), builtin_trace);
        m.insert("debug".into(), builtin_debug);
        m.insert("info".into(), builtin_info);
        m.insert("warn".into(), builtin_warn);
        m.insert("error".into(), builtin_error);
        m.insert("".into(), builtin_empty);

        Mutex::new(m)
    };
}

#[doc(hidden)]
pub fn register_cmd(cmd: &'static str, func: FnFun) {
    CMD_MAP.lock().unwrap().insert(OsString::from(cmd), func);
}

/// Whether debug mode is enabled globally.
/// Can be overridden by the thread-local setting in [`DEBUG_OVERRIDE`].
static DEBUG_ENABLED: LazyLock<AtomicBool> =
    LazyLock::new(|| AtomicBool::new(std::env::var("CMD_LIB_DEBUG") == Ok("1".into())));

/// Whether debug mode is enabled globally.
/// Can be overridden by the thread-local setting in [`PIPEFAIL_OVERRIDE`].
static PIPEFAIL_ENABLED: LazyLock<AtomicBool> =
    LazyLock::new(|| AtomicBool::new(std::env::var("CMD_LIB_PIPEFAIL") != Ok("0".into())));

/// Set debug mode or not, false by default.
///
/// This is **global**, and affects all threads. To set it for the current thread only, use [`ScopedDebug`].
///
/// Setting environment variable CMD_LIB_DEBUG=0|1 has the same effect, but the environment variable is only
/// checked once at an unspecified time, so the only reliable way to do that is when the program is first started.
pub fn set_debug(enable: bool) {
    DEBUG_ENABLED.store(enable, SeqCst);
}

/// Set pipefail or not, true by default.
///
/// This is **global**, and affects all threads. To set it for the current thread only, use [`ScopedPipefail`].
///
/// Setting environment variable CMD_LIB_DEBUG=0|1 has the same effect, but the environment variable is only
/// checked once at an unspecified time, so the only reliable way to do that is when the program is first started.
pub fn set_pipefail(enable: bool) {
    PIPEFAIL_ENABLED.store(enable, SeqCst);
}

pub(crate) fn debug_enabled() -> bool {
    DEBUG_OVERRIDE
        .get()
        .unwrap_or_else(|| DEBUG_ENABLED.load(SeqCst))
}

pub(crate) fn pipefail_enabled() -> bool {
    PIPEFAIL_OVERRIDE
        .get()
        .unwrap_or_else(|| PIPEFAIL_ENABLED.load(SeqCst))
}

thread_local! {
    /// Whether debug mode is enabled in the current thread.
    /// None means to use the global setting in [`DEBUG_ENABLED`].
    static DEBUG_OVERRIDE: Cell<Option<bool>> = Cell::new(None);

    /// Whether pipefail mode is enabled in the current thread.
    /// None means to use the global setting in [`PIPEFAIL_ENABLED`].
    static PIPEFAIL_OVERRIDE: Cell<Option<bool>> = Cell::new(None);
}

/// Overrides the debug mode in the current thread, while the value is in scope.
///
/// Each override restores the previous value when dropped, so they can be nested.
/// Since overrides are thread-local, these values can’t be sent across threads.
///
/// ```
/// # use cmd_lib::{ScopedDebug, run_cmd};
/// // Must give the variable a name, not just `_`
/// let _debug = ScopedDebug::set(true);
/// run_cmd!(echo hello world)?; // Will have debug on
/// # Ok::<(), std::io::Error>(())
/// ```
// PhantomData field is equivalent to `impl !Send for Self {}`
pub struct ScopedDebug(Option<bool>, PhantomData<*const ()>);

/// Overrides the pipefail mode in the current thread, while the value is in scope.
///
/// Each override restores the previous value when dropped, so they can be nested.
/// Since overrides are thread-local, these values can’t be sent across threads.
// PhantomData field is equivalent to `impl !Send for Self {}`
///
/// ```
/// # use cmd_lib::{ScopedPipefail, run_cmd};
/// // Must give the variable a name, not just `_`
/// let _debug = ScopedPipefail::set(false);
/// run_cmd!(false | true)?; // Will have pipefail off
/// # Ok::<(), std::io::Error>(())
/// ```
pub struct ScopedPipefail(Option<bool>, PhantomData<*const ()>);

impl ScopedDebug {
    /// ```compile_fail
    /// let _: Box<dyn Send> = Box::new(cmd_lib::ScopedDebug::set(true));
    /// ```
    /// ```compile_fail
    /// let _: Box<dyn Sync> = Box::new(cmd_lib::ScopedDebug::set(true));
    /// ```
    #[doc(hidden)]
    pub fn test_not_send_not_sync() {}

    pub fn set(enabled: bool) -> Self {
        let result = Self(DEBUG_OVERRIDE.get(), PhantomData);
        DEBUG_OVERRIDE.set(Some(enabled));
        result
    }
}
impl Drop for ScopedDebug {
    fn drop(&mut self) {
        DEBUG_OVERRIDE.set(self.0)
    }
}

impl ScopedPipefail {
    /// ```compile_fail
    /// let _: Box<dyn Send> = Box::new(cmd_lib::ScopedPipefail::set(true));
    /// ```
    /// ```compile_fail
    /// let _: Box<dyn Sync> = Box::new(cmd_lib::ScopedPipefail::set(true));
    /// ```
    #[doc(hidden)]
    pub fn test_not_send_not_sync() {}

    pub fn set(enabled: bool) -> Self {
        let result = Self(PIPEFAIL_OVERRIDE.get(), PhantomData);
        PIPEFAIL_OVERRIDE.set(Some(enabled));
        result
    }
}
impl Drop for ScopedPipefail {
    fn drop(&mut self) {
        PIPEFAIL_OVERRIDE.set(self.0)
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct GroupCmds {
    group_cmds: Vec<Cmds>,
    current_dir: PathBuf,
}

impl GroupCmds {
    pub fn append(mut self, cmds: Cmds) -> Self {
        self.group_cmds.push(cmds);
        self
    }

    pub fn run_cmd(&mut self) -> CmdResult {
        for cmds in self.group_cmds.iter_mut() {
            if let Err(e) = cmds.run_cmd(&mut self.current_dir) {
                if !cmds.ignore_error {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    pub fn run_fun(&mut self) -> FunResult {
        // run previous commands
        let mut last_cmd = self.group_cmds.pop().unwrap();
        self.run_cmd()?;
        // run last function command
        let ret = last_cmd.run_fun(&mut self.current_dir);
        if ret.is_err() && last_cmd.ignore_error {
            return Ok("".into());
        }
        ret
    }

    pub fn spawn(mut self, with_output: bool) -> Result<CmdChildren> {
        assert_eq!(self.group_cmds.len(), 1);
        let mut cmds = self.group_cmds.pop().unwrap();
        cmds.spawn(&mut self.current_dir, with_output)
    }

    pub fn spawn_with_output(self) -> Result<FunChildren> {
        self.spawn(true).map(CmdChildren::into_fun_children)
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct Cmds {
    cmds: Vec<Cmd>,
    full_cmds: String,
    ignore_error: bool,
    file: String,
    line: u32,
}

impl Cmds {
    pub fn pipe(mut self, cmd: Cmd) -> Self {
        if self.full_cmds.is_empty() {
            self.file = cmd.file.clone();
            self.line = cmd.line;
        } else {
            self.full_cmds += " | ";
        }
        self.full_cmds += &cmd.cmd_str();
        let (ignore_error, cmd) = cmd.gen_command();
        if ignore_error {
            if self.cmds.is_empty() {
                // first command in the pipe
                self.ignore_error = true;
            } else {
                warn!(
                    "Builtin {IGNORE_CMD:?} command at wrong position ({}:{})",
                    self.file, self.line
                );
            }
        }
        self.cmds.push(cmd);
        self
    }

    fn spawn(&mut self, current_dir: &mut PathBuf, with_output: bool) -> Result<CmdChildren> {
        let full_cmds = self.full_cmds.clone();
        let file = self.file.clone();
        let line = self.line;
        if debug_enabled() {
            debug!("Running [{full_cmds}] at {file}:{line} ...");
        }

        // spawning all the sub-processes
        let mut children: Vec<CmdChild> = Vec::new();
        let len = self.cmds.len();
        let mut prev_pipe_in = None;
        for (i, mut cmd) in take(&mut self.cmds).into_iter().enumerate() {
            if i != len - 1 {
                // not the last, update redirects
                let (pipe_reader, pipe_writer) =
                    os_pipe::pipe().map_err(|e| new_cmd_io_error(&e, &full_cmds, &file, line))?;
                cmd.setup_redirects(&mut prev_pipe_in, Some(pipe_writer), with_output)
                    .map_err(|e| new_cmd_io_error(&e, &full_cmds, &file, line))?;
                prev_pipe_in = Some(pipe_reader);
            } else {
                cmd.setup_redirects(&mut prev_pipe_in, None, with_output)
                    .map_err(|e| new_cmd_io_error(&e, &full_cmds, &file, line))?;
            }
            let child = cmd
                .spawn(full_cmds.clone(), current_dir, with_output)
                .map_err(|e| new_cmd_io_error(&e, &full_cmds, &file, line))?;
            children.push(child);
        }

        Ok(CmdChildren::new(children, self.ignore_error))
    }

    fn spawn_with_output(&mut self, current_dir: &mut PathBuf) -> Result<FunChildren> {
        self.spawn(current_dir, true)
            .map(CmdChildren::into_fun_children)
    }

    fn run_cmd(&mut self, current_dir: &mut PathBuf) -> CmdResult {
        self.spawn(current_dir, false)?.wait()
    }

    fn run_fun(&mut self, current_dir: &mut PathBuf) -> FunResult {
        self.spawn_with_output(current_dir)?.wait_with_output()
    }
}

#[doc(hidden)]
pub enum Redirect {
    FileToStdin(PathBuf),
    StdoutToStderr,
    StderrToStdout,
    StdoutToFile(PathBuf, bool),
    StderrToFile(PathBuf, bool),
}
impl fmt::Debug for Redirect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Redirect::FileToStdin(path) => f.write_str(&format!("<{:?}", path.display())),
            Redirect::StdoutToStderr => f.write_str(">&2"),
            Redirect::StderrToStdout => f.write_str("2>&1"),
            Redirect::StdoutToFile(path, append) => {
                if *append {
                    f.write_str(&format!("1>>{:?}", path.display()))
                } else {
                    f.write_str(&format!("1>{:?}", path.display()))
                }
            }
            Redirect::StderrToFile(path, append) => {
                if *append {
                    f.write_str(&format!("2>>{:?}", path.display()))
                } else {
                    f.write_str(&format!("2>{:?}", path.display()))
                }
            }
        }
    }
}

#[doc(hidden)]
pub struct Cmd {
    // for parsing
    in_cmd_map: bool,
    args: Vec<OsString>,
    vars: HashMap<String, String>,
    redirects: Vec<Redirect>,
    file: String,
    line: u32,

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
            file: "".into(),
            line: 0,
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
    pub fn with_location(mut self, file: &str, line: u32) -> Self {
        self.file = file.into();
        self.line = line;
        self
    }

    pub fn add_arg<O>(mut self, arg: O) -> Self
    where
        O: AsRef<OsStr>,
    {
        let arg = arg.as_ref();
        if arg.is_empty() {
            // Skip empty arguments
            return self;
        }

        let arg_str = arg.to_string_lossy().to_string();
        if arg_str != IGNORE_CMD && !self.args.iter().any(|cmd| *cmd != IGNORE_CMD) {
            if let Some((key, value)) = arg_str.split_once('=') {
                if key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    self.vars.insert(key.into(), value.into());
                    return self;
                }
            }
            self.in_cmd_map = CMD_MAP.lock().unwrap().contains_key(arg);
        }
        self.args.push(arg.to_os_string());
        self
    }

    pub fn add_args<I, O>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: AsRef<OsStr>,
    {
        for arg in args {
            self = self.add_arg(arg);
        }
        self
    }

    pub fn add_redirect(mut self, redirect: Redirect) -> Self {
        self.redirects.push(redirect);
        self
    }

    fn arg0(&self) -> OsString {
        let mut args = self.args.iter().skip_while(|cmd| *cmd == IGNORE_CMD);
        if let Some(arg) = args.next() {
            return arg.into();
        }
        "".into()
    }

    fn cmd_str(&self) -> String {
        self.vars
            .iter()
            .map(|(k, v)| format!("{k}={v:?}"))
            .chain(self.args.iter().map(|s| format!("{s:?}")))
            .chain(self.redirects.iter().map(|r| format!("{r:?}")))
            .collect::<Vec<String>>()
            .join(" ")
    }

    fn gen_command(mut self) -> (bool, Self) {
        let args: Vec<OsString> = self
            .args
            .iter()
            .skip_while(|cmd| *cmd == IGNORE_CMD)
            .map(|s| s.into())
            .collect();
        if !self.in_cmd_map {
            let mut cmd = Command::new(&args[0]);
            cmd.args(&args[1..]);
            for (k, v) in self.vars.iter() {
                cmd.env(k, v);
            }
            self.std_cmd = Some(cmd);
        }
        (self.args.len() > args.len(), self)
    }

    fn spawn(
        mut self,
        full_cmds: String,
        current_dir: &mut PathBuf,
        with_output: bool,
    ) -> Result<CmdChild> {
        let arg0 = self.arg0();
        if arg0 == CD_CMD {
            self.run_cd_cmd(current_dir, &self.file, self.line)?;
            Ok(CmdChild::new(
                CmdChildHandle::SyncFn,
                full_cmds,
                self.file,
                self.line,
                self.stdout_logging,
                self.stderr_logging,
            ))
        } else if self.in_cmd_map {
            let pipe_out = self.stdout_logging.is_none();
            let mut env = CmdEnv {
                args: self
                    .args
                    .into_iter()
                    .skip_while(|cmd| *cmd == IGNORE_CMD)
                    .map(|s| s.to_string_lossy().to_string())
                    .collect(),
                vars: self.vars,
                current_dir: if current_dir.as_os_str().is_empty() {
                    std::env::current_dir()?
                } else {
                    current_dir.clone()
                },
                stdin: if let Some(redirect_in) = self.stdin_redirect.take() {
                    redirect_in
                } else {
                    CmdIn::pipe(os_pipe::dup_stdin()?)
                },
                stdout: if let Some(redirect_out) = self.stdout_redirect.take() {
                    redirect_out
                } else {
                    CmdOut::pipe(os_pipe::dup_stdout()?)
                },
                stderr: if let Some(redirect_err) = self.stderr_redirect.take() {
                    redirect_err
                } else {
                    CmdOut::pipe(os_pipe::dup_stderr()?)
                },
            };

            let internal_cmd = CMD_MAP.lock().unwrap()[&arg0];
            if pipe_out || with_output {
                let handle = thread::Builder::new().spawn(move || internal_cmd(&mut env))?;
                Ok(CmdChild::new(
                    CmdChildHandle::Thread(Some(handle)),
                    full_cmds,
                    self.file,
                    self.line,
                    self.stdout_logging,
                    self.stderr_logging,
                ))
            } else {
                internal_cmd(&mut env)?;
                Ok(CmdChild::new(
                    CmdChildHandle::SyncFn,
                    full_cmds,
                    self.file,
                    self.line,
                    self.stdout_logging,
                    self.stderr_logging,
                ))
            }
        } else {
            let mut cmd = self.std_cmd.take().unwrap();

            // setup current_dir
            if !current_dir.as_os_str().is_empty() {
                cmd.current_dir(current_dir.clone());
            }

            // update stdin
            if let Some(redirect_in) = self.stdin_redirect.take() {
                cmd.stdin(redirect_in);
            }

            // update stdout
            if let Some(redirect_out) = self.stdout_redirect.take() {
                cmd.stdout(redirect_out);
            }

            // update stderr
            if let Some(redirect_err) = self.stderr_redirect.take() {
                cmd.stderr(redirect_err);
            }

            // spawning process
            let child = cmd.spawn()?;
            Ok(CmdChild::new(
                CmdChildHandle::Proc(child),
                full_cmds,
                self.file,
                self.line,
                self.stdout_logging,
                self.stderr_logging,
            ))
        }
    }

    fn run_cd_cmd(&self, current_dir: &mut PathBuf, file: &str, line: u32) -> CmdResult {
        if self.args.len() == 1 {
            return Err(Error::new(
                ErrorKind::Other,
                "{CD_CMD}: missing directory at {file}:{line}",
            ));
        } else if self.args.len() > 2 {
            let err_msg = format!("{CD_CMD}: too many arguments at {file}:{line}");
            return Err(Error::new(ErrorKind::Other, err_msg));
        }

        let dir = current_dir.join(&self.args[1]);
        if !dir.is_dir() {
            let err_msg = format!("{CD_CMD}: No such file or directory at {file}:{line}");
            return Err(Error::new(ErrorKind::Other, err_msg));
        }

        dir.access(AccessMode::EXECUTE)?;
        *current_dir = dir;
        Ok(())
    }

    fn open_file(path: &Path, read_only: bool, append: bool) -> Result<File> {
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
        with_output: bool,
    ) -> CmdResult {
        // set up stdin pipe
        if let Some(pipe) = pipe_in.take() {
            self.stdin_redirect = Some(CmdIn::pipe(pipe));
        }
        // set up stdout pipe
        if let Some(pipe) = pipe_out {
            self.stdout_redirect = Some(CmdOut::pipe(pipe));
        } else if with_output {
            let (pipe_reader, pipe_writer) = os_pipe::pipe()?;
            self.stdout_redirect = Some(CmdOut::pipe(pipe_writer));
            self.stdout_logging = Some(pipe_reader);
        }
        // set up stderr pipe
        let (pipe_reader, pipe_writer) = os_pipe::pipe()?;
        self.stderr_redirect = Some(CmdOut::pipe(pipe_writer));
        self.stderr_logging = Some(pipe_reader);

        for redirect in self.redirects.iter() {
            match redirect {
                Redirect::FileToStdin(path) => {
                    self.stdin_redirect = Some(if path == Path::new("/dev/null") {
                        CmdIn::null()
                    } else {
                        CmdIn::file(Self::open_file(path, true, false)?)
                    });
                }
                Redirect::StdoutToStderr => {
                    if let Some(ref redirect) = self.stderr_redirect {
                        self.stdout_redirect = Some(redirect.try_clone()?);
                    } else {
                        self.stdout_redirect = Some(CmdOut::pipe(os_pipe::dup_stderr()?));
                    }
                }
                Redirect::StderrToStdout => {
                    if let Some(ref redirect) = self.stdout_redirect {
                        self.stderr_redirect = Some(redirect.try_clone()?);
                    } else {
                        self.stderr_redirect = Some(CmdOut::pipe(os_pipe::dup_stdout()?));
                    }
                }
                Redirect::StdoutToFile(path, append) => {
                    self.stdout_redirect = Some(if path == Path::new("/dev/null") {
                        CmdOut::null()
                    } else {
                        CmdOut::file(Self::open_file(path, false, *append)?)
                    });
                }
                Redirect::StderrToFile(path, append) => {
                    self.stderr_redirect = Some(if path == Path::new("/dev/null") {
                        CmdOut::null()
                    } else {
                        CmdOut::file(Self::open_file(path, false, *append)?)
                    });
                }
            }
        }
        Ok(())
    }
}

#[doc(hidden)]
pub trait AsOsStr {
    fn as_os_str(&self) -> OsString;
}

impl<T: ToString> AsOsStr for T {
    fn as_os_str(&self) -> OsString {
        self.to_string().into()
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct CmdString(OsString);
impl CmdString {
    pub fn append<T: AsRef<OsStr>>(mut self, value: T) -> Self {
        self.0.push(value);
        self
    }

    pub fn into_os_string(self) -> OsString {
        self.0
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.0.into()
    }
}

impl AsRef<OsStr> for CmdString {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl<T: ?Sized + AsRef<OsStr>> From<&T> for CmdString {
    fn from(s: &T) -> Self {
        Self(s.as_ref().into())
    }
}

impl fmt::Display for CmdString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_string_lossy())
    }
}

pub(crate) fn new_cmd_io_error(e: &Error, command: &str, file: &str, line: u32) -> Error {
    Error::new(
        e.kind(),
        format!("Running [{command}] failed: {e} at {file}:{line}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_piped_cmds() {
        let mut current_dir = PathBuf::new();
        assert!(Cmds::default()
            .pipe(Cmd::default().add_args(["echo", "rust"]))
            .pipe(Cmd::default().add_args(["wc"]))
            .run_cmd(&mut current_dir)
            .is_ok());
    }

    #[test]
    fn test_run_piped_funs() {
        let mut current_dir = PathBuf::new();
        assert_eq!(
            Cmds::default()
                .pipe(Cmd::default().add_args(["echo", "rust"]))
                .run_fun(&mut current_dir)
                .unwrap(),
            "rust"
        );

        assert_eq!(
            Cmds::default()
                .pipe(Cmd::default().add_args(["echo", "rust"]))
                .pipe(Cmd::default().add_args(["wc", "-c"]))
                .run_fun(&mut current_dir)
                .unwrap()
                .trim(),
            "5"
        );
    }

    #[test]
    fn test_stdout_redirect() {
        let mut current_dir = PathBuf::new();
        let tmp_file = "/tmp/file_echo_rust";
        let mut write_cmd = Cmd::default().add_args(["echo", "rust"]);
        write_cmd = write_cmd.add_redirect(Redirect::StdoutToFile(PathBuf::from(tmp_file), false));
        assert!(Cmds::default()
            .pipe(write_cmd)
            .run_cmd(&mut current_dir)
            .is_ok());

        let read_cmd = Cmd::default().add_args(["cat", tmp_file]);
        assert_eq!(
            Cmds::default()
                .pipe(read_cmd)
                .run_fun(&mut current_dir)
                .unwrap(),
            "rust"
        );

        let cleanup_cmd = Cmd::default().add_args(["rm", tmp_file]);
        assert!(Cmds::default()
            .pipe(cleanup_cmd)
            .run_cmd(&mut current_dir)
            .is_ok());
    }
}
