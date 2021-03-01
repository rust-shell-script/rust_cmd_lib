use cmd_lib::{export_cmd, run_cmd, run_fun, use_cmd, CmdArgs, CmdEnvs, FunResult};

#[export_cmd(my_cmd)]
fn foo(args: CmdArgs, _envs: CmdEnvs) -> FunResult {
    eprintln!("msg from foo(), args: {:?}", args);
    Ok("bar".into())
}

#[export_cmd(my_cmd2)]
fn foo2(args: CmdArgs, _envs: CmdEnvs) -> FunResult {
    eprintln!("msg from foo2(), args: {:?}", args);
    Ok("bar2".into())
}

fn main() {
    cmd_lib::set_debug(true);
    use_cmd!(my_cmd, my_cmd2);
    #[rustfmt::skip]
    run_cmd!(my_cmd -a).unwrap();
    run_cmd!(my_cmd2).unwrap();
    println!("get result: {}", run_fun!(my_cmd).unwrap());
}
