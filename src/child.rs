use crate::{CmdResult, FunResult};
use log::{error, info};
use os_pipe::PipeReader;
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Result};
use std::process::{Child, ExitStatus};
use std::thread::JoinHandle;
use CmdChild::{ProcChild, SyncChild, ThreadChild};

/// Representation of running or exited children processes, connected with pipes
/// optionally.
///
/// Calling `spawn!` macro will return `Result<CmdChildren>`
pub struct CmdChildren(Vec<CmdChild>);
impl CmdChildren {
    pub(crate) fn from(children: Vec<CmdChild>) -> Self {
        Self(children)
    }

    pub fn wait_cmd_result(&mut self) -> CmdResult {
        let ret = self.wait_cmd_result_nolog();
        if let Err(ref err) = ret {
            error!(
                "Running {} failed, Error: {}",
                CmdChild::get_full_cmd(&self.0),
                err
            );
        }
        ret
    }

    pub(crate) fn wait_cmd_result_nolog(&mut self) -> CmdResult {
        // wait last process result
        let handle = self.0.pop().unwrap();
        handle.wait(true)?;
        Self::wait_children(&mut self.0)
    }

    fn wait_children(children: &mut Vec<CmdChild>) -> CmdResult {
        while !children.is_empty() {
            let child_handle = children.pop().unwrap();
            child_handle.wait(false)?;
        }
        Ok(())
    }

    pub fn wait_fun_result(&mut self) -> FunResult {
        let ret = self.wait_fun_result_nolog();
        if let Err(ref err) = ret {
            error!(
                "Running {} failed, Error: {}",
                CmdChild::get_full_cmd(&self.0),
                err
            );
        }
        ret
    }

    pub(crate) fn wait_fun_result_nolog(&mut self) -> FunResult {
        // wait last process result
        let handle = self.0.pop().unwrap();
        let wait_last = handle.wait_with_output();
        match wait_last {
            Err(e) => {
                let _ = CmdChildren::wait_children(&mut self.0);
                Err(e)
            }
            Ok(output) => {
                let mut ret = String::from_utf8_lossy(&output).to_string();
                if ret.ends_with('\n') {
                    ret.pop();
                }
                CmdChildren::wait_children(&mut self.0)?;
                Ok(ret)
            }
        }
    }

    pub fn wait_with_pipe(&mut self, f: &mut dyn FnMut(PipeReader) -> CmdResult) -> CmdResult {
        let handle = self.0.pop().unwrap();
        let mut ret = Ok(());
        match handle {
            ProcChild {
                mut child,
                stderr,
                stdout,
                ..
            } => {
                if let Some(stdout) = stdout {
                    ret = f(stdout);
                    let _ = child.kill();
                }
                CmdChild::log_stderr_output(stderr);
            }
            ThreadChild { .. } => {
                panic!("should not wait pipe on thread");
            }
            SyncChild { stderr, stdout, .. } => {
                CmdChild::log_stderr_output(stderr);
                if let Some(stdout) = stdout {
                    ret = f(stdout);
                }
            }
        };
        let _ = Self::wait_children(&mut self.0);
        ret
    }
}

#[derive(Debug)]
pub enum CmdChild {
    ProcChild {
        child: Child,
        cmd: String,
        stdout: Option<PipeReader>,
        stderr: Option<PipeReader>,
    },
    ThreadChild {
        child: JoinHandle<CmdResult>,
        cmd: String,
        stdout: Option<PipeReader>,
        stderr: Option<PipeReader>,
    },
    SyncChild {
        cmd: String,
        stdout: Option<PipeReader>,
        stderr: Option<PipeReader>,
    },
}

impl CmdChild {
    pub fn wait(self, is_last: bool) -> CmdResult {
        let pipefail = std::env::var("CMD_LIB_PIPEFAIL") != Ok("0".into());
        let check_result = |result| {
            if let Err(e) = result {
                if is_last || pipefail {
                    return Err(e);
                }
            }
            Ok(())
        };
        match self {
            ProcChild {
                mut child,
                stderr,
                cmd,
                ..
            } => {
                Self::log_stderr_output(stderr);
                if let Some(stdout) = child.stdout.take() {
                    BufReader::new(stdout)
                        .lines()
                        .filter_map(|line| line.ok())
                        .for_each(|line| println!("{}", line));
                }
                let status = child.wait()?;
                if !status.success() && (is_last || pipefail) {
                    return Err(Self::status_to_io_error(
                        status,
                        &format!("{} exited with error", cmd),
                    ));
                }
            }
            ThreadChild {
                child, cmd, stderr, ..
            } => {
                let status = child.join();
                Self::log_stderr_output(stderr);
                match status {
                    Err(e) => {
                        if is_last || pipefail {
                            return Err(Error::new(
                                ErrorKind::Other,
                                format!("{} thread exited with error: {:?}", cmd, e),
                            ));
                        }
                    }
                    Ok(result) => {
                        check_result(result)?;
                    }
                }
            }
            SyncChild { stdout, stderr, .. } => {
                Self::log_stderr_output(stderr);
                if let Some(mut out) = stdout {
                    let mut buf = vec![];
                    check_result(out.read_to_end(&mut buf).map(|_| ()))?;
                    print!("{}", String::from_utf8_lossy(&buf));
                }
            }
        }
        Ok(())
    }

    pub fn wait_with_output(self) -> Result<Vec<u8>> {
        match self {
            ProcChild {
                mut child,
                cmd,
                stdout,
                stderr,
            } => {
                let mut buf = vec![];
                if let Some(mut stdout) = stdout {
                    stdout.read_to_end(&mut buf)?;
                }
                Self::log_stderr_output(stderr);
                let status = child.wait()?;
                if !status.success() {
                    return Err(Self::status_to_io_error(
                        status,
                        &format!("{} exited with error", cmd),
                    ));
                }
                Ok(buf)
            }
            ThreadChild { cmd, .. } => {
                panic!("{} thread should not be waited for output", cmd);
            }
            SyncChild { stdout, stderr, .. } => {
                Self::log_stderr_output(stderr);
                if let Some(mut out) = stdout {
                    let mut buf = vec![];
                    out.read_to_end(&mut buf)?;
                    return Ok(buf);
                }
                Ok(vec![])
            }
        }
    }

    fn log_stderr_output(stderr: Option<impl Read>) {
        if let Some(stderr) = stderr {
            BufReader::new(stderr)
                .lines()
                .filter_map(|line| line.ok())
                .for_each(|line| info!("{}", line));
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

    pub fn get_full_cmd(children: &[Self]) -> String {
        children
            .iter()
            .map(|child| match child {
                ProcChild { cmd, .. } => cmd.to_owned(),
                ThreadChild { cmd, .. } => cmd.to_owned(),
                SyncChild { cmd, .. } => cmd.to_owned(),
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }
}
