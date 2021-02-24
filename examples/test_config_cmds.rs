use std::collections::HashMap;
use cmd_lib::{
    cmd,
    config_cmd,
    FunResult,
};

#[cmd(ls)]
fn foo(args: Option<&Vec<&str>>, envs: Option<&HashMap<&str, &str>>) -> FunResult {
    println!("msg from foo(), args: {:?}, envs: {:?}", args, envs);
    Ok("".into())
}

fn main() {
    config_cmd!(ls);
}
