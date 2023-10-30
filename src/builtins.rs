use crate::{debug, error, info, trace, warn};
use crate::{CmdEnv, CmdResult};
use std::io::{Read, Write};

pub(crate) fn builtin_echo(env: &mut CmdEnv) -> CmdResult {
    let args = env.args();
    let msg = if args.len() > 1 && args[1] == "-n" {
        args[2..].join(" ")
    } else {
        args[1..].join(" ") + "\n"
    };

    write!(env.stdout(), "{}", msg)
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

pub(crate) fn builtin_empty(env: &mut CmdEnv) -> CmdResult {
    let mut buf = vec![];
    env.stdin().read_to_end(&mut buf)?;
    env.stdout().write_all(&buf)?;
    Ok(())
}
