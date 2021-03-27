use crate::{CmdArgs, CmdEnvs, CmdResult, CmdStdio};
use std::io::{Read, Write};

#[doc(hidden)]
pub fn builtin_true(_args: CmdArgs, _envs: CmdEnvs, _io: &mut CmdStdio) -> CmdResult {
    Ok(())
}

#[doc(hidden)]
pub fn builtin_echo(args: CmdArgs, _envs: CmdEnvs, io: &mut CmdStdio) -> CmdResult {
    let msg = args[1..].join(" ");
    writeln!(io.stdout(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_info(args: CmdArgs, _envs: CmdEnvs, io: &mut CmdStdio) -> CmdResult {
    let msg = args[1..].join(" ");
    writeln!(io.stderr(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_warn(args: CmdArgs, _envs: CmdEnvs, io: &mut CmdStdio) -> CmdResult {
    let msg = format!("WARNING: {}", args[1..].join(" "));
    writeln!(io.stderr(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_err(args: CmdArgs, _envs: CmdEnvs, io: &mut CmdStdio) -> CmdResult {
    let msg = format!("ERROR: {}", args[1..].join(" "));
    writeln!(io.stderr(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_die(args: CmdArgs, _envs: CmdEnvs, io: &mut CmdStdio) -> CmdResult {
    let msg = format!("FATAL: {}", args[1..].join(" "));
    writeln!(io.stderr(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_cat(args: CmdArgs, _envs: CmdEnvs, io: &mut CmdStdio) -> CmdResult {
    if args.len() == 1 {
        let mut buf = vec![];
        io.stdin().read_to_end(&mut buf)?;
        io.stdout().write_all(&buf)?;
        return Ok(());
    }

    io.stdout().write_all(&std::fs::read(&args[1])?)?;
    Ok(())
}
