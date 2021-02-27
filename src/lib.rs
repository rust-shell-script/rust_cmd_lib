mod parser;
mod process;
mod proc_var;

pub use cmd_lib_macros::{
    export_cmd,
    use_cmd,
    run_cmd,
    run_fun,
};
pub type FunResult = std::io::Result<String>;
pub type CmdResult = std::io::Result<()>;
pub use process::{
    CmdArgs,
    CmdEnvs,
    export_cmd,
    set_debug,
};
pub use parser::{
    ParseArg,
    Parser,
};
