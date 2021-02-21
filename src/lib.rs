mod parser;
mod process;
mod proc_env;
mod proc_var;

pub type FunResult = std::io::Result<String>;
pub type CmdResult = std::io::Result<()>;
pub use proc_env::Env;
pub use parser::Parser;

pub fn run_cmd<S: Into<String>>(cmds: S) -> CmdResult {
    Parser::new(cmds.into()).parse().run_cmd()
}

pub fn run_fun<S: Into<String>>(cmds: S) -> FunResult {
    Parser::new(cmds.into()).parse().run_fun()
}

pub use cmd_lib_macros::{
    run_cmd,
    run_fun,
};


// APIs For proc_macros
use std::collections::{HashMap, VecDeque};

pub fn run_cmd_with_ctx(
    code: &str,
    fn_sym_table: impl FnOnce(&mut HashMap<&str, String>),
    fn_str_lits: impl FnOnce(&mut VecDeque<String>),
) -> CmdResult {
    parse_cmds_with_ctx(code, fn_sym_table, fn_str_lits).run_cmd()
}

pub fn run_fun_with_ctx(
    code: &str,
    fn_sym_table: impl FnOnce(&mut HashMap<&str, String>),
    fn_str_lits: impl FnOnce(&mut VecDeque<String>),
) -> FunResult {
    parse_cmds_with_ctx(code, fn_sym_table, fn_str_lits).run_fun()
}

fn parse_cmds_with_ctx(
    code: &str,
    fn_sym_table: impl FnOnce(&mut HashMap<&str, String>),
    fn_str_lits: impl FnOnce(&mut VecDeque<String>),
) -> process::GroupCmds {
    let mut sym_table = HashMap::new();
    fn_sym_table(&mut sym_table);

    let mut str_lits = VecDeque::new();
    fn_str_lits(&mut str_lits);

    Parser::new(code)
        .with_sym_table(sym_table)
        .with_lits(str_lits)
        .parse()
}
