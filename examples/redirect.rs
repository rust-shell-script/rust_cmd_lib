use cmd_lib::{ CmdResult, run_cmd };
fn main() -> CmdResult {
    run_cmd!(echo xxxx > /tmp/f)?;
    run_cmd!(echo yyyy >> /tmp/f)?;
    run_cmd!(cat /tmp/f >&2)?;

    Ok(())
}
