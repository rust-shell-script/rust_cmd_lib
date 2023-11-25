use os_pipe::*;
use std::fs::File;
use std::io::{Read, Result, Write};
use std::process::Stdio;

/// Standard input stream for custom command implementation, which is part of [`CmdEnv`](crate::CmdEnv).
pub struct CmdIn(CmdInInner);

impl Read for CmdIn {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match &mut self.0 {
            CmdInInner::Null => Ok(0),
            CmdInInner::File(file) => file.read(buf),
            CmdInInner::Pipe(pipe) => pipe.read(buf),
        }
    }
}

impl From<CmdIn> for Stdio {
    fn from(cmd_in: CmdIn) -> Stdio {
        match cmd_in.0 {
            CmdInInner::Null => Stdio::null(),
            CmdInInner::File(file) => Stdio::from(file),
            CmdInInner::Pipe(pipe) => Stdio::from(pipe),
        }
    }
}

impl CmdIn {
    pub(crate) fn null() -> Self {
        Self(CmdInInner::Null)
    }

    pub(crate) fn file(f: File) -> Self {
        Self(CmdInInner::File(f))
    }

    pub(crate) fn pipe(p: PipeReader) -> Self {
        Self(CmdInInner::Pipe(p))
    }

    pub fn try_clone(&self) -> Result<Self> {
        match &self.0 {
            CmdInInner::Null => Ok(Self(CmdInInner::Null)),
            CmdInInner::File(file) => file.try_clone().map(|f| Self(CmdInInner::File(f))),
            CmdInInner::Pipe(pipe) => pipe.try_clone().map(|p| Self(CmdInInner::Pipe(p))),
        }
    }
}

enum CmdInInner {
    Null,
    File(File),
    Pipe(PipeReader),
}

/// Standard output stream for custom command implementation, which is part of [`CmdEnv`](crate::CmdEnv).
pub struct CmdOut(CmdOutInner);

impl Write for CmdOut {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match &mut self.0 {
            CmdOutInner::Null => Ok(buf.len()),
            CmdOutInner::File(file) => file.write(buf),
            CmdOutInner::Pipe(pipe) => pipe.write(buf),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match &mut self.0 {
            CmdOutInner::Null => Ok(()),
            CmdOutInner::File(file) => file.flush(),
            CmdOutInner::Pipe(pipe) => pipe.flush(),
        }
    }
}

impl CmdOut {
    pub(crate) fn null() -> Self {
        Self(CmdOutInner::Null)
    }

    pub(crate) fn file(f: File) -> Self {
        Self(CmdOutInner::File(f))
    }

    pub(crate) fn pipe(p: PipeWriter) -> Self {
        Self(CmdOutInner::Pipe(p))
    }

    pub fn try_clone(&self) -> Result<Self> {
        match &self.0 {
            CmdOutInner::Null => Ok(Self(CmdOutInner::Null)),
            CmdOutInner::File(file) => file.try_clone().map(|f| Self(CmdOutInner::File(f))),
            CmdOutInner::Pipe(pipe) => pipe.try_clone().map(|p| Self(CmdOutInner::Pipe(p))),
        }
    }
}

impl From<CmdOut> for Stdio {
    fn from(cmd_out: CmdOut) -> Stdio {
        match cmd_out.0 {
            CmdOutInner::Null => Stdio::null(),
            CmdOutInner::File(file) => Stdio::from(file),
            CmdOutInner::Pipe(pipe) => Stdio::from(pipe),
        }
    }
}

enum CmdOutInner {
    Null,
    File(File),
    Pipe(PipeWriter),
}
