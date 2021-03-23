# cmd_lib

## Rust command-line library

Common rust command-line macros and utilities, to write shell-script like tasks
easily in rust programming language. Available at [crates.io](https://crates.io/crates/cmd_lib).

[![Build status](https://github.com/rust-shell-script/rust_cmd_lib/workflows/ci/badge.svg)](https://github.com/rust-shell-script/rust_cmd_lib/actions)
[![Crates.io](https://img.shields.io/crates/v/cmd_lib.svg)](https://crates.io/crates/cmd_lib)

### Why you need this
If you need to run some external commands in rust, the
[std::process::Command](https://doc.rust-lang.org/std/process/struct.Command.html) is a good
abstraction layer on top of different OS syscalls. It provides fine-grained control over
how a new process should be spawned, and it allows you to wait for process to finish and check the
exit status or collect all of its output. However, when
[Redirection](https://en.wikipedia.org/wiki/Redirection_(computing)) or
[Piping](https://en.wikipedia.org/wiki/Redirection_(computing)#Piping) is needed, you need to
set up the parent and child IO handles manually, like this in the
[rust cookbook](https://rust-lang-nursery.github.io/rust-cookbook/os/external.html), which is often a tedious
work.

A lot of developers just choose shell(sh, bash, ...) scripts for such tasks, by using `<` to redirect input,
`>` to redirect output and '|' to pipe outputs. In my experience, this is **the only good parts** of shell script.
You can find all kinds of pitfalls and mysterious tricks to make other parts of shell script work. As the shell
scripts grow, they will ultimately be unmaintainable and no one wants to touch them any more.

This cmd_lib library is trying to provide the redirection and piping capabilities, and other facilities to make writing
shell-script like tasks easily **without launching any shell**. For the
[rust cookbook examples](https://rust-lang-nursery.github.io/rust-cookbook/os/external.html),
they can usually be implemented as one line of rust macro with the help of this library, as in the
[examples/rust_cookbook.rs](https://github.com/rust-shell-script/rust_cmd_lib/blob/master/examples/rust_cookbook.rs).
Since they are rust code, you can always rewrite them in rust natively in the future, if necessary without spawning external commands.

### What this library looks like

To get a first impression, here is an example from
[examples/dd_test.rs](https://github.com/rust-shell-script/rust_cmd_lib/blob/master/examples/dd_test.rs):

```rust
run_cmd! (
    info "Dropping caches at first";
    sudo bash -c "echo 3 > /proc/sys/vm/drop_caches";
    info "Running with thread_num: $thread_num, block_size: $block_size";
)?;
let cnt = DATA_SIZE / thread_num / block_size;
let now = Instant::now();
(0..thread_num).into_par_iter().for_each(|i| {
    let off = cnt * i;
    let bandwidth = run_fun!(
        sudo bash -c "dd if=$file of=/dev/null bs=$block_size skip=$off count=$cnt 2>&1"
        | awk r"/copied/{print $10 $11}" | cut -d / -f1
    )
    .unwrap();
    cmd_info!("thread $i bandwidth: ${bandwidth}/s");
});
let total_bandwidth =
    Byte::from_bytes((DATA_SIZE / now.elapsed().as_secs()) as u128).get_appropriate_unit(true);
cmd_info!("Total bandwidth: ${total_bandwidth}/s");
```

Output will be like this:

```console
➜  rust_cmd_lib git:(master) ✗ cargo run --example dd_test -- -b 4096 -f /dev/nvme0n1 -t 4
    Finished dev [unoptimized + debuginfo] target(s) in 1.56s
     Running `target/debug/examples/dd_test -b 4096 -f /dev/nvme0n1 -t 4`
Dropping caches at first
Running with thread_num: 4, block_size: 4096
thread 1 bandwidth: 286MB/s
thread 3 bandwidth: 269MB/s
thread 2 bandwidth: 267MB/s
thread 0 bandwidth: 265MB/s
Total bandwidth: 1.01 GiB/s
```

### What this library provides

#### Macros to run external commands
- run_cmd! --> CmdResult

```rust
let msg = "I love rust";
run_cmd!(echo $msg)?;
run_cmd!(echo "This is the message: $msg")?;

// pipe commands are also supported
run_cmd!(du -ah . | sort -hr | head -n 10)?;

// or a group of commands
// if any command fails, just return Err(...)
let file = "/tmp/f";
let keyword = "rust";
if run_cmd! {
    cat ${file} | grep ${keyword};
    echo "bad cmd" >&2;
    ls /nofile || true;
    date;
    ls oops;
    cat oops;
}.is_err() {
    // your error handling code
}
```

- run_fun! --> FunResult

```rust
let version = run_fun!(rustc --version)?;
eprintln!("Your rust version is {}", version);

// with pipes
let n = run_fun!(echo "the quick brown fox jumped over the lazy dog" | wc -w)?;
eprintln!("There are {} words in above sentence", n);
```

#### Abstraction without overhead
Since all the macros' lexical analysis and syntactic analysis happen at compile time, it can
basically generate code the same as calling `std::process` APIs manually. It also includes
command type checking, so most of the errors can be found at compile time instead of at
runtime.

#### Intuitive parameters passing
When passing parameters to `run_cmd!` and `run_fun!` macros, if they are not part to rust
[String literals](https://doc.rust-lang.org/reference/tokens.html#string-literals), they will be
converted to string as an atomic component, so you don't need to quote them. The parameters will be
like $a or ${a} in `run_cmd!` or `run_fun!` macros.

```rust
let dir = "my folder";
run_cmd!(echo "Creating $dir at /tmp")?;
run_cmd!(mkdir -p /tmp/$dir)?;

// or with group commands:
let dir = "my folder";
run_cmd!(echo "Creating $dir at /tmp"; mkdir -p /tmp/$dir)?;
```
You can consider "" as glue, so everything inside the quotes will be treated as a single atomic component.

If they are part of [Raw string literals](https://doc.rust-lang.org/reference/tokens.html#raw-string-literals),
there will be no string interpolation, the same as in idiomatic rust. However, you can always use `format!` macro
to form the new string. For example:
```rust
// string interpolation
let key_word = "time";
let awk_opts = format!(r#"/{}/ {{print $(NF-3) " " $(NF-1) " " $NF}}"#, key_word);
run_cmd!(ping -c 10 www.google.com | awk $awk_opts)?;
```
Notice here `$awk_opts` will be treated as single option passing to awk command.

If you want to use dynamic parameters, you can use $[] to access vector variable:
```rust
let gopts = vec![vec!["-l", "-a", "/"], vec!["-a", "/var"]];
for opts in gopts {
    run_cmd!(ls $[opts])?;
}
```

#### Redirection and Piping
Right now piping and stdin, stdout, stderr redirection are supported. Most parts are the same as in
[bash scripts](https://www.gnu.org/software/bash/manual/html_node/Redirections.html#Redirections).

#### Builtin commands
##### cd
cd: set process current directory, which is always enabled
```rust
run_cmd! (
    cd /tmp;
    ls | wc -l;
)?;
```
Notice that builtin `cd` will only change with current scope
and it will restore the previous current directory when it
exits the scope.

Use `std::env::set_current_dir` if you want to change the current
working directory for the whole program.

##### true

Just return true without launching any processes.

##### echo

```rust
use_builtin_cmd!(true, echo); // find more builtin commands in src/builtins.rs
run_cmd!(echo "This is from builtin command!")?;
```

#### Macros to register your own commands
Declare your function with `export_cmd` attribute, and import it with `use_custom_cmd` macro:

```rust
#[export_cmd(my_cmd)]
fn foo(args: CmdArgs, _envs: CmdEnvs) -> FunResult {
    println!("msg from foo(), args: {:?}", args);
    Ok("bar".into())
}

use_custom_cmd!(my_cmd);
run_cmd!(my_cmd)?;
println!("get result: {}", run_fun!(my_cmd)?);
```

#### Low-level process spawning macro

spawn!() macro executes the whole command as a child process, returning a handle to it. By
default, stdin, stdout and stderr are inherited from the parent. To capture the output, you
can use spawn_with_output!() macro instead.

To get result, you can call wait_result() to get CmdResult/FunResult.

```rust
spawn!(ping -c 10 192.168.0.1)?.wait_result()?;
let output = spawn_with_output!(/bin/cat file.txt | sed s/a/b/)?.wait_result();
```


#### Macros to define, get and set thread-local global variables
- `tls_init!` to define thread local global variable
- `tls_get!` to get the value
- `tls_set!` to set the value
```rust
tls_init!(DELAY, f64, 1.0);
const DELAY_FACTOR: f64 = 0.8;
tls_set!(DELAY, |d| *d *= DELAY_FACTOR);
let d = tls_get!(DELAY);
// check more examples in examples/tetris.rs
```

### Other Notes

#### Environment Variables

You can use [std::env::var](https://doc.rust-lang.org/std/env/fn.var.html) to fetch the environment variable
key from the current process. It will report error if the environment variable is not present, and it also
includes other checks to avoid silent failures.

To set environment variables, you can use [std::env::set_var](https://doc.rust-lang.org/std/env/fn.set_var.html).
There are also other related APIs in the [std::env](https://doc.rust-lang.org/std/env/index.html) module.

To set environment variables for the command only, you can put the assignments before the command.
Like this:
```rust
run_cmd!(FOO=100 /tmp/test_run_cmd_lib.sh)?;
```

#### Security Notes
Using macros can actually avoid command injection, since we do parsing before variable substitution.
For example, below code is fine even without any quotes:
```rust
fn cleanup_uploaded_file(file: &str) -> CmdResult {
    run_cmd!(/bin/rm -f /var/upload/$file)
}
```
It is not the case in bash, which will always do variable substitution at first.

#### Glob/Wildcard

This library does not provide glob functions, to avoid silent errors and other surprises.
You can use the [glob](https://github.com/rust-lang-nursery/glob) package instead.

#### Thread Safety

This library tries very hard to not set global states, so parallel `cargo test` can be executed just fine.
However, there might be some internal process related APIs which are not thread-safe. More
investigation is needed.


License: MIT OR Apache-2.0
