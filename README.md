# Command-line library for writing rust style shell scripts

Common rust command-line macros and utilities, to write shell-script like tasks
easily in rust programming language.
Available at [crates.io](https://crates.io/crates/cmd_lib).

## run_cmd! --> CmdResult
```rust
let name = "rust";
run_cmd!(echo $name)?;
run_cmd!(|name| echo "hello, $name")?;

// pipe commands are also supported
run_cmd!(du -ah . | sort -hr | head -n 10)?;

// or a group of commands
// if any command fails, just return Err(...)
let file = "/tmp/f";
let keyword = "rust";
if run_cmd! {
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
let version = run_fun!(rustc --version).unwrap();
eprintln!("Your rust version is {}", version);

// with pipes
let n = run_fun!(echo "the quick brown fox jumped over the lazy dog" | wc -w).unwrap();
eprintln!("There are {} words in above sentence", n);
```

## Run pipe commands in the builder style

These are low level APIs, without using macros. Parameters could be
passed much clearer in this style:
```rust
Process::new("du -ah .")
    .pipe("sort -hr")
    .pipe("head -n 5")
    .wait::<CmdResult>()?;
// the same run_cmd! macro
run_cmd!(du -ah . | sort -hr | head -n 10)?;

Process::new("ls")
    .pipe("wc -l")
    .current_dir("/src/rust-shell-script/")
    .wait::<CmdResult>()?;
```

## Builtin commands
### cd
cd: set procecess current directory
```rust
run_cmd! {
    cd /tmp;
    ls | wc -l;
};
```
Notice that builtin `cd` will only change with current scope
and it will restore the previous current directory when it
exits the scope.

Use `std::env::set_current_dir` if you want to change the current
working directory for the whole program.

## Complete Example

```rust
use cmd_lib::{run_cmd, run_fun, CmdResult, FunResult};

fn foo(time: &str) -> CmdResult {
    let wait = 3;
    run_cmd!{
        sleep $wait;
        ls $f;
    }
}

fn get_year() -> FunResult {
    run_fun!(date +%Y)
}

fn main() -> CmdResult {
    run_cmd!(cd /tmp; ls | wc -l;)?;

    let name = "rust";
    run_cmd!(echo "hello, $name")?;

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
```
## Related

See [rust-shell-script](https://github.com/rust-shell-script/rust-shell-script/), which can compile
rust-shell-script scripting language directly into rust code.
