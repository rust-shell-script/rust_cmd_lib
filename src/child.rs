use crate::io::CmdIn;
use crate::CmdResult;
use log::info;
use os_pipe::PipeReader;
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Result, Write};
use std::process::{Child, ExitStatus};
use std::thread::JoinHandle;

pub enum CmdChildHandle {
    ProcChild(CmdProcChild),
    ThreadChild(CmdThreadChild),
    SyncChild(CmdSyncChild),
}

impl CmdChildHandle {
    pub fn get_cmd(&self) -> String {
        match self {
            Self::ProcChild(p) => p.cmd.to_string(),
            Self::ThreadChild(t) => t.cmd.to_string(),
            Self::SyncChild(s) => s.cmd.to_string(),
        }
    }

    pub fn wait(self) -> CmdResult {
        match self {
            Self::ProcChild(mut p) => {
                let status = p.child.wait()?;
                Self::log_stderr(&mut p.child);
                if !status.success() && std::env::var("CMD_LIB_PIPEFAIL") != Ok("0".into()) {
                    return Err(Self::status_to_io_error(
                        status,
                        &format!("{} exited with error", p.cmd),
                    ));
                }
                Ok(())
            }
            Self::ThreadChild(t) => {
                let status = t.child.join();
                if let Err(e) = status {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!("{} thread exited with error: {:?}", t.cmd, e),
                    ));
                }
                Ok(())
            }
            Self::SyncChild(s) => {
                if let Some(mut out) = s.output {
                    let mut buf = vec![];
                    out.read_to_end(&mut buf)?;
                    std::io::stdout().write_all(&buf[..])?;
                }
                Ok(())
            }
        }
    }

    pub fn wait_last_with_output(self) -> Result<Vec<u8>> {
        match self {
            Self::ProcChild(p) => {
                let output = p.child.wait_with_output()?;
                Self::log_stderr_output(&output.stderr[..]);
                if !output.status.success() {
                    return Err(Self::status_to_io_error(
                        output.status,
                        &format!("{} exited with error", p.cmd),
                    ));
                } else {
                    Ok(output.stdout)
                }
            }
            Self::ThreadChild(t) => {
                let status = t.child.join();
                if let Err(e) = status {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!("{} thread exited with error: {:?}", t.cmd, e),
                    ));
                }
                Ok(vec![])
            }
            Self::SyncChild(s) => {
                if let Some(mut out) = s.output {
                    let mut buf = vec![];
                    out.read_to_end(&mut buf)?;
                    return Ok(buf);
                }
                Ok(vec![])
            }
        }
    }

    fn log_stderr(child: &mut Child) {
        if let Some(stderr) = child.stderr.take() {
            Self::log_stderr_output(stderr);
        }
    }

    pub fn log_stderr_output(output: impl Read) {
        BufReader::new(output)
            .lines()
            .filter_map(|line| line.ok())
            .for_each(|line| info!("{}", line));
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
            .map(|cmd| cmd.get_cmd())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

pub struct CmdProcChild {
    pub child: Child,
    pub cmd: String,
}

pub struct CmdThreadChild {
    pub child: JoinHandle<()>,
    pub cmd: String,
    pub stderr_logging: Option<CmdIn>,
}

pub struct CmdSyncChild {
    pub output: Option<PipeReader>,
    pub cmd: String,
}
