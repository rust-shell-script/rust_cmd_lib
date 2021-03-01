mod parser;
mod proc_var;
mod process;

pub use cmd_lib_macros::{export_cmd, run_cmd, run_fun, use_cmd};
pub type FunResult = std::io::Result<String>;
pub type CmdResult = std::io::Result<()>;
pub use parser::{ParseArg, Parser};
pub use process::{export_cmd, set_debug, CmdArgs, CmdEnvs};
