use cmd_lib::{run_cmd, run_fun, CmdResult};

fn main() -> CmdResult {
    cmd_lib::set_debug(true);
    if run_cmd!(false | wc).is_err() {
        eprintln!("running pipe failed");
    }
    let _result = run_fun!(du -ah . | sort -hr | head -n 5)?;
    run_cmd!(echo xx | false | wc | wc | wc)?;
    Ok(())
}
