use cmd_lib::{CmdResult, run_cmd};

#[cmd_lib::main]
fn main() -> CmdResult {
    run_cmd!(dd if=/dev/urandom of=/dev/null bs=1M status=progress)?;

    Ok(())
}
