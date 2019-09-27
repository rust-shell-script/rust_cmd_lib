use std::io::{Error, ErrorKind};
use std::process::{Command, Stdio, ExitStatus, Child};
use std::collections::HashMap;

pub type FunResult = Result<String, std::io::Error>;
pub type CmdResult = Result<(), std::io::Error>;
type PipeResult = Result<Child, std::io::Error>;

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
    ($($arg:tt)*) => {{
        use std::process::exit;
        eprintln!("FATAL: {}", format!($($arg)*));
        exit(1);
    }}
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

// XX: hack here to return orignal macro string
// In future, use proc macro or wait for std provide such a macro
#[doc(hidden)]
#[macro_export]
macro_rules! macro_str {
    ($macro:ident) => {{
        let macro_name = stringify!($macro);
        let mut macro_str = String::new();
        let src = String::from(format!("{}/{}",
                                       env!("CARGO_MANIFEST_DIR"),
                                       file!()));
        let target_line = line!() as usize;
	let file: Vec<char> = std::fs::read_to_string(src)
	    .expect("error reading file")
	    .chars()
	    .collect();
	let len = file.len();
        let mut i: usize = 0;
        let mut line = 1;
        let mut level = 0;
	while i < len {
            if file[i] == '\n' {
                line += 1;
            }
            if line == target_line {
                let cmp_str: String = file[i..i+macro_name.len()].iter().collect();
                if cmp_str == macro_name {
                    i += macro_name.len()+1;
                    while file[i] != '{' && file[i] != '(' {
                        i += 1;
                    }
                    i += 1;
                    level += 1;

                    let with_quote = file[i] == '"';
                    let mut in_single_quote = false;
                    let mut in_double_quote = false;
                    if with_quote {
                        in_double_quote = true;
                        i += 1;
                    }
                    loop {
                        if !in_single_quote &&
                           !in_double_quote {
                            if file[i] == '}' || file[i] == ')' {
                                level -= 1;
                            } else if file[i] == '{' || file[i] == '(' {
                                level += 1;
                            }

                            if level == 0 {
                                break;
                            }
                        }

                        if file[i] == '"' && !in_single_quote {
                            in_double_quote = !in_double_quote;
                        } else if file[i] == '\'' && !in_double_quote {
                            in_single_quote = !in_single_quote;
                        }

                        macro_str.push(file[i]);
                        i += 1;
                    }
                    if with_quote {
                        macro_str.pop();
                    }
                    break;
                }
            }
            i += 1;
        }
        macro_str
    }}
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
   ($cmd:ident $($arg:tt)*) => {
       $crate::run_fun(&$crate::macro_str!(run_fun))
   };
   ($($arg:tt)*) => {
       $crate::run_fun(&format!($($arg)*))
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
///
/// // or a group of commands
/// // if any command fails, just return Err(...)
/// run_cmd!{
///     use file;
///
///     date;
///     ls -l ${file};
/// }
/// ```
#[macro_export]
macro_rules! run_cmd {
    (use $($arg:tt)*) => {{
        let mut sym_table = ::std::collections::HashMap::new();
        run_cmd!(&sym_table; $($arg)*)
    }};
    (&$st:expr; $var:ident, $($arg:tt)*) => {{
        $st.insert(stringify!($var).into(), format!("{}", $var));
        run_cmd!(&$st; $($arg)*)
    }};
    (&$st:expr; $var:ident; $($arg:tt)*) => {{
        $st.insert(stringify!($var).into(), format!("{}", $var));
        let src = $crate::macro_str!(run_cmd);
        $crate::run_cmd(&$crate::resolve_name(&src, &$st, &file!(), line!()))
    }};
    ($cmd:ident $($arg:tt)*) => {{
        $crate::run_cmd(&$crate::macro_str!(run_cmd))
    }};
    ($($arg:tt)*) => {{
        $crate::run_cmd(&format!($($arg)*))
    }};
}

fn pipe(last_proc: Option<Child>, pipe_cmd: &str) -> PipeResult {
    let args = parse_args(pipe_cmd);
    let argv = parse_argv(&args);
    match last_proc {
        None => {
            Command::new(&argv[0])
                .args(&argv[1..])
                .stdout(Stdio::piped())
                .spawn()
        },
        Some(p) =>  {
            let mut last_proc = p;
            let new_proc = Command::new(&argv[0])
                            .args(&argv[1..])
                            .stdin(last_proc.stdout.take().unwrap())
                            .stdout(Stdio::piped())
                            .spawn();
            last_proc.wait()?;
            new_proc
        }
    }
}

#[doc(hidden)]
pub fn run_pipe(full_command: &str) -> PipeResult {
    let pipe_args = parse_pipes(full_command.trim());
    let pipe_argv = parse_argv(&pipe_args);

    info!("Running \"{}\" ...", full_command.trim());
    let mut last_proc: Option<Child> = None;
    for pipe_cmd in pipe_argv {
        last_proc = Some(pipe(last_proc, pipe_cmd)?);
    }

   Ok(last_proc.unwrap())
}

fn run(full_cmd: &str) -> FunResult {
    let last_proc = run_pipe(full_cmd)?;
    let output = last_proc.wait_with_output()?;
    if !output.status.success() {
        Err(to_io_error(full_cmd, output.status))
    } else {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[doc(hidden)]
pub fn run_fun(full_cmd: &str) -> FunResult {
    run(full_cmd)
}

#[doc(hidden)]
pub fn run_cmd(cmds: &str) -> CmdResult {
    let cmd_args = parse_cmds(cmds);
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

fn parse_argv(s: &str) -> Vec<&str> {
    s.split("\n")
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<&str>>()
}

#[doc(hidden)]
pub fn resolve_name(src: &str, st: &HashMap<String,String>, file: &str, line: u32) -> String {
    let mut output = String::new();
    let input: Vec<char> = src.chars().collect();
    let len = input.len();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    let mut i = 0;
    while i < len {
        if i == 0 { // skip variable declaration part
            while input[i] == ' ' || input[i] == '\t' || input[i] == '\n' {
                i += 1;
            }
            let first = input[i..i+4].iter().collect::<String>();
            if i < len-4 && first == "use " || first == "use\t" {
                while input[i] != ';' {
                    i += 1;
                }
            }
        }

        if input[i] == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
        } else if input[i] == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
        }

        if !in_single_quote && i < len-2 &&
           input[i] == '$' && input[i+1] == '{' {
            i += 2;
            let mut var = String::new();
            while input[i] != '}' {
                var.push(input[i]);
                if input[i] == ';' || input[i] == '\n' || i == len-1 {
                    die!("invalid name {}, {}:{}\n{}", var, file, line, src);
                }
                i += 1;
            }
            match st.get(&var) {
                None => {
                    die!("resolve {} failed, {}:{}\n{}", var, file, line, src);
                },
                Some(v) => {
                    if in_double_quote {
                        output += v;
                    } else {
                        output += "\"";
                        output += v;
                        output += "\"";
                    }
                }
            }
        } else {
            output.push(input[i]);
        }
        i += 1;
    }

    output
}



