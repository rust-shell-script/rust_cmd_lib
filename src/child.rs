use crate::CmdResult;
use log::info;
use os_pipe::{self, PipeReader};
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Result};
use std::process::{Child, ExitStatus};
use std::thread::JoinHandle;
use CmdChild::{ProcChild, SyncChild, ThreadChild};

#[derive(Debug)]
pub enum CmdChild {
    ProcChild {
        child: Child,
        cmd: String,
        stderr: Option<PipeReader>,
    },
    ThreadChild {
        child: JoinHandle<CmdResult>,
        cmd: String,
        stderr: Option<PipeReader>,
    },
    SyncChild {
        output: Option<PipeReader>,
        cmd: String,
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
            ThreadChild { child, cmd, stderr } => {
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
            SyncChild { output, stderr, .. } => {
                Self::log_stderr_output(stderr);
                if let Some(mut out) = output {
                    let mut buf = vec![];
                    check_result(out.read_to_end(&mut buf).map(|_| ()))?;
                    println!("{}", String::from_utf8_lossy(&buf));
                }
            }
        }
        Ok(())
    }

    pub fn wait_with_output(self) -> Result<Vec<u8>> {
        match self {
            ProcChild { child, cmd, stderr } => {
                Self::log_stderr_output(stderr);
                let output = child.wait_with_output()?;
                if !output.status.success() {
                    return Err(Self::status_to_io_error(
                        output.status,
                        &format!("{} exited with error", cmd),
                    ));
                } else {
                    Ok(output.stdout)
                }
            }
            ThreadChild { cmd, .. } => {
                panic!("{} thread should not be waited for output", cmd);
            }
            SyncChild { output, stderr, .. } => {
                Self::log_stderr_output(stderr);
                if let Some(mut out) = output {
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
