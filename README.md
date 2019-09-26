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
    use keyword, file;

    cat ${file} | grep ${keyword};
    echo "bad cmd";
    ls -l /nofile;
    date;
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
use cmd_lib::{info, warn, output, run_cmd, run_fun, CmdResult, FunResult};

fn foo() -> CmdResult {
    let f = "/var/tmp/nofile";
    run_cmd!{
        use f;
        sleep 3;
        ls ${f};
    }
}

fn get_year() -> FunResult {
    let year = run_fun!("date +%Y")?;
    output!("{}", year.trim())
}

fn main() -> CmdResult {
    let name = "rust";
    run_cmd!("echo hello, {}", name);

    let result = run_fun!("du -ah . | sort -hr | head -n 5")?;
    info!("Top 5 directories:\n{}", result.trim());

    if foo().is_err() {
        warn!("Failed to run foo()");
    }

    if get_year()? == "2019" {
        info!("You are in year 2019");
    } else {
        info!("Which year are you in ?");
    }

    Ok(())
}
```

output:
```bash
INFO: Running "echo hello, rust" ...
hello, rust
INFO: Running "du -ah . | sort -hr | head -n 5" ...
INFO: Top 5 directories:
318M    .
313M    ./target
205M    ./target/debug
127M    ./target/debug/incremental
91M ./target/package
INFO: Running "sleep 3" ...
INFO: Running "ls "/var/tmp/nofile"" ...
ls: cannot access '/var/tmp/nofile': No such file or directory
WARN: Failed to run foo()
INFO: Running "date +%Y" ...
INFO: You are in year 2019
```

## Related

See [rust-shell-script](https://github.com/rust-shell-script/rust-shell-script/), which can compile
rust-shell-script scripting language directly into rust code.
