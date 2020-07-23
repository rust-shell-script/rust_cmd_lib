use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::slice::Iter;
use std::iter::Peekable;
use crate::{CmdResult, FunResult};
use crate::sym_table::resolve_name;
use crate::parser;
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
    let cmd_argv = parser::parse_cmds(&cmds);
    let mut cmd_iter = cmd_argv.iter().peekable();
    let mut cmd_env = process::Env::new();
    let mut ret = String::new();
    while let Some(_) = cmd_iter.peek() {
        run_builtin_cmds(&mut cmd_iter, &mut cmd_env)?;
        if let Some(cmd) = cmd_iter.next() {
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
    let cmd_argv = parser::parse_cmds(&cmds);
    let mut cmd_iter = cmd_argv.iter().peekable();
    let mut cmd_env = process::Env::new();
    while let Some(_) = cmd_iter.peek() {
        run_builtin_cmds(&mut cmd_iter, &mut cmd_env)?;
        if let Some(cmd) = cmd_iter.next() {
            run_pipe::<CmdResult>(&cmd)?;
        }
    }
    Ok(())
}

fn run_builtin_cmds(cmd_iter: &mut Peekable<Iter<String>>, cmd_env: &mut process::Env) -> CmdResult {
    if let Some(cmd) = cmd_iter.peek() {
        let mut arg_iter = cmd.split_whitespace();
        let arg0 = arg_iter.next().unwrap();
        if arg0 == "cd" {
            let mut dir = arg_iter.next().unwrap().trim().to_owned();
            if arg_iter.next() != None {
                let err = Error::new(
                    ErrorKind::Other,
                    format!("{} format wrong: {}", arg0, cmd),
                );
                return Err(err);
            }
            // if it is relative path, always convert it to absolute one
            if !dir.starts_with("/") {
                process::ENV_VARS.with(|vars| {
                    if let Some(cmd_lib_pwd) = vars.borrow().get("PWD") {
                        dir = format!("{}/{}", cmd_lib_pwd, dir);
                    } else {
                        dir = format!("{}/{}", std::env::current_dir().unwrap().to_str().unwrap(), dir);
                    }
                });
            }
            dir = parser::trim_quotes(&dir);
            if !std::path::Path::new(&dir).exists() {
                let err_msg = format!("cd: {}: No such file or directory", dir);
                eprintln!("{}", err_msg);
                let err = Error::new(
                    ErrorKind::Other,
                    err_msg,
                );
                return Err(err);
            }
            cmd_env.set_var("PWD".to_string(), dir);
            cmd_iter.next();
        }
    }
    Ok(())
}

fn run_pipe<T: process::ProcessResult>(full_command: &str) -> T {
    let pipe_argv = parser::parse_pipes(full_command.trim());
    let mut last_proc = process::Process::new(pipe_argv[0].clone());
    for pipe_cmd in pipe_argv.iter().skip(1) {
        last_proc.pipe(pipe_cmd.clone());
    }

    last_proc.wait::<T>()
}
