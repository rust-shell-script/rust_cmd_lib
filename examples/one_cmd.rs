use cmd_lib::{run_cmd, CmdResult};

fn main() -> CmdResult {
    cmd_lib::set_debug(true);
    let f = "/tmp/test_rust_cmd_lib.sh";
    let content = "#!/bin/bash\necho \"FOO=$FOO from /tmp/test.sh\"";
    run_cmd!(touch $f)?;
    run_cmd!(echo $content > $f)?;
    run_cmd!(echo "echo hello" >> $f)?;
    run_cmd!(chmod +x $f)?;
    run_cmd!(FOO=100 $f)?;
    run_cmd!(rm -f $f)?;
    Ok(())
}
