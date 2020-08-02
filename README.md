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
    eprintln!("Run group command failed");
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
See `examples` directory, which contains a tetris game converted from bash implementation.

## Related

See [rust-shell-script](https://github.com/rust-shell-script/rust-shell-script/), which can compile
rust-shell-script scripting language directly into rust code.
