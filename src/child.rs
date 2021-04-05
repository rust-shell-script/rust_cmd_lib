use crate::io::CmdIn;
use crate::CmdResult;
use log::info;
use os_pipe::PipeReader;
use std::io::{self, BufRead, BufReader, Error, ErrorKind, Read, Result, Write};
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
            Self::ProcChild(mut p) => {
                let status = p.child.wait()?;
                Self::log_stderr(&mut p.child);
                if !status.success() && (is_last || pipefail) {
                    return Err(Self::status_to_io_error(
                        status,
                        &format!("{} exited with error", p.cmd),
                    ));
                }
            }
            Self::ThreadChild(t) => {
                let status = t.child.join();
                match status {
                    Err(e) => {
                        if is_last || pipefail {
                            return Err(Error::new(
                                ErrorKind::Other,
                                format!("{} thread exited with error: {:?}", t.cmd, e),
                            ));
                        }
                    }
                    Ok(result) => {
                        check_result(result)?;
                    }
                }
            }
            Self::SyncChild(s) => {
                if let Some(mut out) = s.output {
                    let mut buf = vec![];
                    check_result(out.read_to_end(&mut buf).map(|_|()))?;
                    check_result(io::stdout().write_all(&buf[..]))?;
                }
            }
        }
        Ok(())
    }

    pub fn wait_with_output(self) -> Result<Vec<u8>> {
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
                panic!("{} thread should not be waited for output", t.cmd);
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
    pub child: JoinHandle<CmdResult>,
    pub cmd: String,
    pub stderr_logging: Option<CmdIn>,
}

pub struct CmdSyncChild {
    pub output: Option<PipeReader>,
    pub cmd: String,
}
