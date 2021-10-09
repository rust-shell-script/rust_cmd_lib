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
    ignore_error: bool,
}

impl CmdChildren {
    pub(crate) fn from(children: Vec<CmdChild>, ignore_error: bool) -> Self {
        Self {
            children,
            ignore_error,
        }
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
        let mut ret = Ok(());
        let polling_stderr = child.stderr.map(CmdChildHandle::log_stderr_output);
        match child.handle {
            CmdChildHandle::Proc(proc) => match proc {
                Err(e) => ret = Err(CmdChildHandle::cmd_io_error(e, &child.cmd, true)),
                Ok(mut proc) => {
                    if let Some(stdout) = child.stdout {
                        f(Box::new(stdout));
                        let _ = proc.kill();
                    }
                }
            },
            CmdChildHandle::Thread(thread) => match thread {
                Err(e) => ret = Err(CmdChildHandle::cmd_io_error(e, &child.cmd, true)),
                Ok(_) => {
                    if let Some(stdout) = child.stdout {
                        f(Box::new(stdout));
                    }
                }
            },
            CmdChildHandle::SyncFn(sync_fn) => match sync_fn {
                Err(e) => ret = Err(CmdChildHandle::cmd_io_error(e, &child.cmd, true)),
                Ok(_) => {
                    if let Some(stdout) = child.stdout {
                        f(Box::new(stdout));
                    }
                }
            },
        };
        CmdChildHandle::wait_logging_thread(&child.cmd, polling_stderr);
        Self::wait_children(&mut self.children)?;
        ret
    }
}

#[derive(Debug)]
pub(crate) struct CmdChild {
    pub(crate) handle: CmdChildHandle,
    pub(crate) cmd: String,
    pub(crate) stdout: Option<PipeReader>,
    pub(crate) stderr: Option<PipeReader>,
}

impl CmdChild {
    fn wait(self, is_last: bool) -> CmdResult {
        let ret = self.handle.wait_with_stderr(self.stderr, &self.cmd);
        if let Err(e) = ret {
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
        let ret = self.handle.wait_with_stderr(self.stderr, &self.cmd);
        if let Err(e) = ret {
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
        let mut ret = Ok(());
        let polling_stderr = stderr.map(CmdChildHandle::log_stderr_output);
        match self {
            CmdChildHandle::Proc(proc) => match proc {
                Err(e) => ret = Err(CmdChildHandle::cmd_io_error(e, cmd, true)),
                Ok(mut proc) => {
                    let status = proc.wait();
                    match status {
                        Err(e) => ret = Err(CmdChildHandle::cmd_io_error(e, cmd, false)),
                        Ok(status) => {
                            if !status.success() {
                                ret = Err(Self::status_to_io_error(
                                    status,
                                    &format!("Running {} exited with error", cmd),
                                ));
                            }
                        }
                    }
                }
            },
            CmdChildHandle::Thread(thread) => match thread {
                Err(e) => ret = Err(CmdChildHandle::cmd_io_error(e, cmd, true)),
                Ok(thread) => {
                    let status = thread.join();
                    match status {
                        Ok(result) => {
                            if let Err(e) = result {
                                ret = Err(CmdChildHandle::cmd_io_error(e, cmd, false));
                            }
                        }
                        Err(e) => {
                            ret = Err(Error::new(
                                ErrorKind::Other,
                                format!("Running {} thread joined with error: {:?}", cmd, e),
                            ))
                        }
                    }
                }
            },
            CmdChildHandle::SyncFn(sync_fn) => {
                if let Err(e) = sync_fn {
                    ret = Err(CmdChildHandle::cmd_io_error(e, cmd, false));
                }
            }
        }
        Self::wait_logging_thread(cmd, polling_stderr);
        ret
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
