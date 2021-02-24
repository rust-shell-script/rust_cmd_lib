use cmd_lib::{
    cmd,
    config_cmd,
    debug_cmd,
    run_cmd,
    CmdArgs,
    FunResult,
};

#[cmd(my_cmd)]
fn foo(args: CmdArgs) -> FunResult {
    println!("msg from foo(), args: {:?}", args);
    Ok("".into())
}

fn main() {
    debug_cmd(true);
    config_cmd!(my_cmd);
    run_cmd!(my_cmd).unwrap();
}
