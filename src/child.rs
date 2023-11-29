use crate::{info, warn};
use crate::{process, CmdResult, FunResult};
use os_pipe::PipeReader;
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Result};
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
        // wait for the last child result
        let handle = self.children.pop().unwrap();
        if let Err(e) = handle.wait(true) {
            let _ = Self::wait_children(&mut self.children);
            return Err(e);
        }
        Self::wait_children(&mut self.children)
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
        if let Err(e) = res {
            if !self.ignore_error {
                return Err(e);
            }
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
                if self.ignore_error {
                    Ok(())
                } else {
                    ret
                }
            }
        }
    }

    /// Waits for the children processes to exit completely, pipe content will be processed by
    /// provided function.
    pub fn wait_with_pipe(&mut self, f: &mut dyn FnMut(Box<dyn Read>)) -> CmdResult {
        let child = self.children.pop().unwrap();
        let stderr_thread =
            StderrThread::new(&child.cmd, &child.file, child.line, child.stderr, false);
        match child.handle {
            CmdChildHandle::Proc(mut proc) => {
                if let Some(stdout) = child.stdout {
                    f(Box::new(stdout));
                    let _ = proc.kill();
                }
            }
            CmdChildHandle::Thread(_) => {
                if let Some(stdout) = child.stdout {
                    f(Box::new(stdout));
                }
            }
            CmdChildHandle::SyncFn => {
                if let Some(stdout) = child.stdout {
                    f(Box::new(stdout));
                }
            }
        };
        drop(stderr_thread);
        CmdChildren::wait_children(&mut self.children)
    }

    /// Returns the OS-assigned process identifiers associated with these children processes.
    pub fn pids(&self) -> Vec<u32> {
        self.children.iter().filter_map(|x| x.pid()).collect()
    }

    fn inner_wait_with_all(&mut self, capture_stderr: bool) -> (CmdResult, String, String) {
        // wait for the last child result
        let handle = self.children.pop().unwrap();
        let mut stdout_buf = Vec::new();
        let mut stderr = String::new();
        let res = handle.wait_with_all(capture_stderr, &mut stdout_buf, &mut stderr);
        let _ = CmdChildren::wait_children(&mut self.children);
        let mut stdout: String = String::from_utf8_lossy(&stdout_buf).into();
        if stdout.ends_with('\n') {
            stdout.pop();
        }
        (res, stdout, stderr)
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
        if let Err(e) = res {
            if is_last || process::pipefail_enabled() {
                return Err(e);
            }
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
        if let Some(mut stdout) = self.stdout.take() {
            if let Err(e) = stdout.read_to_end(stdout_buf) {
                stdout_res = Err(e)
            }
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
    Thread(JoinHandle<CmdResult>),
    SyncFn,
}

impl CmdChildHandle {
    fn wait(self, cmd: &str, file: &str, line: u32) -> CmdResult {
        match self {
            CmdChildHandle::Proc(mut proc) => {
                let status = proc.wait();
                match status {
                    Err(e) => return Err(process::new_cmd_io_error(&e, cmd, file, line)),
                    Ok(status) => {
                        if !status.success() {
                            return Err(Self::status_to_io_error(status, cmd, file, line));
                        }
                    }
                }
            }
            CmdChildHandle::Thread(thread) => {
                let status = thread.join();
                match status {
                    Ok(result) => {
                        if let Err(e) = result {
                            return Err(process::new_cmd_io_error(&e, cmd, file, line));
                        }
                    }
                    Err(e) => {
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!(
                                "Running [{cmd}] thread joined with error: {e:?} at {file}:{line}"
                            ),
                        ))
                    }
                }
            }
            CmdChildHandle::SyncFn => {}
        }
        Ok(())
    }

    fn status_to_io_error(status: ExitStatus, cmd: &str, file: &str, line: u32) -> Error {
        if let Some(code) = status.code() {
            Error::new(
                ErrorKind::Other,
                format!("Running [{cmd}] exited with error; status code: {code} at {file}:{line}"),
            )
        } else {
            Error::new(
                ErrorKind::Other,
                format!(
                    "Running [{cmd}] exited with error; terminated by {status} at {file}:{line}"
                ),
            )
        }
    }

    fn kill(self, cmd: &str, file: &str, line: u32) -> CmdResult {
        match self {
            CmdChildHandle::Proc(mut proc) => proc.kill().map_err(|e| {
                Error::new(
                    e.kind(),
                    format!("Killing process [{cmd}] failed with error: {e} at {file}:{line}"),
                )
            }),
            CmdChildHandle::Thread(_thread) => Err(Error::new(
                ErrorKind::Other,
                format!("Killing thread [{cmd}] failed: not supported at {file}:{line}"),
            )),
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
        if let Some(stderr) = stderr {
            let thread = std::thread::spawn(move || {
                let mut output = String::new();
                BufReader::new(stderr)
                    .lines()
                    .map_while(Result::ok)
                    .for_each(|line| {
                        if !capture {
                            info!("{line}");
                        } else {
                            if !output.is_empty() {
                                output.push('\n');
                            }
                            output.push_str(&line);
                        }
                    });
                output
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
