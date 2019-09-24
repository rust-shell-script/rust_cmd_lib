use std::io::{Read, Error, ErrorKind};
use std::process::{Command, Stdio, ExitStatus,Child, ChildStdout};
use std::collections::VecDeque;

pub type FunResult = Result<String, std::io::Error>;
pub type CmdResult = Result<(), std::io::Error>;
type PipeResult = Result<(Child, ChildStdout), std::io::Error>;

/// To print warning information to stderr, no return value
/// ```rust
/// info!("Running command xxx ...");
/// ```
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        eprintln!("INFO: {}", format!($($arg)*));
    }
}

/// To print warning information to stderr, no return value
/// ```rust
/// warn!("Running command failed");
/// ```
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        eprintln!("WARN: {}", format!($($arg)*));
    }
}

/// To print error information to stderr, no return value
/// ```rust
/// err!("Copying file failed");
/// ```
#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => {
        eprintln!("ERROR: {}", format!($($arg)*));
    }
}

/// To print information to stderr, and exit current process with non-zero
/// ```rust
/// die!("command failed: {}", reason);
/// ```
#[macro_export]
macro_rules! die {
    ($($arg:tt)*) => {
        eprintln!("FATAL: {}", format!($($arg)*));
        std::process::exit(1);
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
        Ok(format!($($arg)*)) as FunResult
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
///
/// // without string quotes
/// let files = run_fun!(du -ah . | sort -hr | head -n 10)?;
/// ```
#[macro_export]
macro_rules! run_fun {
   // without string quotes
   ($cmd:ident $($arg:tt)*) => {
       $crate::run_fun(&format!("{} {}", stringify!($cmd), stringify!($($arg)*)), true)
   };
   // normal: start with string
   ($($arg:tt)*) => {
       $crate::run_fun(&format!($($arg)*), false)
   };
}

///
/// ## run_cmd! --> CmdResult
/// ```rust
/// let name = "rust";
/// run_cmd!("echo hello, {}", name);
///
/// // pipe commands are also supported
/// run_cmd!("du -ah . | sort -hr | head -n 10");
///
/// // work without string quote
/// run_cmd!(du -ah . | sort -hr | head -n 10);
/// ```
/// // or a group of commands
/// // if any command fails, just return Err(...)
/// run_cmd!{
///     date;
///     ls -l /file;
/// }
/// ```
#[macro_export]
macro_rules! run_cmd {
    // use {{ to work around bug:
    // https://github.com/rust-lang/rust/issues/53667
    ($x:ident $($other:tt)*) => {{
        let mut s = String::from(stringify!($x));
        run_cmd!(&s; $($other)*)
    }};
    (&$s:expr; $x:tt $($other:tt)*) => {{
        $s += " ";
        $s += stringify!($x);
        run_cmd!(&$s; $($other)*)
    }};
    (&$s:expr; $x:tt; $($other:tt)*) => {{
        $s += " ";
        $s += stringify!($x);
        $s += ";";
        run_cmd!(&$s; $($other)*)
    }};
    (&$s:expr;) => {
        $crate::run_cmd(&$s, true)
    };

    // normal: start with string
    ($($arg:tt)*) => {
        $crate::run_cmd(&format!($($arg)*), false)
    };
}

#[doc(hidden)]
pub fn run_pipe(full_command: &str) -> PipeResult {
    let pipe_args = parse_pipes(full_command.trim());
    let pipe_argv = parse_argv(&pipe_args);
    let n = pipe_argv.len();
    let mut pipe_procs = VecDeque::with_capacity(n);
    let mut pipe_outputs = VecDeque::with_capacity(n);

    info!("Running \"{}\" ...", full_command.trim());
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

fn run(full_cmd: &str) -> FunResult {
    let (mut proc, mut output) = run_pipe(full_cmd)?;
    let status = proc.wait()?;
    if !status.success() {
        Err(to_io_error(full_cmd, status))
    } else {
        let mut s = String::new();
        output.read_to_string(&mut s)?;
        Ok(s)
    }
}

#[doc(hidden)]
pub fn run_fun(full_cmd: &str, need_filter: bool) -> FunResult {
    let full_cmd = if need_filter {
        filter_spaces(full_cmd)
    } else {
        full_cmd.into()
    };
    run(&full_cmd)
}

#[doc(hidden)]
pub fn run_cmd(cmds: &str, need_filter: bool) -> CmdResult {
    let cmds = if need_filter {
        filter_spaces(cmds)
    } else {
        cmds.into()
    };
    let cmd_args = parse_cmds(&cmds);
    let cmd_argv = parse_argv(&cmd_args);
    for cmd in cmd_argv {
        match run(cmd) {
            Err(e) => return Err(e),
            Ok(s) => print!("{}", s),
        }
    }
    Ok(())
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

fn parse_cmds(s: &str) -> String {
    parse_seps(s, ';')
}

fn parse_pipes(s: &str) -> String {
    parse_seps(s, '|')
}

fn parse_seps(s: &str, sep: char) -> String {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    s.chars()
        .map(|c| {
            if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
            } else if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
            }

            if c == sep && !in_single_quote && !in_double_quote {
                '\n'
            } else {
                c
            }
        })
        .collect()
}

fn filter_spaces(s: &str) -> String {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut is_dash = false;
    let mut is_slash = false;
    let mut is_plus = false;
    let mut is_dot = false;
    let mut is_star = false;
    s.chars()
        .filter(|c| {
            if *c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
            } else if *c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
            }

            if !in_single_quote && !in_double_quote {
                match *c {
                    '.' => { is_dot = true; true},
                    '+' => { is_plus = true; true},
                    '-' => { is_dash = true; true},
                    '*' => { is_star = true; true},
                    '/' => { is_slash = true; true},
                    ' ' => {
                        if is_dot {
                            is_dot = false;
                            false
                        } else if is_plus {
                            is_plus = false;
                            false
                        } else if is_dash {
                            is_dash = false;
                            false
                        } else if is_star {
                            is_star = false;
                            false
                        } else if is_slash {
                            is_slash = false;
                            false
                        } else {
                            true
                        }
                    },
                    _ => true,
                }
            } else {
                true
            }
        })
        .collect()
}

fn parse_argv(s: &str) -> Vec<&str> {
    s.split("\n")
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
}

