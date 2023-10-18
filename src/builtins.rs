use crate::logger::try_init_default_logger;
use crate::{CmdEnv, CmdResult};
use log::*;
use std::io::Write;

pub(crate) fn builtin_echo(env: &mut CmdEnv) -> CmdResult {
    let msg = env.args()[1..].join(" ");
    writeln!(env.stdout(), "{}", msg)
}

pub(crate) fn builtin_die(env: &mut CmdEnv) -> CmdResult {
    let _ = try_init_default_logger();
    error!("FATAL: {}", env.args()[1..].join(" "));
    std::process::exit(1);
}

pub(crate) fn builtin_error(env: &mut CmdEnv) -> CmdResult {
    let _ = try_init_default_logger();
    error!("{}", env.args()[1..].join(" "));
    Ok(())
}

pub(crate) fn builtin_warn(env: &mut CmdEnv) -> CmdResult {
    let _ = try_init_default_logger();
    warn!("{}", env.args()[1..].join(" "));
    Ok(())
}

pub(crate) fn builtin_info(env: &mut CmdEnv) -> CmdResult {
    let _ = try_init_default_logger();
    info!("{}", env.args()[1..].join(" "));
    Ok(())
}

pub(crate) fn builtin_debug(env: &mut CmdEnv) -> CmdResult {
    let _ = try_init_default_logger();
    debug!("{}", env.args()[1..].join(" "));
    Ok(())
}

pub(crate) fn builtin_trace(env: &mut CmdEnv) -> CmdResult {
    let _ = try_init_default_logger();
    trace!("{}", env.args()[1..].join(" "));
    Ok(())
}
