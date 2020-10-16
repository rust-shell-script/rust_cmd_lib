pub use cmd_lib_core;
pub use cmd_lib_macros;

#[macro_export]
macro_rules! run_cmd {
    ($($cur:tt)*) => {{
	use $crate::cmd_lib_core;
	$crate::cmd_lib_macros::run_cmd!($($cur)*)
    }};
}

#[macro_export]
macro_rules! run_fun {
    ($($cur:tt)*) => {{
	use $crate::cmd_lib_core;
	$crate::cmd_lib_macros::run_fun!($($cur)*)
    }};
}

pub use cmd_lib_core::{
    run_cmd,
    run_fun,
    CmdResult,
    FunResult,
    proc_env_set,
    proc_var,
    proc_var_get,
    proc_var_set,
};
