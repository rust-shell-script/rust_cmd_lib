use std::fmt::Display;
use std::io::{Read, Error, ErrorKind};
use std::process::{Command, Stdio, ExitStatus,Child, ChildStdout};
use std::collections::VecDeque;

pub type FunResult = Result<String, std::io::Error>;
pub type CmdResult = Result<(), std::io::Error>;
type PipeResult = Result<(Child, ChildStdout), std::io::Error>;

/// To display information to stderr, no return value
/// ```rust
/// info!("Running command xxx ...");
/// ```
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::info(format!($($arg)*))
    }
}

/// To return FunResult
/// ```rust
/// fn foo() -> FunResult
/// ...
/// output!("yes");
/// ```
#[macro_export]
macro_rules! output {
    ($($arg:tt)*) => {
        $crate::output(format!($($arg)*))
    }
}

///
/// ## run_cmd! --> CmdResult
/// ```rust
/// let name = "rust";
/// run_cmd!("echo hello, {}", name);
///
/// // pipe commands are also supported
/// run_cmd!("du -ah . | sort -hr | head -n 10");
/// ```
#[macro_export]
macro_rules! run_cmd {
    ($($arg:tt)*) => {
        $crate::run_cmd(format!($($arg)*))
    }
}

/// ## run_fun! --> FunResult
/// ```rust
/// let version = run_fun!("rustc --version")?;
/// info!("Your rust version is {}", version.trim());
///
/// // with pipes
/// let n = run_fun!("echo the quick brown fox jumped over the lazy dog | wc -w")?;
/// info!("There are {} words in above sentence", n.trim());
/// ```
#[macro_export]
macro_rules! run_fun {
    ($($arg:tt)*) => {
        $crate::run_fun(format!($($arg)*))
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
