use crate::{debug, error, info, trace, warn};
use crate::{CmdEnv, CmdResult};
use std::io::{Read, Write};

pub(crate) fn builtin_echo(env: &mut CmdEnv) -> CmdResult {
    let args = env.get_args();
    let msg = if !args.is_empty() && args[0] == "-n" {
        args[1..].join(" ")
    } else {
        args.join(" ") + "\n"
    };

    write!(env.stdout(), "{}", msg)
}

pub(crate) fn builtin_error(env: &mut CmdEnv) -> CmdResult {
    error!("{}", env.get_args().join(" "));
    Ok(())
}

pub(crate) fn builtin_warn(env: &mut CmdEnv) -> CmdResult {
    warn!("{}", env.get_args().join(" "));
    Ok(())
}

pub(crate) fn builtin_info(env: &mut CmdEnv) -> CmdResult {
    info!("{}", env.get_args().join(" "));
    Ok(())
}

pub(crate) fn builtin_debug(env: &mut CmdEnv) -> CmdResult {
    debug!("{}", env.get_args().join(" "));
    Ok(())
}

pub(crate) fn builtin_trace(env: &mut CmdEnv) -> CmdResult {
    trace!("{}", env.get_args().join(" "));
    Ok(())
}

pub(crate) fn builtin_empty(env: &mut CmdEnv) -> CmdResult {
    let mut buf = vec![];
    env.stdin().read_to_end(&mut buf)?;
    env.stdout().write_all(&buf)?;
    Ok(())
}
