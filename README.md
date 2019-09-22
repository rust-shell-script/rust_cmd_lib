# cmd_lib - Rust command line library

Common rust command line macros and utils, to write shell script like tasks
easily in rust programming language.


## run_cmd! --> CmdResult
```
let name = "rust";
run_cmd!("echo hello, {}", name);

// pipe commands are also supported
run_cmd!("du -ah . | sort -hr | head -n 10");
```

## run_fun! --> FunResult
```
let version = run_fun!("rustc --version")?;
info!("Your rust version is {}", version.trim());

// with pipes
let n = run_fun!("echo the quick brown fox jumped over the lazy dog | wc -w")?;
info!("There are {} words in above sentence", n.trim());
```

## Complete example

```rust
use cmd_lib::{info, run_cmd, run_fun, CmdResult, FunResult};

fn foo() -> CmdResult {
    run_cmd!("sleep 3")?;
    run_cmd!("ls /nofile")?;
    Ok(())
}

fn get_year() -> FunResult {
    run_fun!("date +%Y")
}

fn main() -> CmdResult {
    let result = run_fun!("du -ah . | sort -hr | head -n 5")?;
    info!("Top 5 directories:\n{}", result.trim());

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
Running "du -ah . | sort -hr | head -n 5" ...
Top 5 directories:
18M .
2.7M    ./main
2.6M    ./pipe3
2.6M    ./pipe2
2.6M    ./echo
Running "sleep 3" ...
Running "ls /nofile" ...
ls: cannot access '/nofile': No such file or directory
Failed to run foo()
Running "date +%Y" ...
You are in year 2019
```

## Related

See [rust-shell-script](https://github.com/rust-shell-script/rust-shell-script/), which can compile
rust-shell-script scripting language directly into rust code.
