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
        $crate::cmd_lib::run_cmd(format!($($arg)*));
    }
}

#[macro_export]
macro_rules! run_fun {
    ($($arg:tt)*) => {
        $crate::cmd_lib::run_fun(format!($($arg)*));
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
pub fn run_cmd(full_command: String) -> CmdResult {
    let args = parse_args(&full_command);
    let argv = parse_argv(&args);

    info!("Running {:?} ...", argv);
    let status = process::Command::new(&argv[0]).args(&argv[1..]).status()?;
    if !status.success() {
        Err(to_io_error(&argv[0], status))
    } else {
        Ok(())
    }
}

#[doc(hidden)]
pub fn run_fun(full_command: String) -> FunResult {
    let args = parse_args(&full_command);
    let argv = parse_argv(&args);

    info!("Running {:?} ...", argv);
    let output = process::Command::new(&argv[0]).args(&argv[1..]).output()?;
    if !output.status.success() {
        Err(to_io_error(&argv[0], output.status))
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

fn parse_args(s: &str) -> String {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    s.chars()
        .map(|c| {
            if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
                '\n'
            } else if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
                '\n'
            } else if !in_single_quote && !in_double_quote && char::is_whitespace(c) {
                '\n'
            } else {
                c
            }
        })
        .collect()
}

fn parse_argv(s: &str) -> Vec<&str> {
    s.split("\n")
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
}
