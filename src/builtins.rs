use crate::{CmdEnv, CmdResult};
use std::io::{Read, Write};

#[doc(hidden)]
pub fn builtin_true(_env: &mut CmdEnv) -> CmdResult {
    Ok(())
}

#[doc(hidden)]
pub fn builtin_echo(env: &mut CmdEnv) -> CmdResult {
    let msg = env.args()[1..].join(" ");
    writeln!(env.stdout(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_info(env: &mut CmdEnv) -> CmdResult {
    let msg = env.args()[1..].join(" ");
    writeln!(env.stderr(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_warn(env: &mut CmdEnv) -> CmdResult {
    let msg = format!("WARNING: {}", env.args()[1..].join(" "));
    writeln!(env.stderr(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_err(env: &mut CmdEnv) -> CmdResult {
    let msg = format!("ERROR: {}", env.args()[1..].join(" "));
    writeln!(env.stderr(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_die(env: &mut CmdEnv) -> CmdResult {
    let msg = format!("FATAL: {}", env.args()[1..].join(" "));
    writeln!(env.stderr(), "{}", msg)
}

#[doc(hidden)]
pub fn builtin_cat(env: &mut CmdEnv) -> CmdResult {
    if env.args().len() == 1 {
        let mut buf = vec![];
        env.stdin().read_to_end(&mut buf)?;
        env.stdout().write_all(&buf)?;
        return Ok(());
    }

    let mut file = env.args()[1].clone();
    if !file.starts_with('/') && !env.current_dir().is_empty() {
        file = format!("{}/{}", env.current_dir(), file);
    }
    env.stdout().write_all(&std::fs::read(file)?)?;
    Ok(())
}
