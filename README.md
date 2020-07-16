# Command-line library for writing rust style shell scripts

Common rust command-line macros and utilities, to write shell-script like tasks
easily in rust programming language.
Available at [crates.io](https://crates.io/crates/cmd_lib).

## run_cmd! --> CmdResult
```rust
let name = "rust";
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
let version = run_fun!(rustc --version)?;
info!("Your rust version is {}", version);

// with pipes
let n = run_fun!(echo "the quick brown fox jumped over the lazy dog" | wc -w)?;
info!("There are {} words in above sentence", n);
```

## sh! to write shell-like code
```rust
sh! {
    fn foo() -> CmdResult {
        let file = "/tmp/f";
        #(ls $file)?;
        Ok(())
    }

    fn bar(a: &str) -> FunResult {
        $(date +%Y)
    }
}
```
or inside a function:
```rust
fn main() -> CmdResult {
    ...
    sh! {
        #(mkfs.ext3 -b 4096 /dev/sda)?;
        if #(ls /tmp).is_ok() {
            println!("running cmd success");
        } else {
            println!("running cmd failed");
        }
    }
}
```

## Run pipe commands in the builder style

parameters could be passed much clearer in this style
```rust
Process::new("du -ah .")
    .pipe("sort -hr")
    .pipe("head -n 5")
    .wait::<CmdResult>()?;
// the same run_cmd! macro
run_cmd!("du -ah . | sort -hr | head -n 10")?;

Process::new("ls")
    .pipe("wc -l")
    .current_dir("/src/rust-shell-script/")
    .wait::<CmdResult>()?;
```

## Builtin commands
### pwd
pwd: print current working directory

### cd
cd: set procecess current directory

```rust
run_cmd! {
    cd /tmp;
    ls | wc -l;
};
run_cmd!(pwd);
```

output will be "/tmp"

### lcd
lcd: set group commands current directory

```rust
run_cmd! {
    lcd /tmp;
    ls | wc -l;
};
run_cmd!(pwd);
```

output will be the old current directory

## Complete Example

```rust
use cmd_lib::{sh, run_cmd, run_fun, CmdResult, FunResult};

sh! {
    fn foo(time: &str) -> CmdResult {
        let wait = 3;
        #(sleep $wait)?;
        #(ls $f)?;
    }

    fn get_year() -> FunResult {
        $(date +%Y)
    }
}

fn main() -> CmdResult {
    run_cmd!(lcd /tmp; ls | wc -l;)?;

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

output:
```bash
INFO: Set local current_dir: "/tmp"
INFO: Running "ls | wc -l (cd: /tmp)" ...
42
INFO: Running "pwd" ...
/home/tao/src/rust-shell-script/rust_cmd_lib
INFO: Running "echo hello, rust" ...
hello, rust
INFO: Running "du -ah . | sort -hr | head -n 5" ...
INFO: Top 5 directories:
488M	.
485M	./target
286M	./target/debug
170M	./target/debug/incremental
163M	./target/package
INFO: Set env current_dir: "/var/tmp"
INFO: Running "sleep 3" ...
INFO: Running "ls nofile" ...
ls: cannot access 'nofile': No such file or directory
WARN: Failed to run foo()
INFO: Running "date +%Y" ...
INFO: You are in year 2019
```

## Related

See [rust-shell-script](https://github.com/rust-shell-script/rust-shell-script/), which can compile
rust-shell-script scripting language directly into rust code.
