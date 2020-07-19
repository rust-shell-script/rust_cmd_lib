pub(crate) mod source_text;
pub(crate) mod sym_table;
pub(crate) mod process;
pub(crate) mod parser;
pub mod cmd_fun;

pub type FunResult = std::io::Result<String>;
pub type CmdResult = std::io::Result<()>;
pub use cmd_fun::run_cmd;
pub use cmd_fun::run_fun;
pub use process::Process;
