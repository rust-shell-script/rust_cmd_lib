use crate::{CmdEnv, CmdResult};
use log::*;
use std::io::Write;

pub(crate) fn builtin_echo(env: &mut CmdEnv) -> CmdResult {
    let msg = env.args()[1..].join(" ");
    writeln!(env.stdout(), "{}", msg)
}

pub(crate) fn builtin_die(env: &mut CmdEnv) -> CmdResult {
    error!("FATAL: {}", env.args()[1..].join(" "));
    std::process::exit(1);
}

pub(crate) fn builtin_error(env: &mut CmdEnv) -> CmdResult {
    error!("{}", env.args()[1..].join(" "));
    Ok(())
}

pub(crate) fn builtin_warn(env: &mut CmdEnv) -> CmdResult {
    warn!("{}", env.args()[1..].join(" "));
    Ok(())
}

pub(crate) fn builtin_info(env: &mut CmdEnv) -> CmdResult {
    info!("{}", env.args()[1..].join(" "));
    Ok(())
}

pub(crate) fn builtin_debug(env: &mut CmdEnv) -> CmdResult {
    debug!("{}", env.args()[1..].join(" "));
    Ok(())
}

pub(crate) fn builtin_trace(env: &mut CmdEnv) -> CmdResult {
    trace!("{}", env.args()[1..].join(" "));
    Ok(())
}
