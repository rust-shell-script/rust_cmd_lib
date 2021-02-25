use cmd_lib::{
    export_cmd,
    use_cmd,
    run_cmd,
    run_fun,
    CmdArgs,
    CmdEnvs,
    FunResult,
};

#[export_cmd(my_cmd)]
fn foo(args: CmdArgs, _envs: CmdEnvs) -> FunResult {
    println!("msg from foo(), args: {:?}", args);
    Ok("bar".into())
}

fn main() {
    cmd_lib::set_debug(true);
    use_cmd!(my_cmd);
    run_cmd!(my_cmd).unwrap();
    println!("get result: {}", run_fun!(my_cmd).unwrap());
}
