use crate::{CmdEnv, CmdResult};
use log::*;
use std::io::{Read, Write};
use std::path::PathBuf;

#[doc(hidden)]
pub fn builtin_echo(env: &mut CmdEnv) -> CmdResult {
    let msg = env.args()[1..].join(" ");
    writeln!(env.stdout(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_die(env: &mut CmdEnv) -> CmdResult {
    error!("FATAL: {}", env.args()[1..].join(" "));
    std::process::exit(1);
}

#[doc(hidden)]
pub fn builtin_error(env: &mut CmdEnv) -> CmdResult {
    error!("{}", env.args()[1..].join(" "));
    Ok(())
}

#[doc(hidden)]
pub fn builtin_warn(env: &mut CmdEnv) -> CmdResult {
    warn!("{}", env.args()[1..].join(" "));
    Ok(())
}

#[doc(hidden)]
pub fn builtin_info(env: &mut CmdEnv) -> CmdResult {
    info!("{}", env.args()[1..].join(" "));
    Ok(())
}

#[doc(hidden)]
pub fn builtin_debug(env: &mut CmdEnv) -> CmdResult {
    debug!("{}", env.args()[1..].join(" "));
    Ok(())
}

#[doc(hidden)]
pub fn builtin_trace(env: &mut CmdEnv) -> CmdResult {
    trace!("{}", env.args()[1..].join(" "));
    Ok(())
}

#[doc(hidden)]
pub fn builtin_cat(env: &mut CmdEnv) -> CmdResult {
    if env.args().len() == 1 {
        let mut buf = vec![];
        env.stdin().read_to_end(&mut buf)?;
        env.stdout().write_all(&buf)?;
        return Ok(());
    }

    let mut file = PathBuf::from(env.args()[1].to_owned());
    if file.is_relative() {
        file = PathBuf::from(env.current_dir()).join(file);
    }
    env.stdout().write_all(&std::fs::read(file)?)?;
    Ok(())
}
