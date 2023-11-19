use crate::info;
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
    /// Waits for the children processes to exit completely, returning the output.
    pub fn wait_with_output(&mut self) -> FunResult {
        // wait for the last child result
        let handle = self.children.pop().unwrap();
        let wait_last = handle.wait_with_output(self.ignore_error);
        match wait_last {
            Err(e) => {
                let _ = CmdChildren::wait_children(&mut self.children);
                Err(e)
            }
            Ok(output) => {
                let ret = CmdChildren::wait_children(&mut self.children);
                if let Err(e) = ret {
                    if !self.ignore_error {
                        return Err(e);
                    }
                }
                Ok(output)
            }
        }
    }

    /// Waits for the children processes to exit completely, pipe content will be processed by
    /// provided function.
    pub fn wait_with_pipe(&mut self, f: &mut dyn FnMut(Box<dyn Read>)) -> CmdResult {
        let child = self.children.pop().unwrap();
        let stderr_thread = StderrThread::new(&child.cmd, child.stderr, false);
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

    /// Waits for the children processes to exit completely, returning the command result, stdout
    /// read result and stderr read result.
    pub fn wait_with_all(&mut self) -> (CmdResult, FunResult, FunResult) {
        // wait for the last child result
        let handle = self.children.pop().unwrap();
        let wait_all = handle.wait_with_all(true);
        let _ = CmdChildren::wait_children(&mut self.children);
        wait_all
    }

    /// Returns the OS-assigned process identifiers associated with these children processes
    pub fn pids(&self) -> Vec<u32> {
        self.children.iter().filter_map(|x| x.pid()).collect()
    }
}

pub(crate) struct CmdChild {
    handle: CmdChildHandle,
    cmd: String,
    stdout: Option<PipeReader>,
    stderr: Option<PipeReader>,
}

impl CmdChild {
    pub(crate) fn new(
        handle: CmdChildHandle,
        cmd: String,
        stdout: Option<PipeReader>,
        stderr: Option<PipeReader>,
    ) -> Self {
        Self {
            handle,
            cmd,
            stdout,
            stderr,
        }
    }

    fn wait(mut self, is_last: bool) -> CmdResult {
        let _stderr_thread = StderrThread::new(&self.cmd, self.stderr.take(), false);
        let res = self.handle.wait(&self.cmd);
        if let Err(e) = res {
            if is_last || process::pipefail_enabled() {
                return Err(e);
            }
        }
        Ok(())
    }

    fn wait_with_output(self, ignore_error: bool) -> FunResult {
        let (res, stdout, _) = self.wait_with_all(false);
        if !ignore_error {
            res?;
        }
        stdout
    }

    fn wait_with_all(mut self, capture: bool) -> (CmdResult, FunResult, FunResult) {
        let mut stderr_thread = StderrThread::new(&self.cmd, self.stderr.take(), capture);
        let stdout_output = {
            if let Some(mut out) = self.stdout.take() {
                let mut s = String::new();
                match out.read_to_string(&mut s) {
                    Err(e) => Err(e),
                    Ok(_) => {
                        if s.ends_with('\n') {
                            s.pop();
                        }
                        Ok(s)
                    }
                }
            } else {
                Ok("".into())
            }
        };
        let stderr_output = stderr_thread.join();
        let res = self.handle.wait(&self.cmd);
        (res, stdout_output, stderr_output)
    }

    fn kill(self) -> CmdResult {
        self.handle.kill()
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
    fn wait(self, cmd: &str) -> CmdResult {
        match self {
            CmdChildHandle::Proc(mut proc) => {
                let status = proc.wait();
                match status {
                    Err(e) => return Err(process::new_cmd_io_error(&e, cmd)),
                    Ok(status) => {
                        if !status.success() {
                            return Err(Self::status_to_io_error(
                                status,
                                &format!("Running [{cmd}] exited with error"),
                            ));
                        }
                    }
                }
            }
            CmdChildHandle::Thread(thread) => {
                let status = thread.join();
                match status {
                    Ok(result) => {
                        if let Err(e) = result {
                            return Err(process::new_cmd_io_error(&e, cmd));
                        }
                    }
                    Err(e) => {
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!("Running [{cmd}] thread joined with error: {e:?}"),
                        ))
                    }
                }
            }
            CmdChildHandle::SyncFn => {}
        }
        Ok(())
    }

    fn status_to_io_error(status: ExitStatus, cmd: &str) -> Error {
        if let Some(code) = status.code() {
            Error::new(ErrorKind::Other, format!("{cmd}; status code: {code}"))
        } else {
            Error::new(ErrorKind::Other, format!("{cmd}; terminated by {status}"))
        }
    }

    fn kill(self) -> CmdResult {
        match self {
            CmdChildHandle::Proc(mut proc) => proc.kill(),
            CmdChildHandle::Thread(_thread) => {
                panic!("thread killing not suppported!")
            }
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
}

impl StderrThread {
    fn new(cmd: &str, stderr: Option<PipeReader>, capture: bool) -> Self {
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
                            output.push_str(&line);
                            output.push('\n');
                        }
                    });
                if output.ends_with('\n') {
                    output.pop();
                }
                output
            });
            Self {
                cmd: cmd.into(),
                thread: Some(thread),
            }
        } else {
            Self {
                cmd: cmd.into(),
                thread: None,
            }
        }
    }

    fn join(&mut self) -> FunResult {
        if let Some(thread) = self.thread.take() {
            match thread.join() {
                Err(e) => {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!(
                            "Running [{}] stderr thread joined with error: {e:?}",
                            self.cmd
                        ),
                    ))
                }
                Ok(output) => return Ok(output),
            }
        }
        Ok("".into())
    }
}

impl Drop for StderrThread {
    fn drop(&mut self) {
        let _ = self.join();
    }
}
