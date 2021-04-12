use os_pipe::*;
use std::fs::File;
use std::io::{Read, Result, Write};
use std::process::Stdio;

pub enum CmdIn {
    CmdNull,
    CmdFile(File),
    CmdPipe(PipeReader),
}

impl Read for CmdIn {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self {
            CmdIn::CmdNull => Ok(0),
            CmdIn::CmdFile(file) => file.read(buf),
            CmdIn::CmdPipe(pipe) => pipe.read(buf),
        }
    }
}

impl From<CmdIn> for Stdio {
    fn from(cmd_in: CmdIn) -> Stdio {
        match cmd_in {
            CmdIn::CmdNull => Stdio::null(),
            CmdIn::CmdFile(file) => Stdio::from(file),
            CmdIn::CmdPipe(pipe) => Stdio::from(pipe),
        }
    }
}

pub enum CmdOut {
    CmdNull,
    CmdFile(File),
    CmdPipe(PipeWriter),
}

impl Write for CmdOut {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match self {
            CmdOut::CmdNull => Ok(buf.len()),
            CmdOut::CmdFile(file) => file.write(buf),
            CmdOut::CmdPipe(pipe) => pipe.write(buf),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match self {
            CmdOut::CmdNull => Ok(()),
            CmdOut::CmdFile(file) => file.flush(),
            CmdOut::CmdPipe(pipe) => pipe.flush(),
        }
    }
}

impl CmdOut {
    pub fn try_clone(&self) -> Result<Self> {
        match self {
            CmdOut::CmdNull => Ok(CmdOut::CmdNull),
            CmdOut::CmdFile(file) => file.try_clone().map(CmdOut::CmdFile),
            CmdOut::CmdPipe(pipe) => pipe.try_clone().map(CmdOut::CmdPipe),
        }
    }
}

impl From<CmdOut> for Stdio {
    fn from(cmd_out: CmdOut) -> Stdio {
        match cmd_out {
            CmdOut::CmdNull => Stdio::null(),
            CmdOut::CmdFile(file) => Stdio::from(file),
            CmdOut::CmdPipe(pipe) => Stdio::from(pipe),
        }
    }
}
