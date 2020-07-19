pub mod source_text;
pub mod sym_table;
pub mod process;
pub mod parser;
pub mod cmd_fun;

pub type FunResult = std::io::Result<String>;
pub type CmdResult = std::io::Result<()>;
pub use cmd_fun::run_cmd;
pub use cmd_fun::run_fun;
pub use process::Process;
