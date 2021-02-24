use cmd_lib::{
    cmd,
    config_cmd,
    debug_cmd,
    CmdArgs,
    FunResult,
};

#[cmd(ls)]
fn foo(args: CmdArgs) -> FunResult {
    println!("msg from foo(), args: {:?}", args);
    Ok("".into())
}

fn main() {
    debug_cmd(true);
    config_cmd!(ls);
}
