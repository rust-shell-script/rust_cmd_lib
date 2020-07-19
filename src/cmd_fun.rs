use std::collections::HashMap;
use crate::{CmdResult, FunResult};
use std::io::{Error, ErrorKind};
use crate::sym_table::resolve_name;
use crate::parser::{parse_cmds, parse_argv, parse_pipes};
use crate::process::Process;

/// ## run_fun! --> FunResult
/// ```no_run
/// #[macro_use]
/// use cmd_lib::run_fun;
/// let version = run_fun!(rustc --version).unwrap();
/// eprintln!("Your rust version is {}", version);
///
/// // with pipes
/// let files = run_fun!(du -ah . | sort -hr | head -n 10).unwrap();
/// eprintln!("files: {}", files);
/// ```
#[macro_export]
macro_rules! run_fun {
   ($($cur:tt)*) => {
       $crate::cmd_fun::run_fun_with_sym_table(
           &$crate::source_text!(run_fun),
           &$crate::parse_sym_table!($($cur)*),
           &file!(),
           line!())
   };
}

///
/// ## run_cmd! --> CmdResult
/// ```rust
/// #[macro_use]
/// use cmd_lib::run_cmd;
///
/// let name = "rust";
/// run_cmd!(echo $name);
/// run_cmd!(|name| echo "hello, $name");
///
/// // pipe commands are also supported
/// run_cmd!(du -ah . | sort -hr | head -n 10);
///
/// // or a group of commands
/// // if any command fails, just return Err(...)
/// let file = "/tmp/f";
/// run_cmd!{
///     date;
///     ls -l $file;
/// };
/// ```
#[macro_export]
macro_rules! run_cmd {
   ($($cur:tt)*) => {
       $crate::cmd_fun::run_cmd_with_sym_table(
           &$crate::source_text!(run_cmd),
           &$crate::parse_sym_table!($($cur)*),
           &file!(),
           line!())
   };
}

#[doc(hidden)]
pub fn run_fun_with_sym_table(
    fun: &str,
    sym_table: &HashMap<String, String>,
    file: &str,
    line: u32,
) -> FunResult {
    run_fun(&resolve_name(&fun, &sym_table, &file, line))
}

#[doc(hidden)]
pub fn run_cmd_with_sym_table(
    cmd: &str,
    sym_table: &HashMap<String, String>,
    file: &str,
    line: u32,
) -> CmdResult {
    run_cmd(&resolve_name(&cmd, &sym_table, &file, line))
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

fn run_pipe_cmd(full_command: &str, cd_opt: &mut Option<String>) -> CmdResult {
    let pipe_args = parse_pipes(full_command.trim());
    let pipe_argv = parse_argv(pipe_args);

    let mut pipe_iter = pipe_argv[0].split_whitespace();
    let cmd = pipe_iter.next().unwrap();
    if cmd == "cd" || cmd == "lcd" {
        let dir = pipe_iter.next().unwrap().trim();
        if pipe_iter.next() != None {
            let err = Error::new(
                ErrorKind::Other,
                format!("{} format wrong: {}", cmd, full_command),
            );
            return Err(err);
        } else {
            if cmd == "cd" {
                return std::env::set_current_dir(dir);
            } else {
                *cd_opt = Some(dir.into());
                return Ok(());
            }
        }
    } else if cmd == "pwd" {
        let pwd = std::env::current_dir()?;
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

    let mut pipe_iter = pipe_argv[0].split_whitespace();
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
