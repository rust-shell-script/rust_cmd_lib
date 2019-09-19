use std::io::{Error, ErrorKind};
use std::process;
use std::process::ExitStatus;
use std::fmt::Display;

pub type FunResult = Result<String, std::io::Error>;
pub type CmdResult = Result<(), std::io::Error>;

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        info(format!($($arg)*))
    }
}

#[macro_export]
macro_rules! output {
    ($($arg:tt)*) => {
        output(format!($($arg)*))
    }
}

#[macro_export]
macro_rules! run_cmd {
    ($($arg:tt)*) => {
        run_cmd(&format!($($arg)*).split_whitespace().collect::<Vec<&str>>());
    }
}

#[macro_export]
macro_rules! run_fun {
    ($($arg:tt)*) => {
        run_fun(&format!($($arg)*).split_whitespace().collect::<Vec<&str>>());
    }
}

pub fn info<S>(msg: S) where S: Into<String> + Display {
    eprintln!("{}", msg);
}

pub fn output<S>(msg: S) -> String where S: Into<String> {
    msg.into()
}

pub fn run_cmd(full_command: &[&str]) -> CmdResult {
    info!("Running {:?} ...", full_command);
    let command = &full_command[0];
    let status = process::Command::new(command)
                                  .args(&full_command[1..])
                                  .status()?;
    if !status.success() {
        Err(to_io_error(command, status))
    } else {
        Ok(())
    }
}

pub fn run_fun(full_command: &[&str]) -> FunResult {
    info!("Running {:?} ...", full_command);
    let command = &full_command[0];
    let output = process::Command::new(command)
                                  .args(&full_command[1..])
                                  .output()?;
    if ! output.status.success() {
        Err(to_io_error(command, output.status))
    } else {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

fn to_io_error(command: &str, status: ExitStatus) -> Error {
    if let Some(code) = status.code() {
        Error::new(ErrorKind::Other,
                        format!("{} exit with {}", command, code))
    } else {
        Error::new(ErrorKind::Other, "Unknown error")
    }
}

