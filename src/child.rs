use crate::{process, CmdResult, FunResult};
use log::{error, info, warn};
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
        let ret = self.wait_cmd_result_nolog();
        if let Err(ref err) = ret {
            if self.ignore_error {
                return Ok(());
            } else {
                error!(
                    "Running {} failed, Error: {}",
                    CmdChild::get_full_cmd(&self.children),
                    err
                );
            }
        }
        ret
    }

    pub(crate) fn wait_cmd_result_nolog(&mut self) -> CmdResult {
        // wait last process result
        let handle = self.children.pop().unwrap();
        if let Err(e) = handle.wait(true, self.ignore_error) {
            if !self.ignore_error {
                let _ = Self::wait_children(&mut self.children, self.ignore_error);
                return Err(e);
            }
        }
        Self::wait_children(&mut self.children, self.ignore_error)
    }

    fn wait_children(children: &mut Vec<CmdChild>, ignore_error: bool) -> CmdResult {
        let mut ret = Ok(());
        while !children.is_empty() {
            let child_handle = children.pop().unwrap();
            if let Err(e) = child_handle.wait(false, ignore_error) {
                ret = Err(e);
            }
        }
        ret
    }

    pub fn wait_fun_result(&mut self) -> FunResult {
        let ret = self.wait_fun_result_nolog();
        if let Err(ref err) = ret {
            if self.ignore_error {
                return Ok("".into());
            } else {
                error!(
                    "Running {} failed, Error: {}",
                    CmdChild::get_full_cmd(&self.children),
                    err
                );
            }
        }
        ret
    }

    pub(crate) fn wait_fun_result_nolog(&mut self) -> FunResult {
        // wait last process result
        let handle = self.children.pop().unwrap();
        let wait_last = handle.wait_with_output(self.ignore_error);
        match wait_last {
            Err(e) => {
                let _ = CmdChildren::wait_children(&mut self.children, self.ignore_error);
                Err(e)
            }
            Ok(output) => {
                let mut ret = String::from_utf8_lossy(&output).to_string();
                if ret.ends_with('\n') {
                    ret.pop();
                }
                CmdChildren::wait_children(&mut self.children, self.ignore_error)?;
                Ok(ret)
            }
        }
    }

    pub fn wait_with_pipe(&mut self, f: &mut dyn FnMut(Box<dyn Read>)) {
        let handle = self.children.pop().unwrap();
        match handle {
            CmdChild::Proc {
                mut child,
                stdout,
                stderr,
                cmd,
                ..
            } => {
                let polling_stderr = stderr.map(CmdChild::log_stderr_output);
                if let Some(stdout) = stdout {
                    f(Box::new(stdout));
                    let _ = child.kill();
                }
                CmdChild::wait_logging_thread(&cmd, polling_stderr);
            }
            CmdChild::ThreadFn {
                stderr,
                stdout,
                cmd,
                ..
            } => {
                let polling_stderr = stderr.map(CmdChild::log_stderr_output);
                if let Some(stdout) = stdout {
                    f(Box::new(stdout));
                }
                CmdChild::wait_logging_thread(&cmd, polling_stderr);
            }
            CmdChild::SyncFn {
                stderr,
                stdout,
                cmd,
                ..
            } => {
                let polling_stderr = stderr.map(CmdChild::log_stderr_output);
                if let Some(stdout) = stdout {
                    f(Box::new(stdout));
                }
                CmdChild::wait_logging_thread(&cmd, polling_stderr);
            }
        };
        let _ = Self::wait_children(&mut self.children, self.ignore_error);
    }
}

#[derive(Debug)]
pub(crate) enum CmdChild {
    Proc {
        child: Child,
        cmd: String,
        stdout: Option<PipeReader>,
        stderr: Option<PipeReader>,
    },
    ThreadFn {
        child: JoinHandle<CmdResult>,
        cmd: String,
        stdout: Option<PipeReader>,
        stderr: Option<PipeReader>,
    },
    SyncFn {
        cmd: String,
        stdout: Option<PipeReader>,
        stderr: Option<PipeReader>,
    },
}

