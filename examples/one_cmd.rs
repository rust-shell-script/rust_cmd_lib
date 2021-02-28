// use std::fs;
use cmd_lib::{run_cmd, CmdResult};

fn main() -> CmdResult {
    cmd_lib::set_debug(true);
    let msg = "rust";
    run_cmd!(echo $msg)?;
    // let f = "/tmp/test_rust_cmd_lib.sh";
    // let content = "#!/bin/bash\n echo \"FOO=$FOO from /tmp/test.sh\"";
    // fs::write(f, content).unwrap();
    // run_cmd!(chmod +x $f)?;
    // run_cmd!(FOO=100 $f)?;
    // run_cmd!(rm -f $f)?;
    Ok(())
}
