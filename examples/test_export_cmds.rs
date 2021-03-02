use cmd_lib::{
    export_cmd, run_cmd, run_fun, use_builtin_cmd, use_custom_cmd, CmdArgs, CmdEnvs, CmdResult,
    FunResult,
};

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

fn main() -> CmdResult {
    cmd_lib::set_debug(true);
    use_builtin_cmd!(echo, true);
    use_custom_cmd!(my_cmd, my_cmd2);
    run_cmd!(echo "from" "builtin")?;
    run_cmd!(my_cmd arg1 arg2)?;
    run_cmd!(my_cmd2)?;
    println!("get result: {}", run_fun!(my_cmd)?);
    Ok(())
}
