# cmd_lib - Rust command line library

Common rust command line macros and utils, to write shell script like tasks
easily in rust programming language.


## run_cmd! --> CmdResult
```rust
let name = "rust";
run_cmd!("echo hello, {}", name);

// pipe commands are also supported
run_cmd!("du -ah . | sort -hr | head -n 10");

// also work without string quote
run_cmd!(du -ah . | sort -hr | head -n 10);

// or a group of commands
// if any command fails, just return Err(...)
if run_cmd! {
    ls / | wc -w
    echo "bad cmd"
    ls -l /nofile
    date
}.is_err() {
    warn!("Run group command failed");
}
```

## run_fun! --> FunResult
```rust
let version = run_fun!("rustc --version")?;
info!("Your rust version is {}", version.trim());

// with pipes
let n = run_fun!("echo the quick brown fox jumped over the lazy dog | wc -w")?;
info!("There are {} words in above sentence", n.trim());
```

## Easy Reporting
```rust
info!("Running command xxx ...");
warn!("Running command failed");
err!("Copying failed");
die!("Command exit unexpectedly: {}", reason);
```
output:
```bash
INFO: Running command xxx ...
WARN: Running command failed
ERROR: Copying file failed
FATAL: Command exit unexpectedly: disk is full
```

## Complete Example

```rust
use cmd_lib::{info, warn, run_cmd, run_fun, CmdResult, FunResult};

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

    if foo().is_err() {
        warn!("Failed to run foo()");
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
INFO: Running "du -ah . | sort -hr | head -n 5" ...
INFO: Top 5 directories:
5.1M    .
2.7M    ./main
2.4M    ./main2
8.0K    ./lib.rs
4.0K    ./main.rs
INFO: Running "sleep 3" ...
INFO: Running "ls /nofile" ...
ls: cannot access '/nofile': No such file or directory
WARN: Failed to run foo()
INFO: Running "date +%Y" ...
INFO: You are in year 2019
```

## Related

See [rust-shell-script](https://github.com/rust-shell-script/rust-shell-script/), which can compile
rust-shell-script scripting language directly into rust code.
