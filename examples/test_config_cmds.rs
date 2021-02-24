use cmd_lib::{
    cmd,
    config_cmd,
    debug_cmd,
    run_cmd,
    run_fun,
    CmdArgs,
    FunResult,
};

#[cmd(my_cmd)]
fn foo(args: CmdArgs) -> FunResult {
    println!("msg from foo(), args: {:?}", args);
    Ok("bar".into())
}

fn main() {
    debug_cmd(true);
    config_cmd!(my_cmd);
    run_cmd!(my_cmd).unwrap();
    println!("get result: {}", run_fun!(my_cmd).unwrap());
}
