use os_pipe::*;
use std::fs::File;
use std::io::{Read, Result, Write};
use std::process::Stdio;

#[derive(Debug)]
pub enum CmdIn {
    Null,
    File(File),
    Pipe(PipeReader),
}

impl Read for CmdIn {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self {
            CmdIn::Null => Ok(0),
            CmdIn::File(file) => file.read(buf),
            CmdIn::Pipe(pipe) => pipe.read(buf),
        }
    }
}

impl From<CmdIn> for Stdio {
    fn from(cmd_in: CmdIn) -> Stdio {
        match cmd_in {
            CmdIn::Null => Stdio::null(),
            CmdIn::File(file) => Stdio::from(file),
            CmdIn::Pipe(pipe) => Stdio::from(pipe),
        }
    }
}

#[derive(Debug)]
pub enum CmdOut {
    Null,
    File(File),
    Pipe(PipeWriter),
}

impl Write for CmdOut {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match self {
            CmdOut::Null => Ok(buf.len()),
            CmdOut::File(file) => file.write(buf),
            CmdOut::Pipe(pipe) => pipe.write(buf),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match self {
            CmdOut::Null => Ok(()),
            CmdOut::File(file) => file.flush(),
            CmdOut::Pipe(pipe) => pipe.flush(),
        }
    }
}

impl CmdOut {
    pub fn try_clone(&self) -> Result<Self> {
        match self {
            CmdOut::Null => Ok(CmdOut::Null),
            CmdOut::File(file) => file.try_clone().map(CmdOut::File),
            CmdOut::Pipe(pipe) => pipe.try_clone().map(CmdOut::Pipe),
        }
    }
}

impl From<CmdOut> for Stdio {
    fn from(cmd_out: CmdOut) -> Stdio {
        match cmd_out {
            CmdOut::Null => Stdio::null(),
            CmdOut::File(file) => Stdio::from(file),
            CmdOut::Pipe(pipe) => Stdio::from(pipe),
        }
    }
}
