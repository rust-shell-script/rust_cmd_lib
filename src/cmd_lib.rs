use std::fmt::Display;
use std::io::{Error, ErrorKind};
use std::process;
use std::process::ExitStatus;

pub type FunResult = Result<String, std::io::Error>;
pub type CmdResult = Result<(), std::io::Error>;

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::cmd_lib::info(format!($($arg)*))
    }
}

#[macro_export]
macro_rules! output {
    ($($arg:tt)*) => {
        $crate::cmd_lib::output(format!($($arg)*))
    }
}

#[macro_export]
macro_rules! run_cmd {
    ($($arg:tt)*) => {
        $crate::cmd_lib::run_cmd(
            $crate::cmd_lib::split_cmd_args(&mut format!($($arg)*)).as_ref());
    }
}

#[macro_export]
macro_rules! run_fun {
    ($($arg:tt)*) => {
        $crate::cmd_lib::run_fun(
            $crate::cmd_lib::split_cmd_args(&mut format!($($arg)*)).as_ref());
    }
}

#[doc(hidden)]
pub fn info<S>(msg: S)
where
    S: Into<String> + Display,
{
    eprintln!("{}", msg);
}

#[doc(hidden)]
pub fn output<S>(msg: S) -> FunResult
where
    S: Into<String>,
{
    Ok(msg.into())
}

#[doc(hidden)]
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

#[doc(hidden)]
pub fn run_fun(full_command: &[&str]) -> FunResult {
    info!("Running {:?} ...", full_command);
    let command = &full_command[0];
    let output = process::Command::new(command)
        .args(&full_command[1..])
        .output()?;
    if !output.status.success() {
        Err(to_io_error(command, output.status))
    } else {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

fn to_io_error(command: &str, status: ExitStatus) -> Error {
    if let Some(code) = status.code() {
        Error::new(ErrorKind::Other, format!("{} exit with {}", command, code))
    } else {
        Error::new(ErrorKind::Other, "Unknown error")
    }
}

#[doc(hidden)]
pub fn split_cmd_args(s: &mut str) -> Vec<&str> {
    let s = s.trim_start();
    let first_space = s.find(char::is_whitespace);
    match first_space {
        None => vec![s],
        Some(c) => vec![&s[..c], s[c..].trim_start()],
    }
}
