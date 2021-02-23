use std::collections::HashMap;
use cmd_lib::{
    cmd,
    config_cmd,
    CmdResult,
};

#[cmd(ls)]
fn foo(args: Option<Vec<String>>, envs: Option<HashMap<String, String>>) -> CmdResult {
    println!("msg from foo(), args: {:?}, envs: {:?}", args, envs);
    Ok(())
}

fn main() {
    config_cmd!(ls);
}
