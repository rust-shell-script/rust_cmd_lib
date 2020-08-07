use std::io::{Error, ErrorKind};
use std::slice::Iter;
use std::iter::Peekable;
use crate::{
    CmdResult,
    FunResult,
    process,
};

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
           $crate::Parser::new($crate::source_text!(run_fun).clone())
           .with_lits($crate::parse_string_literal!($($cur)*))
           .with_sym_table($crate::parse_sym_table!($($cur)*))
           .with_location(file!(), line!())
           .parse())
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
           $crate::Parser::new($crate::source_text!(run_cmd).clone())
           .with_lits($crate::parse_string_literal!($($cur)*))
           .with_sym_table($crate::parse_sym_table!($($cur)*))
           .with_location(file!(), line!())
           .parse())
   };
}

#[doc(hidden)]
pub fn run_fun(cmds: Vec<Vec<Vec<String>>>) -> FunResult {
    let mut cmd_iter = cmds.iter().peekable();
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
pub fn run_cmd(cmds: Vec<Vec<Vec<String>>>) -> CmdResult {
    let mut cmd_iter = cmds.iter().peekable();
    let mut cmd_env = process::Env::new();
    while let Some(_) = cmd_iter.peek() {
        run_builtin_cmds(&mut cmd_iter, &mut cmd_env)?;
        if let Some(cmd) = cmd_iter.next() {
            run_pipe::<CmdResult>(&cmd)?;
        }
    }
    Ok(())
}

fn run_builtin_cmds(cmd_iter: &mut Peekable<Iter<Vec<Vec<String>>>>, cmd_env: &mut process::Env) -> CmdResult {
    if let Some(cmd) = cmd_iter.peek() {
        let arg0 = cmd[0][0].clone();
        if arg0 == "cd" {
            let mut dir = cmd[0][1].clone();
            if cmd[0].len() != 2 {
                let err = Error::new(
                    ErrorKind::Other,
                    format!("cd format wrong: {}", cmd[0].join(" ")),
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

fn run_pipe<T: process::ProcessResult>(pipes: &Vec<Vec<String>>) -> T {
    let mut proc = process::Process::new(&pipes[0]);
    for p in pipes.into_iter().skip(1) {
        proc.pipe(p);
    }

    proc.wait::<T>()
}
