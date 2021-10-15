use crate::{process, CmdResult, FunResult};
use log::{info, warn};
use os_pipe::PipeReader;
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Result};
use std::process::{Child, ExitStatus};
use std::thread::JoinHandle;

/// Representation of running or exited children processes, connected with pipes
/// optionally.
///
/// Calling `spawn!` macro will return `Result<CmdChildren>`
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
        while !children.is_empty() {
            let child_handle = children.pop().unwrap();
            if let Err(e) = child_handle.wait(false) {
                ret = Err(e);
            }
        }
        ret
    }
}

/// Representation of running or exited children processes with output, connected with pipes
/// optionally.
///
/// Calling `spawn_with_output!` macro will return `Result<FunChildren>`
pub struct FunChildren {
    children: Vec<CmdChild>,
    ignore_error: bool,
}

impl FunChildren {
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
                let mut s = String::from_utf8_lossy(&output).to_string();
                if s.ends_with('\n') {
                    s.pop();
                }
                let ret = CmdChildren::wait_children(&mut self.children);
                if let Err(e) = ret {
                    if !self.ignore_error {
                        return Err(e);
                    }
                }
                Ok(s)
            }
        }
    }

    pub fn wait_with_pipe(&mut self, f: &mut dyn FnMut(Box<dyn Read>)) -> CmdResult {
        let child = self.children.pop().unwrap();
        let polling_stderr = StderrLogging::new(&child.cmd, child.stderr);
        match child.handle {
            CmdChildHandle::Proc(proc) => match proc {
                Err(e) => return Err(CmdChildHandle::cmd_io_error(e, &child.cmd, true)),
                Ok(mut proc) => {
                    if let Some(stdout) = child.stdout {
                        f(Box::new(stdout));
                        let _ = proc.kill();
                    }
                }
            },
            CmdChildHandle::Thread(thread) => match thread {
                Err(e) => return Err(CmdChildHandle::cmd_io_error(e, &child.cmd, true)),
                Ok(_) => {
                    if let Some(stdout) = child.stdout {
                        f(Box::new(stdout));
                    }
                }
            },
            CmdChildHandle::SyncFn(sync_fn) => match sync_fn {
                Err(e) => return Err(CmdChildHandle::cmd_io_error(e, &child.cmd, true)),
                Ok(_) => {
                    if let Some(stdout) = child.stdout {
                        f(Box::new(stdout));
                    }
                }
            },
        };
        drop(polling_stderr);
        CmdChildren::wait_children(&mut self.children)
    }
}

#[derive(Debug)]
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

    fn wait(self, is_last: bool) -> CmdResult {
        let res = self.handle.wait_with_stderr(self.stderr, &self.cmd);
        if let Err(e) = res {
            if is_last || process::pipefail_enabled() {
                return Err(e);
            }
        }
        Ok(())
    }

    fn wait_with_output(self, ignore_error: bool) -> Result<Vec<u8>> {
        let buf = {
            if let Some(mut out) = self.stdout {
                let mut buf = vec![];
                if let Err(e) = out.read_to_end(&mut buf) {
                    if !ignore_error {
                        return Err(CmdChildHandle::cmd_io_error(e, &self.cmd, false));
                    }
                }
                buf
            } else {
                vec![]
            }
        };
        let res = self.handle.wait_with_stderr(self.stderr, &self.cmd);
        if let Err(e) = res {
            if !ignore_error {
                return Err(e);
            }
        }
        Ok(buf)
    }
}

#[derive(Debug)]
pub(crate) enum CmdChildHandle {
    Proc(Result<Child>),
    Thread(Result<JoinHandle<CmdResult>>),
    SyncFn(CmdResult),
}

impl CmdChildHandle {
    fn wait_with_stderr(self, stderr: Option<PipeReader>, cmd: &str) -> CmdResult {
        let polling_stderr = StderrLogging::new(cmd, stderr);
        match self {
            CmdChildHandle::Proc(proc) => match proc {
                Err(e) => return Err(CmdChildHandle::cmd_io_error(e, cmd, true)),
                Ok(mut proc) => {
                    let status = proc.wait();
                    match status {
                        Err(e) => return Err(CmdChildHandle::cmd_io_error(e, cmd, false)),
                        Ok(status) => {
                            if !status.success() {
                                return Err(Self::status_to_io_error(
                                    status,
                                    &format!("Running {} exited with error", cmd),
                                ));
                            }
                        }
                    }
                }
            },
            CmdChildHandle::Thread(thread) => match thread {
                Err(e) => return Err(CmdChildHandle::cmd_io_error(e, cmd, true)),
                Ok(thread) => {
                    let status = thread.join();
                    match status {
                        Ok(result) => {
                            if let Err(e) = result {
                                return Err(CmdChildHandle::cmd_io_error(e, cmd, false));
                            }
                        }
                        Err(e) => {
                            return Err(Error::new(
                                ErrorKind::Other,
                                format!("Running {} thread joined with error: {:?}", cmd, e),
                            ))
                        }
                    }
                }
            },
            CmdChildHandle::SyncFn(sync_fn) => {
                if let Err(e) = sync_fn {
                    return Err(CmdChildHandle::cmd_io_error(e, cmd, false));
                }
            }
        }
        drop(polling_stderr);
        Ok(())
    }

    fn cmd_io_error(e: Error, command: &str, spawning: bool) -> Error {
        Error::new(
            e.kind(),
            format!(
                "{} {} failed: {}",
                if spawning { "Spawning" } else { "Running" },
                command,
                e
            ),
        )
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

struct StderrLogging {
    thread: Option<JoinHandle<()>>,
    cmd: String,
}

impl StderrLogging {
    fn new(cmd: &str, stderr: Option<PipeReader>) -> Self {
        if let Some(stderr) = stderr {
            let thread = std::thread::spawn(move || {
                BufReader::new(stderr)
                    .lines()
                    .filter_map(|line| line.ok())
                    .for_each(|line| info!("{}", line))
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
}

impl Drop for StderrLogging {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            if let Err(e) = thread.join() {
                warn!("{} logging thread exited with error: {:?}", self.cmd, e);
            }
        }
    }
}
