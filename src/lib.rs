use std::borrow::Borrow;
use std::collections::HashMap;
use std::io::{Error, ErrorKind, Result};
use std::process::{Child, Command, ExitStatus, Stdio};

pub type FunResult = Result<String>;
pub type CmdResult = Result<()>;

/// To print warning information to stderr, no return value
/// ```rust
/// #[macro_use]
/// use cmd_lib::info;
///
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
/// #[macro_use]
/// use cmd_lib::warn;
///
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
/// #[macro_use]
/// use cmd_lib::err;
///
/// err!("Copying file failed");
/// ```
#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => {
        eprintln!("ERROR: {}", format!($($arg)*));
    }
}

/// To print information to stderr, and exit current process with non-zero
/// ```should_panic
/// #[macro_use]
/// use cmd_lib::die;
///
/// let reason = "system error";
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
/// ```compile_fail
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
        let src = String::from(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), file!()));
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
                let cmp_str: String = file[i..i + macro_name.len()].iter().collect();
                if cmp_str == macro_name {
                    i += macro_name.len() + 1;
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
                        if !in_single_quote && !in_double_quote {
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
    }};
}

/// ## run_fun! --> FunResult
/// ```no_run
/// #[macro_use]
/// use cmd_lib::{info, run_fun};
/// let version = run_fun!("rustc --version");
/// info!("Your rust version is {:?}", version);
///
/// // with pipes
/// let n = run_fun!("echo the quick brown fox jumped over the lazy dog | wc -w");
/// info!("There are {:?} words in above sentence", n);
///
/// // without string quotes
/// let files = run_fun!(du -ah . | sort -hr | head -n 10);
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
/// #[macro_use]
/// use cmd_lib::run_cmd;
///
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
///     date;
///     ls -l ${file};
/// };
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

#[doc(hidden)]
pub trait ProcessResult {
    fn get_result(process: &mut Process) -> Self;
}

///
/// Low level process API, wrapper on std::process module
///
/// Pipe command could also lauched in builder style
/// ```rust
/// use cmd_lib::{Process,CmdResult};
///
/// Process::new("du -ah .")
///     .pipe("sort -hr")
///     .pipe("head -n 5")
///     .wait::<CmdResult>();
/// ```
///
pub struct Process {
    cur_dir: Option<String>,
    full_cmd: Vec<Vec<String>>,
}

impl Process {
    pub fn new<S: Borrow<str>>(pipe_cmd: S) -> Self {
        let args = parse_args(pipe_cmd.borrow());
        let argv = parse_argv(args);

        Self {
            cur_dir: None,
            full_cmd: vec![argv],
        }
    }

    pub fn current_dir<S: Borrow<str>>(&mut self, dir: S) -> &mut Self {
        self.cur_dir = Some(dir.borrow().to_string());
        self
    }

    pub fn pipe<S: Borrow<str>>(&mut self, pipe_cmd: S) -> &mut Self {
        let args = parse_args(pipe_cmd.borrow());
        let argv = parse_argv(args);

        self.full_cmd.push(argv);
        self
    }

    pub fn wait<T: ProcessResult>(&mut self) -> T {
        T::get_result(self)
    }
}

impl ProcessResult for FunResult {
    fn get_result(process: &mut Process) -> Self {
        let (last_proc, full_cmd_str) = run_full_cmd(process, true)?;
        let output = last_proc.wait_with_output()?;
        if !output.status.success() {
            Err(to_io_error(&full_cmd_str, output.status))
        } else {
            let mut ans = String::from_utf8_lossy(&output.stdout).to_string();
            if ans.ends_with('\n') {
                ans.pop();
            }
            Ok(ans)
        }
    }
}

impl ProcessResult for CmdResult {
    fn get_result(process: &mut Process) -> Self {
        let (mut last_proc, full_cmd_str) = run_full_cmd(process, false)?;
        let status = last_proc.wait()?;
        if !status.success() {
            Err(to_io_error(&full_cmd_str, status))
        } else {
            Ok(())
        }
    }
}

fn format_full_cmd(full_cmd: &Vec<Vec<String>>) -> String {
    let mut full_cmd_str = String::from(full_cmd[0].join(" "));
    for cmd in full_cmd.iter().skip(1) {
        full_cmd_str += " | ";
        full_cmd_str += &cmd.join(" ");
    }
    full_cmd_str
}

