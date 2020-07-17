use cmd_lib::{run_cmd, run_fun, sh, CmdResult, FunResult};

sh! {
    fn foo() -> CmdResult {
        let dir = "/var/tmp";
        let f = "nofile";
        let gap = 3;

        #(cd $dir)?;
        #(sleep $gap)?;
        #(ls $f)?;
        Ok(())
    }

    fn get_year() -> FunResult {
        run_fun!(date +%Y)
    }
}

fn main() -> CmdResult {
    run_cmd!(lcd /tmp; ls | wc -l;)?;
    run_cmd!(pwd)?;

    let name = "rust";
    run_cmd!(echo $name)?;
    run_cmd!(|name| echo "hello, $name")?;

    let result = run_fun!(du -ah . | sort -hr | head -n 5)?;
    eprintln!("Top 5 directories:\n{}", result);

    if foo().is_err() {
        eprintln!("Failed to run foo()");
    }

    if get_year()? == "2020" {
        eprintln!("You are in year 2020");
    } else {
        eprintln!("Which year are you in ?");
    }

    Ok(())
}
