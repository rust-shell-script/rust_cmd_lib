use std::fmt::Display;
use std::io::{Read, Error, ErrorKind};
use std::process::{Command, Stdio, ExitStatus,Child, ChildStdout};
use std::collections::VecDeque;

pub type FunResult = Result<String, std::io::Error>;
pub type CmdResult = Result<(), std::io::Error>;
pub type PipeResult = Result<(Child, ChildStdout), std::io::Error>;

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
        $crate::cmd_lib::run_cmd(format!($($arg)*))
    }
}

#[macro_export]
macro_rules! run_fun {
    ($($arg:tt)*) => {
        $crate::cmd_lib::run_fun(format!($($arg)*))
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
pub fn run_pipe(full_command: &str) -> PipeResult {
    let pipe_args = parse_pipes(full_command);
    let pipe_argv = parse_argv(&pipe_args);
    let n = pipe_argv.len();
    let mut pipe_procs = VecDeque::with_capacity(n);
    let mut pipe_outputs = VecDeque::with_capacity(n);

    info!("Running \"{}\" ...", full_command);
    for (i, pipe_cmd) in pipe_argv.iter().enumerate() {
        let args = parse_args(pipe_cmd);
        let argv = parse_argv(&args);

        if i == 0 {
            pipe_procs.push_back(Command::new(&argv[0])
                .args(&argv[1..])
                .stdout(Stdio::piped())
                .spawn()?);
        } else {
            pipe_procs.push_back(Command::new(&argv[0])
                .args(&argv[1..])
                .stdin(pipe_outputs.pop_front().unwrap())
                .stdout(Stdio::piped())
                .spawn()?);
            pipe_procs.pop_front().unwrap().wait()?;
        }

        pipe_outputs.push_back(pipe_procs.back_mut().unwrap().stdout.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Broken pipe")
        })?);
   }

   Ok((pipe_procs.pop_front().unwrap(), pipe_outputs.pop_front().unwrap()))
}

#[doc(hidden)]
pub fn run_cmd(full_command: String) -> CmdResult {
    let (mut proc, mut output) = run_pipe(&full_command)?;
    let status = proc.wait()?;
    if !status.success() {
        Err(to_io_error(&full_command, status))
    } else {
        let mut s = String::new();
        output.read_to_string(&mut s)?;
        print!("{}", s);
        Ok(())
    }
}

#[doc(hidden)]
pub fn run_fun(full_command: String) -> FunResult {
    let (mut proc, mut output) = run_pipe(&full_command)?;
    let status = proc.wait()?;
    if !status.success() {
        Err(to_io_error(&full_command, status))
    } else {
        let mut s = String::new();
        output.read_to_string(&mut s)?;
        Ok(s)
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

fn parse_pipes(s: &str) -> String {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    s.chars()
        .map(|c| {
            if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
            } else if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
            }

            if c == '|' && !in_single_quote && !in_double_quote {
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