fn run_full_cmd(process: &mut Process, pipe_last: bool) -> Result<(Child, String)> {
    let mut full_cmd_str = format_full_cmd(&process.full_cmd);
    let first_cmd = &process.full_cmd[0];
    let mut cmd = Command::new(&first_cmd[0]);
    if let Some(dir) = &process.cur_dir {
        full_cmd_str += &format!(" (cd: {})", dir);
        cmd.current_dir(dir);
    }

    let mut last_proc = cmd
        .args(&first_cmd[1..])
        .stdout(if pipe_last || process.full_cmd.len() > 1 {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .spawn()?;
    for (i, cmd) in process.full_cmd.iter().skip(1).enumerate() {
        let new_proc = Command::new(&cmd[0])
            .args(&cmd[1..])
            .stdin(last_proc.stdout.take().unwrap())
            .stdout(if !pipe_last && i == process.full_cmd.len() - 2 {
                Stdio::inherit()
            } else {
                Stdio::piped()
            })
            .spawn()?;
        last_proc = new_proc;
    }

    Ok((last_proc, full_cmd_str))
}

fn run_pipe_cmd(full_command: &str, cd_opt: &mut Option<String>) -> CmdResult {
    let pipe_args = parse_pipes(full_command.trim());
    let pipe_argv = parse_argv(pipe_args);

    let mut pipe_iter =  pipe_argv[0].split_whitespace();
    let cmd = pipe_iter.next().unwrap();
    if cmd == "cd" || cmd == "lcd" {
        let dir = pipe_iter.next().unwrap().trim_matches('"');
        if pipe_iter.next() != None {
            let err = Error::new(ErrorKind::Other,
            format!("{} format wrong: {}", cmd, full_command));
            return Err(err);
        } else {
            if cmd == "cd" {
                info!("Set env current_dir: \"{}\"", dir);
                return std::env::set_current_dir(dir);
            } else {
                info!("Set local current_dir: \"{}\"", dir);
                *cd_opt = Some(dir.into());
                return Ok(());
            }
        }
    } else if cmd == "pwd" {
        let pwd = std::env::current_dir()?;
        info!("Running \"pwd\" ...");
        println!("{}", pwd.display());
        return Ok(());
    }

    let mut last_proc = Process::new(pipe_argv[0].clone());
    if let Some(dir) = cd_opt {
        last_proc.current_dir(dir.clone());
    }
    for pipe_cmd in pipe_argv.iter().skip(1) {
        last_proc.pipe(pipe_cmd.clone());
    }

    last_proc.wait::<CmdResult>()
}

fn run_pipe_fun(full_command: &str) -> FunResult {
    let pipe_args = parse_pipes(full_command.trim());
    let pipe_argv = parse_argv(pipe_args);

    let mut pipe_iter =  pipe_argv[0].split_whitespace();
    let cmd = pipe_iter.next().unwrap();
    if cmd == "pwd" {
        let pwd = std::env::current_dir()?;
        return Ok(format!("{}", pwd.display()));
    }

    let mut last_proc = Process::new(pipe_argv[0].clone());
    for pipe_cmd in pipe_argv.iter().skip(1) {
        last_proc.pipe(pipe_cmd.clone());
    }

    last_proc.wait::<FunResult>()
}

#[doc(hidden)]
pub fn run_fun(cmds: &str) -> FunResult {
    run_pipe_fun(cmds)
}

#[doc(hidden)]
pub fn run_cmd(cmds: &str) -> CmdResult {
    let cmd_args = parse_cmds(cmds);
    let cmd_argv = parse_argv(cmd_args);
    let mut cd_opt: Option<String> = None;
    for cmd in cmd_argv {
        if let Err(e) = run_pipe_cmd(&cmd, &mut cd_opt) {
            return Err(e);
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

fn parse_argv(s: String) -> Vec<String> {
    s.split("\n")
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
}

#[doc(hidden)]
pub fn resolve_name(src: &str, st: &HashMap<String, String>, file: &str, line: u32) -> String {
    let mut output = String::new();
    let input: Vec<char> = src.chars().collect();
    let len = input.len();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    let mut i = 0;
    while i < len {
        if i == 0 {
            // skip variable declaration part
            while input[i] == ' ' || input[i] == '\t' || input[i] == '\n' {
                i += 1;
            }
            let first = input[i..i + 4].iter().collect::<String>();
            if i < len - 4 && first == "use " || first == "use\t" {
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

        if !in_single_quote && i < len - 2 && input[i] == '$' && input[i + 1] == '{' {
            i += 2;
            let mut var = String::new();
            while input[i] != '}' {
                var.push(input[i]);
                if input[i] == ';' || input[i] == '\n' || i == len - 1 {
                    die!("invalid name {}, {}:{}\n{}", var, file, line, src);
                }
                i += 1;
            }
            match st.get(&var) {
                None => {
                    die!("resolve {} failed, {}:{}\n{}", var, file, line, src);
                }
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
