use crate::{CmdResult, FunResult, process};
use crate::{info, warn};
use os_pipe::PipeReader;
use std::any::Any;
use std::fmt::Display;
use std::io::{BufRead, BufReader, Error, Read, Result};
use std::process::{Child, ExitStatus};
use std::thread::JoinHandle;

/// Representation of running or exited children processes, connected with pipes
/// optionally.
///
/// Calling [`spawn!`](../cmd_lib/macro.spawn.html) macro will return `Result<CmdChildren>`
pub struct CmdChildren {
    children: Vec<CmdChild>,
    ignore_error: bool,
}

impl CmdChildren {
    pub(crate) fn new(children: Vec<CmdChild>, ignore_error: bool) -> Self {
        Self {
            children,
            ignore_error,
        }
    }

    pub(crate) fn into_fun_children(self) -> FunChildren {
        FunChildren {
            children: self.children,
            ignore_error: self.ignore_error,
        }
    }

    /// Waits for the children processes to exit completely, returning the status that they exited with.
    pub fn wait(&mut self) -> CmdResult {
        let last_child = self.children.pop().unwrap();
        let last_child_res = last_child.wait(true);
        let other_children_res = Self::wait_children(&mut self.children);

        if self.ignore_error {
            Ok(())
        } else {
            last_child_res.and(other_children_res)
        }
    }

    fn wait_children(children: &mut Vec<CmdChild>) -> CmdResult {
        let mut ret = Ok(());
        while let Some(child_handle) = children.pop() {
            if let Err(e) = child_handle.wait(false) {
                ret = Err(e);
            }
        }
        ret
    }

    /// Forces the children processes to exit.
    pub fn kill(&mut self) -> CmdResult {
        let mut ret = Ok(());
        while let Some(child_handle) = self.children.pop() {
            if let Err(e) = child_handle.kill() {
                ret = Err(e);
            }
        }
        ret
    }

    /// Returns the OS-assigned process identifiers associated with these children processes
    pub fn pids(&self) -> Vec<u32> {
        self.children.iter().filter_map(|x| x.pid()).collect()
    }
}

/// Representation of running or exited children processes with output, connected with pipes
/// optionally.
///
/// Calling [spawn_with_output!](../cmd_lib/macro.spawn_with_output.html) macro will return `Result<FunChildren>`
pub struct FunChildren {
    children: Vec<CmdChild>,
    ignore_error: bool,
}

impl FunChildren {
    /// Waits for the children processes to exit completely, returning the command result, stdout
    /// content string and stderr content string.
    pub fn wait_with_all(&mut self) -> (CmdResult, String, String) {
        self.inner_wait_with_all(true)
    }

    /// Waits for the children processes to exit completely, returning the stdout output.
    pub fn wait_with_output(&mut self) -> FunResult {
        let (res, stdout, _) = self.inner_wait_with_all(false);
        if let Err(e) = res
            && !self.ignore_error
        {
            return Err(e);
        }
        Ok(stdout)
    }

    /// Waits for the children processes to exit completely, and read all bytes from stdout into `buf`.
    pub fn wait_with_raw_output(&mut self, buf: &mut Vec<u8>) -> CmdResult {
        // wait for the last child result
        let handle = self.children.pop().unwrap();
        let wait_last = handle.wait_with_raw_output(self.ignore_error, buf);
        match wait_last {
            Err(e) => {
                let _ = CmdChildren::wait_children(&mut self.children);
                Err(e)
            }
            Ok(_) => {
                let ret = CmdChildren::wait_children(&mut self.children);
                if self.ignore_error { Ok(()) } else { ret }
            }
        }
    }

