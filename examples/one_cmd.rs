use cmd_lib::{run_cmd, CmdResult};

fn main() -> CmdResult {
    cmd_lib::set_debug(true);
    let dir = "/tmp";
    // run_cmd!(ls -l -a "/tmp");
    run_cmd!(ls -l /var/"tmp");
    Ok(())
}