impl CmdChild {
    fn wait(self, is_last: bool, ignore_error: bool) -> CmdResult {
        match self {
            CmdChild::Proc {
                mut child,
                stderr,
                cmd,
                ..
            } => {
                let polling_stderr = stderr.map(CmdChild::log_stderr_output);
                let status = child.wait()?;
                Self::wait_logging_thread(&cmd, polling_stderr);
                if !status.success() {
                    if !ignore_error && (is_last || process::pipefail_enabled()) {
                        return Err(Self::status_to_io_error(
                            status,
                            &format!("{} exited with error", cmd),
                        ));
                    } else if process::debug_enabled() {
                        warn!("{} exited with error: {}", cmd, status);
                    }
                }
            }
            CmdChild::ThreadFn {
                child, cmd, stderr, ..
            } => {
                let polling_stderr = stderr.map(CmdChild::log_stderr_output);
                let status = child.join();
                Self::wait_logging_thread(&cmd, polling_stderr);
                match status {
                    Err(e) => {
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!("{} thread joined with error: {:?}", cmd, e),
                        ));
                    }
                    Ok(result) => {
                        if let Err(e) = result {
                            if !ignore_error && (is_last || process::pipefail_enabled()) {
                                return Err(e);
                            } else if process::debug_enabled() {
                                warn!("{} exited with error: {:?}", cmd, e);
                            }
                        }
                    }
                }
            }
            CmdChild::SyncFn { stderr, cmd, .. } => {
                Self::wait_logging_thread(&cmd, stderr.map(Self::log_stderr_output));
            }
        }
        Ok(())
    }

    fn wait_with_output(self, ignore_error: bool) -> Result<Vec<u8>> {
        let read_to_buf = |stdout: Option<PipeReader>| -> Result<Vec<u8>> {
            if let Some(mut out) = stdout {
                let mut buf = vec![];
                out.read_to_end(&mut buf)?;
                Ok(buf)
            } else {
                Ok(vec![])
            }
        };
        match self {
            CmdChild::Proc {
                mut child,
                cmd,
                stdout,
                stderr,
            } => {
                drop(child.stdin.take());
                let polling_stderr = stderr.map(CmdChild::log_stderr_output);
                let buf = read_to_buf(stdout)?;
                let status = child.wait()?;
                Self::wait_logging_thread(&cmd, polling_stderr);
                if !status.success() && !ignore_error {
                    return Err(Self::status_to_io_error(
                        status,
                        &format!("{} exited with error", cmd),
                    ));
                } else if process::debug_enabled() {
                    warn!("{} exited with error: {}", cmd, status);
                }
                Ok(buf)
            }
            CmdChild::ThreadFn {
                cmd,
                stdout,
                stderr,
                child,
                ..
            } => {
                let polling_stderr = stderr.map(CmdChild::log_stderr_output);
                let buf = read_to_buf(stdout)?;
                let status = child.join();
                Self::wait_logging_thread(&cmd, polling_stderr);
                match status {
                    Err(e) => {
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!("{} thread joined with error: {:?}", cmd, e),
                        ));
                    }
                    Ok(result) => {
                        if let Err(e) = result {
                            if !ignore_error {
                                return Err(e);
                            } else if process::debug_enabled() {
                                warn!("{} exited with error: {:?}", cmd, e);
                            }
                        }
                    }
                }
                Ok(buf)
            }
            CmdChild::SyncFn {
                cmd,
                stdout,
                stderr,
                ..
            } => {
                Self::wait_logging_thread(&cmd, stderr.map(Self::log_stderr_output));
                if let Some(mut out) = stdout {
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

    fn get_full_cmd(children: &[Self]) -> String {
        children
            .iter()
            .map(|child| match child {
                CmdChild::Proc { cmd, .. } => cmd.to_owned(),
                CmdChild::ThreadFn { cmd, .. } => cmd.to_owned(),
                CmdChild::SyncFn { cmd, .. } => cmd.to_owned(),
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }
}