    /// Pipes stdout from the last child in the pipeline to the given function, which runs in
    /// the current thread, then waits for all of the children to exit.
    ///
    /// If the function returns early, without reading from stdout until the last child exits,
    /// then the rest of stdout is automatically read and discarded to allow the child to finish.
    pub fn wait_with_pipe(&mut self, f: &mut dyn FnMut(&mut Box<dyn Read>)) -> CmdResult {
        let mut last_child = self.children.pop().unwrap();
        let mut stderr_thread = StderrThread::new(
            &last_child.cmd,
            &last_child.file,
            last_child.line,
            last_child.stderr.take(),
            false,
        );
        let last_child_res = if let Some(stdout) = last_child.stdout {
            let mut stdout: Box<dyn Read> = Box::new(stdout);
            f(&mut stdout);
            // The provided function may have left some of stdout unread.
            // Continue reading stdout on its behalf, until the child exits.
            let mut buf = vec![0; 65536];
            let outcome: Box<dyn ChildOutcome> = loop {
                match last_child.handle {
                    CmdChildHandle::Proc(ref mut child) => {
                        if let Some(result) = child.try_wait().transpose() {
                            break Box::new(ProcWaitOutcome::from(result));
                        }
                    }
                    CmdChildHandle::Thread(ref mut join_handle) => {
                        if let Some(handle) = join_handle.take() {
                            if handle.is_finished() {
                                break Box::new(ThreadJoinOutcome::from(handle.join()));
                            } else {
                                join_handle.replace(handle);
                            }
                        }
                    }
                    CmdChildHandle::SyncFn => {
                        break Box::new(SyncFnOutcome);
                    }
                }
                let _ = stdout.read(&mut buf);
            };
            outcome.to_io_result(&last_child.cmd, &last_child.file, last_child.line)
        } else {
            last_child.wait(true)
        };
        let other_children_res = CmdChildren::wait_children(&mut self.children);
        let _ = stderr_thread.join();

        if self.ignore_error {
            Ok(())
        } else {
            last_child_res.and(other_children_res)
        }
    }

    /// Returns the OS-assigned process identifiers associated with these children processes.
    pub fn pids(&self) -> Vec<u32> {
        self.children.iter().filter_map(|x| x.pid()).collect()
    }

    fn inner_wait_with_all(&mut self, capture_stderr: bool) -> (CmdResult, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = String::new();

        let last_child = self.children.pop().unwrap();
        let last_child_res = last_child.wait_with_all(capture_stderr, &mut stdout, &mut stderr);
        let other_children_res = CmdChildren::wait_children(&mut self.children);
        let cmd_result = if self.ignore_error {
            Ok(())
        } else {
            last_child_res.and(other_children_res)
        };

        let mut stdout: String = String::from_utf8_lossy(&stdout).into();
        if stdout.ends_with('\n') {
            stdout.pop();
        }

        (cmd_result, stdout, stderr)
    }
}

pub(crate) struct CmdChild {
    handle: CmdChildHandle,
    cmd: String,
    file: String,
    line: u32,
    stdout: Option<PipeReader>,
    stderr: Option<PipeReader>,
}

impl CmdChild {
    pub(crate) fn new(
        handle: CmdChildHandle,
        cmd: String,
        file: String,
        line: u32,
        stdout: Option<PipeReader>,
        stderr: Option<PipeReader>,
    ) -> Self {
        Self {
            file,
            line,
            handle,
            cmd,
            stdout,
            stderr,
        }
    }

    fn wait(mut self, is_last: bool) -> CmdResult {
        let _stderr_thread =
            StderrThread::new(&self.cmd, &self.file, self.line, self.stderr.take(), false);
        let res = self.handle.wait(&self.cmd, &self.file, self.line);
        if let Err(e) = res
            && (is_last || process::pipefail_enabled())
        {
            return Err(e);
        }
        Ok(())
    }

    fn wait_with_raw_output(self, ignore_error: bool, stdout_buf: &mut Vec<u8>) -> CmdResult {
        let mut _stderr = String::new();
        let res = self.wait_with_all(false, stdout_buf, &mut _stderr);
        if ignore_error {
            return Ok(());
        }
        res
    }

