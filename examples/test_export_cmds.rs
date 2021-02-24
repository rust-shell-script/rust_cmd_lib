use cmd_lib::{
    export_cmd,
    use_cmd,
    debug_cmd,
    run_cmd,
    run_fun,
    CmdArgs,
    FunResult,
};

#[export_cmd(my_cmd)]
fn foo(args: CmdArgs) -> FunResult {
    println!("msg from foo(), args: {:?}", args);
    Ok("bar".into())
}

fn main() {
    debug_cmd(true);
    use_cmd!(my_cmd);
    run_cmd!(my_cmd).unwrap();
    println!("get result: {}", run_fun!(my_cmd).unwrap());
}
