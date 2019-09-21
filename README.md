# rust_cmd_lib - Rust command line library

Common rust command line macros and utils, to write shell script like tasks
easy in rust programming language.


## run_cmd! --> CmdResult
```
let name = "rust";
run_cmd!("echo hello, {}", name);
```

## run_fun! --> FunResult
```
let version = run_fun!("rustc --version")?;
info!("Your rust version is {}", version);
```

## Complete example

```rustmod cmd_lib;
use cmd_lib::{CmdResult, FunResult};

fn foo() -> CmdResult {
    run_cmd!("sleep 3")?;
    run_cmd!("ls /nofile")?;
    Ok(())
}

fn get_year() -> FunResult {
    run_fun!("date +%Y")
}

fn main() -> CmdResult {
    if !foo().is_ok() {
        info!("Failed to run foo()");
    }

    if get_year()?.trim() == "2019" {
        info!("You are in year 2019");
    } else {
        info!("Which year are you in ?");
    }

    Ok(())
}
```

output:
```bash
Running ["sleep", "3"] ...
Running ["ls", "/nofile"] ...
ls: cannot access '/nofile': No such file or directory
Failed to run foo()
Running ["date", "+%Y"] ...
You are in year 2019
```

## Related

See [rust-shell-script](https://github.com/rust-shell-script/rust-shell-script/), which can compile
rust-shell-script scripting language directly into rust code.