    fn wait_with_all(
        mut self,
        capture_stderr: bool,
        stdout_buf: &mut Vec<u8>,
        stderr_buf: &mut String,
    ) -> CmdResult {
        let mut stderr_thread = StderrThread::new(
            &self.cmd,
            &self.file,
            self.line,
            self.stderr.take(),
            capture_stderr,
        );
        let mut stdout_res = Ok(());
        if let Some(mut stdout) = self.stdout.take()
            && let Err(e) = stdout.read_to_end(stdout_buf)
        {
            stdout_res = Err(e)
        }
        *stderr_buf = stderr_thread.join();
        let wait_res = self.handle.wait(&self.cmd, &self.file, self.line);
        wait_res.and(stdout_res)
    }

    fn kill(self) -> CmdResult {
        self.handle.kill(&self.cmd, &self.file, self.line)
    }

    fn pid(&self) -> Option<u32> {
        self.handle.pid()
    }
}

pub(crate) enum CmdChildHandle {
    Proc(Child),
    Thread(Option<JoinHandle<CmdResult>>),
    SyncFn,
}

#[derive(Debug)]
struct ProcWaitOutcome(std::io::Result<ExitStatus>);
impl From<std::io::Result<ExitStatus>> for ProcWaitOutcome {
    fn from(result: std::io::Result<ExitStatus>) -> Self {
        Self(result)
    }
}
impl Display for ProcWaitOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Ok(status) => {
                if status.success() {
                    write!(f, "Command process succeeded")
                } else if let Some(code) = status.code() {
                    write!(f, "Command process exited normally with status code {code}")
                } else {
                    write!(f, "Command process exited abnormally: {status}")
                }
            }
            Err(error) => write!(f, "Failed to wait for command process: {error:?}"),
        }
    }
}
#[derive(Debug)]
enum ThreadJoinOutcome {
    Ok,
    Err(std::io::Error),
    Panic(Box<dyn Any + Send + 'static>),
}
impl From<std::thread::Result<CmdResult>> for ThreadJoinOutcome {
    fn from(result: std::thread::Result<CmdResult>) -> Self {
        match result {
            Ok(Ok(())) => Self::Ok,
            Ok(Err(error)) => Self::Err(error),
            Err(panic) => Self::Panic(panic),
        }
    }
}
impl Display for ThreadJoinOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "Command thread succeeded"),
            Self::Err(error) => write!(f, "Command thread returned error: {error:?}"),
            Self::Panic(panic) => write!(f, "Command thread panicked: {panic:?}"),
        }
    }
}
#[derive(Debug)]
struct SyncFnOutcome;
impl Display for SyncFnOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command finished")
    }
}
trait ChildOutcome: Display {
    fn success(&self) -> bool;
    fn to_io_result(&self, cmd: &str, file: &str, line: u32) -> std::io::Result<()> {
        if self.success() {
            Ok(())
        } else {
            Err(Error::other(format!(
                "Running [{cmd}] exited with error; {self} at {file}:{line}"
            )))
        }
    }
}
impl ChildOutcome for ProcWaitOutcome {
    fn success(&self) -> bool {
        self.0.as_ref().is_ok_and(|status| status.success())
    }
}
impl ChildOutcome for ThreadJoinOutcome {
    fn success(&self) -> bool {
        matches!(self, Self::Ok)
    }
}
impl ChildOutcome for SyncFnOutcome {
    fn success(&self) -> bool {
        true
    }
}

impl CmdChildHandle {
    fn wait(self, cmd: &str, file: &str, line: u32) -> CmdResult {
        let outcome: Box<dyn ChildOutcome> = match self {
            CmdChildHandle::Proc(mut proc) => Box::new(ProcWaitOutcome::from(proc.wait())),
            CmdChildHandle::Thread(mut thread) => {
                if let Some(thread) = thread.take() {
                    Box::new(ThreadJoinOutcome::from(thread.join()))
                } else {
                    unreachable!()
                }
            }
            CmdChildHandle::SyncFn => return Ok(()),
        };
        outcome.to_io_result(cmd, file, line)
    }

