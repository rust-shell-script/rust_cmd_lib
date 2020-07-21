use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use crate::{CmdResult, FunResult};
use crate::sym_table::resolve_name;
use crate::parser::{parse_cmds, parse_pipes};
use crate::process;

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
       $crate::run_fun(
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
       $crate::run_cmd(
           &$crate::source_text!(run_cmd),
           &$crate::parse_sym_table!($($cur)*),
           &file!(),
           line!())
   };
}

#[doc(hidden)]
// TODO: clean up with run_cmd
pub fn run_fun(
    cmd: &str,
    sym_table: &HashMap<String, String>,
    file: &str,
    line: u32,
) -> FunResult {
    let cmds = resolve_name(&cmd, &sym_table, &file, line);
    let cmd_argv = parse_cmds(&cmds);
    let mut ret = String::new();
    let mut cmd_env = process::Env::new();
    for cmd in cmd_argv {
        let mut cmd_iter = cmd.split_whitespace();
        let cmd0 = cmd_iter.next().unwrap();
        if cmd0 == "cd" {
            let dir = cmd_iter.next().unwrap().trim();
            if cmd_iter.next() != None {
                let err = Error::new(
                    ErrorKind::Other,
                    format!("{} format wrong: {}", cmd0, cmd),
                );
                return Err(err);
            }
            cmd_env.set("PWD".to_string(), dir.to_string());
        } else {
            ret = run_pipe::<FunResult>(&cmd)?;
        }
    }
    Ok(ret)
}

#[doc(hidden)]
pub fn run_cmd(
    cmd: &str,
    sym_table: &HashMap<String, String>,
    file: &str,
    line: u32,
) -> CmdResult {
    let cmds = resolve_name(&cmd, &sym_table, &file, line);
    let cmd_argv = parse_cmds(&cmds);
    let mut cmd_env = process::Env::new();
    for cmd in cmd_argv {
        let mut cmd_iter = cmd.split_whitespace();
        let cmd0 = cmd_iter.next().unwrap();
        if cmd0 == "cd" {
            let dir = cmd_iter.next().unwrap().trim();
            if cmd_iter.next() != None {
                let err = Error::new(
                    ErrorKind::Other,
                    format!("{} format wrong: {}", cmd0, cmd),
                );
                return Err(err);
            }
            cmd_env.set("PWD".to_string(), dir.to_string());
        } else {
            run_pipe::<CmdResult>(&cmd)?;
        }
    }
    Ok(())
}

fn run_pipe<T: process::ProcessResult>(full_command: &str) -> T {
    let pipe_argv = parse_pipes(full_command.trim());

    let mut last_proc = process::Process::new(pipe_argv[0].clone());
    for pipe_cmd in pipe_argv.iter().skip(1) {
        last_proc.pipe(pipe_cmd.clone());
    }

    last_proc.wait::<T>()
}
