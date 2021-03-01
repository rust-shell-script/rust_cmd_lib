use cmd_lib::{run_cmd, run_fun, CmdResult, FunResult};

fn foo() -> CmdResult {
    let dir = "src";
    let f = "nofile";
    let gap = 3;

    run_cmd! {
        cd $dir;    // change current directory
        pwd;        // print pwd
        sleep $gap;
        cd $f;
    }
}

fn get_year() -> FunResult {
    run_fun!(date +%Y)
}

fn main() -> CmdResult {
    cmd_lib::set_debug(true);
    run_cmd!(ls /tmp/nofile || true; echo "continue")?;
    run_cmd!(cd /tmp; ls | wc -l;)?;
    run_cmd!(pwd)?;

    let name = "rust";
    run_cmd!(echo $name)?;
    run_cmd!(echo "hello, $name")?;
    run_cmd!(du -ah . | sort -hr | head -n 5 | wc -w)?;

    let result = run_fun!(du -ah . | sort -hr | head -n 5)?;
    eprintln!("Top 5 directories:\n{}", result);

    if foo().is_err() {
        eprintln!("Failed to run foo()");
    }

    if get_year()? == "2021" {
        eprintln!("You are in year 2021");
    } else {
        eprintln!("Which year are you in ?");
    }

    Ok(())
}