    fn kill(self, cmd: &str, file: &str, line: u32) -> CmdResult {
        match self {
            CmdChildHandle::Proc(mut proc) => proc.kill().map_err(|e| {
                Error::new(
                    e.kind(),
                    format!("Killing process [{cmd}] failed with error: {e} at {file}:{line}"),
                )
            }),
            CmdChildHandle::Thread(_thread) => Err(Error::other(format!(
                "Killing thread [{cmd}] failed: not supported at {file}:{line}"
            ))),
            CmdChildHandle::SyncFn => Ok(()),
        }
    }

    fn pid(&self) -> Option<u32> {
        match self {
            CmdChildHandle::Proc(proc) => Some(proc.id()),
            _ => None,
        }
    }
}

struct StderrThread {
    thread: Option<JoinHandle<String>>,
    cmd: String,
    file: String,
    line: u32,
}

impl StderrThread {
    fn new(cmd: &str, file: &str, line: u32, stderr: Option<PipeReader>, capture: bool) -> Self {
        #[cfg(feature = "tracing")]
        let span = tracing::Span::current();
        if let Some(stderr) = stderr {
            let file_ = file.to_owned();
            let thread = std::thread::spawn(move || {
                #[cfg(feature = "tracing")]
                let _entered = span.enter();
                if capture {
                    let mut output = String::new();
                    BufReader::new(stderr)
                        .lines()
                        .map_while(Result::ok)
                        .for_each(|line| {
                            if !output.is_empty() {
                                output.push('\n');
                            }
                            output.push_str(&line);
                        });
                    return output;
                }

                // Log output one line at a time, including progress output separated by CR
                let mut reader = BufReader::new(stderr);
                let mut buffer = vec![];
                loop {
                    // Unconditionally try to read more data, since the BufReader buffer is empty
                    let result = match reader.fill_buf() {
                        Ok(buffer) => buffer,
                        Err(error) => {
                            warn!("Error reading from child process: {error:?} at {file_}:{line}");
                            break;
                        }
                    };
                    // Add the result onto our own buffer
                    buffer.extend(result);
                    // Empty the BufReader
                    let read_len = result.len();
                    reader.consume(read_len);

                    // Log output. Take whole “lines” at every LF or CR (for progress bars etc),
                    // but leave any incomplete lines in our buffer so we can try to complete them.
                    while let Some(offset) = buffer.iter().position(|&b| b == b'\n' || b == b'\r') {
                        let line = &buffer[..offset];
                        let line = str::from_utf8(line).map_err(|_| line);
                        match line {
                            Ok(string) => info!("{string}"),
                            Err(bytes) => info!("{bytes:?}"),
                        }
                        buffer = buffer.split_off(offset + 1);
                    }

                    if read_len == 0 {
                        break;
                    }
                }

                // Log any remaining incomplete line
                if !buffer.is_empty() {
                    let line = &buffer;
                    let line = str::from_utf8(line).map_err(|_| line);
                    match line {
                        Ok(string) => info!("{string}"),
                        Err(bytes) => info!("{bytes:?}"),
                    }
                }

                "".to_owned()
            });
            Self {
                cmd: cmd.into(),
                file: file.into(),
                line,
                thread: Some(thread),
            }
        } else {
            Self {
                cmd: cmd.into(),
                file: file.into(),
                line,
                thread: None,
            }
        }
    }

    fn join(&mut self) -> String {
        if let Some(thread) = self.thread.take() {
            match thread.join() {
                Err(e) => {
                    warn!(
                        "Running [{}] stderr thread joined with error: {:?} at {}:{}",
                        self.cmd, e, self.file, self.line
                    );
                }
                Ok(output) => return output,
            }
        }
        "".into()
    }
}

impl Drop for StderrThread {
    fn drop(&mut self) {
        self.join();
    }
}
