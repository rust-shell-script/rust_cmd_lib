use cmd_lib::{ CmdResult, run_cmd };
fn main() -> CmdResult {
    eprintln!("echo xxxx to /tmp/f");
    run_cmd!(echo xxxx > /tmp/f)?;

    eprintln!("append yyyy to /tmp/f");
    run_cmd!(echo yyyy >> /tmp/f)?;

    eprintln!("check /tmp/f");
    run_cmd!(cat /tmp/f).unwrap();

    eprintln!("redirect stderr to /tmp/f");
    run_cmd!(
        ls /x 2>/tmp/lsx.log || true;
        echo "dump file:";
        cat /tmp/lsx.log;
    )?;

    eprintln!("redirect stderr to /dev/null");
    run_cmd!(ls /x 2>/dev/null || true)?;

    eprintln!("redirect stdout and stderr to /tmp/f");
    run_cmd!(ls /x &>/tmp/f || true)?;

    eprintln!("redirect stdin from /tmp/f");
    run_cmd!(wc -w < /tmp/f)?;

    Ok(())
}
