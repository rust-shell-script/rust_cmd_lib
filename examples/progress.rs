use cmd_lib::{run_cmd, CmdResult};

#[cmd_lib::main]
fn main() -> CmdResult {
    run_cmd!(dd if=/dev/urandom of=/dev/null bs=1M status=progress)?;

    Ok(())
}
