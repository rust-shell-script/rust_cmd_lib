use crate::{CmdArgs, CmdEnvs, FunResult};

#[doc(hidden)]
pub fn builtin_echo(args: CmdArgs, _envs: CmdEnvs) -> FunResult {
    Ok(args[1..].join(" "))
}

#[doc(hidden)]
pub fn builtin_true(_args: CmdArgs, _envs: CmdEnvs) -> FunResult {
    Ok("".into())
}
