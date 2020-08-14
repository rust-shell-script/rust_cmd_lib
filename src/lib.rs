pub(crate) mod source_text;
pub(crate) mod sym_table;
pub(crate) mod process;
pub(crate) mod parser;
pub(crate) mod cmd_fun;
pub(crate) mod proc_env;
pub(crate) mod proc_var;

pub type FunResult = std::io::Result<String>;
pub type CmdResult = std::io::Result<()>;
pub use proc_env::Env;
pub use parser::Parser;
