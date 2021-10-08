use crate::{process, CmdResult, FunResult};
use log::{info, warn};
use os_pipe::PipeReader;
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Result};
use std::process::{Child, ExitStatus};
use std::thread::JoinHandle;

/// Representation of running or exited children processes, connected with pipes
/// optionally.
///
/// Calling `spawn!` or `spawn_with_output!` macro will return `Result<CmdChildren>`
pub struct CmdChildren {
    children: Vec<CmdChild>,
}

impl CmdChildren {
    pub(crate) fn from(children: Vec<CmdChild>) -> Self {
        Self { children }
    }

    pub fn wait_cmd_result(&mut self) -> CmdResult {
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
        while !children.is_empty() {
            let child_handle = children.pop().unwrap();
            if let Err(e) = child_handle.wait(false) {
                ret = Err(e);
            }
        }
        ret
    }

    pub fn wait_fun_result(&mut self) -> FunResult {
        // wait for the last child result
        let handle = self.children.pop().unwrap();
        let wait_last = handle.wait_with_output();
        match wait_last {
            Err(e) => {
                let _ = CmdChildren::wait_children(&mut self.children);
                Err(e)
            }
            Ok(output) => {
                let mut ret = String::from_utf8_lossy(&output).to_string();
                if ret.ends_with('\n') {
                    ret.pop();
                }
                CmdChildren::wait_children(&mut self.children)?;
                Ok(ret)
            }
        }
    }

    pub fn wait_with_pipe(&mut self, f: &mut dyn FnMut(Box<dyn Read>)) -> CmdResult {
        let child = self.children.pop().unwrap();
        match child.handle {
            CmdChildHandle::Proc(proc) => {
                let polling_stderr = child.stderr.map(CmdChild::log_stderr_output);
                if let Some(stdout) = child.stdout {
                    f(Box::new(stdout));
                    let _ = proc?.kill();
                }
                CmdChild::wait_logging_thread(&child.cmd, polling_stderr);
            }
            CmdChildHandle::Thread(thread) => {
                let _ = thread?;
                let polling_stderr = child.stderr.map(CmdChild::log_stderr_output);
                if let Some(stdout) = child.stdout {
                    f(Box::new(stdout));
                }
                CmdChild::wait_logging_thread(&child.cmd, polling_stderr);
            }
            CmdChildHandle::SyncFn(sync_fn) => {
                let _ = sync_fn?;
                let polling_stderr = child.stderr.map(CmdChild::log_stderr_output);
                if let Some(stdout) = child.stdout {
                    f(Box::new(stdout));
                }
                CmdChild::wait_logging_thread(&child.cmd, polling_stderr);
            }
        };
        Self::wait_children(&mut self.children)
    }
}

#[derive(Debug)]
pub(crate) struct CmdChild {
    pub(crate) handle: CmdChildHandle,
    pub(crate) cmd: String,
    pub(crate) stdout: Option<PipeReader>,
    pub(crate) stderr: Option<PipeReader>,
}

#[derive(Debug)]
pub(crate) enum CmdChildHandle {
    Proc(Result<Child>),
    Thread(Result<JoinHandle<CmdResult>>),
    SyncFn(CmdResult),
}

impl CmdChild {
    fn wait(self, is_last: bool) -> CmdResult {
        let polling_stderr = self.stderr.map(CmdChild::log_stderr_output);
        match self.handle {
            CmdChildHandle::Proc(proc) => {
                let status = proc?.wait()?;
                Self::wait_logging_thread(&self.cmd, polling_stderr);
                if !status.success() && (is_last || process::pipefail_enabled()) {
                    return Err(Self::status_to_io_error(
                        status,
                        &format!("{} exited with error", self.cmd),
                    ));
                }
            }
            CmdChildHandle::Thread(thread) => {
                let status = thread?.join();
                Self::wait_logging_thread(&self.cmd, polling_stderr);
                match status {
                    Err(e) => {
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!("{} thread joined with error: {:?}", self.cmd, e),
                        ));
                    }
                    Ok(result) => {
                        if let Err(e) = result {
                            if is_last || process::pipefail_enabled() {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            CmdChildHandle::SyncFn(sync_fn) => {
                let _ = sync_fn?;
                Self::wait_logging_thread(&self.cmd, polling_stderr);
            }
        }
        Ok(())
    }

    fn wait_with_output(self) -> Result<Vec<u8>> {
        let read_to_buf = |stdout: Option<PipeReader>| -> Result<Vec<u8>> {
            if let Some(mut out) = stdout {
                let mut buf = vec![];
                out.read_to_end(&mut buf)?;
                Ok(buf)
            } else {
                Ok(vec![])
            }
        };
        let polling_stderr = self.stderr.map(CmdChild::log_stderr_output);
        match self.handle {
            CmdChildHandle::Proc(proc) => {
                let buf = read_to_buf(self.stdout)?;
                let status = proc?.wait()?;
                Self::wait_logging_thread(&self.cmd, polling_stderr);
                if !status.success() {
                    return Err(Self::status_to_io_error(
                        status,
                        &format!("{} exited with error", self.cmd),
                    ));
                }
                Ok(buf)
            }
            CmdChildHandle::Thread(thread) => {
                let buf = read_to_buf(self.stdout)?;
                let status = thread?.join();
                Self::wait_logging_thread(&self.cmd, polling_stderr);
                match status {
                    Err(e) => {
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!("{} thread joined with error: {:?}", self.cmd, e),
                        ));
                    }
                    Ok(result) => {
                        if let Err(e) = result {
                            return Err(e);
                        }
                    }
                }
                Ok(buf)
            }
            CmdChildHandle::SyncFn(sync_fn) => {
                let _ = sync_fn?;
                Self::wait_logging_thread(&self.cmd, polling_stderr);
                if let Some(mut out) = self.stdout {
                    let mut buf = vec![];
                    out.read_to_end(&mut buf)?;
                    return Ok(buf);
                }
                Ok(vec![])
            }
        }
    }

    fn log_stderr_output(stderr: PipeReader) -> JoinHandle<()> {
        std::thread::spawn(move || {
            BufReader::new(stderr)
                .lines()
                .filter_map(|line| line.ok())
                .for_each(|line| info!("{}", line))
        })
    }

    fn wait_logging_thread(cmd: &str, thread: Option<JoinHandle<()>>) {
        if let Some(thread) = thread {
            if let Err(e) = thread.join() {
                warn!("{} logging thread exited with error: {:?}", cmd, e);
            }
        }
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
