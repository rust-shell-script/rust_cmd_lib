//! # Rust command-line library
//!
//! Common rust command-line macros and utilities, to write shell-script like tasks
//! easily in rust programming language. Available at [crates.io](https://crates.io/crates/cmd_lib).
//!
//! [![Build status](https://github.com/rust-shell-script/rust_cmd_lib/workflows/ci/badge.svg)](https://github.com/rust-shell-script/rust_cmd_lib/actions)
//! [![Crates.io](https://img.shields.io/crates/v/cmd_lib.svg)](https://crates.io/crates/cmd_lib)
//!
//! ## Why you need this
//! If you need to run some external commands in rust, the
//! [std::process::Command](https://doc.rust-lang.org/std/process/struct.Command.html) is a good
//! abstraction layer on top of different OS syscalls. It provides fine-grained control over
//! how a new process should be spawned, and it allows you to wait for process to finish and check the
//! exit status or collect all of its output. However, when
//! [Redirection](https://en.wikipedia.org/wiki/Redirection_(computing)) or
//! [Piping](https://en.wikipedia.org/wiki/Redirection_(computing)#Piping) is needed, you need to
//! set up the parent and child IO handles manually, like this in the
//! [rust cookbook](https://rust-lang-nursery.github.io/rust-cookbook/os/external.html), which is often tedious
//! and [error prone](https://github.com/ijackson/rust-rfcs/blob/command/text/0000-command-ergonomics.md#currently-accepted-wrong-programs).
//!
//! A lot of developers just choose shell(sh, bash, ...) scripts for such tasks, by using `<` to redirect input,
//! `>` to redirect output and `|` to pipe outputs. In my experience, this is **the only good parts** of shell script.
//! You can find all kinds of pitfalls and mysterious tricks to make other parts of shell script work. As the shell
//! scripts grow, they will ultimately be unmaintainable and no one wants to touch them any more.
//!
//! This cmd_lib library is trying to provide the redirection and piping capabilities, and other facilities to make writing
//! shell-script like tasks easily **without launching any shell**. For the
//! [rust cookbook examples](https://rust-lang-nursery.github.io/rust-cookbook/os/external.html),
//! they can usually be implemented as one line of rust macro with the help of this library, as in the
//! [examples/rust_cookbook.rs](https://github.com/rust-shell-script/rust_cmd_lib/blob/master/examples/rust_cookbook.rs).
//! Since they are rust code, you can always rewrite them in rust natively in the future, if necessary without spawning external commands.
//!
//! ## What this library looks like
//!
//! To get a first impression, here is an example from
//! [examples/dd_test.rs](https://github.com/rust-shell-script/rust_cmd_lib/blob/master/examples/dd_test.rs):
//!
//! ```no_run
//! # use byte_unit::Byte;
//! # use cmd_lib::*;
//! # use rayon::prelude::*;
//! # use std::time::Instant;
//! # const DATA_SIZE: u64 = 10 * 1024 * 1024 * 1024; // 10GB data
//! # let mut file = String::new();
//! # let mut block_size: u64 = 4096;
//! # let mut thread_num: u64 = 1;
//! run_cmd! (
//!     info "Dropping caches at first";
//!     sudo bash -c "echo 3 > /proc/sys/vm/drop_caches";
//!     info "Running with thread_num: $thread_num, block_size: $block_size";
//! )?;
//! let cnt = DATA_SIZE / thread_num / block_size;
//! let now = Instant::now();
//! (0..thread_num).into_par_iter().for_each(|i| {
//!     let off = cnt * i;
//!     let bandwidth = run_fun!(
//!         sudo bash -c "dd if=$file of=/dev/null bs=$block_size skip=$off count=$cnt 2>&1"
//!         | awk r#"/copied/{print $(NF-1) " " $NF}"#
//!     )
//!     .unwrap_or_else(|_| cmd_die!("thread $i failed"));
//!     info!("thread {i} bandwidth: {bandwidth}");
//! });
//! let total_bandwidth = Byte::from_bytes((DATA_SIZE / now.elapsed().as_secs()) as u128).get_appropriate_unit(true);
//! info!("Total bandwidth: {total_bandwidth}/s");
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! Output will be like this:
//!
//! ```console
//! ➜  rust_cmd_lib git:(master) ✗ cargo run --example dd_test -- -b 4096 -f /dev/nvme0n1 -t 4
//!     Finished dev [unoptimized + debuginfo] target(s) in 0.04s
//!      Running `target/debug/examples/dd_test -b 4096 -f /dev/nvme0n1 -t 4`
//! [INFO ] Dropping caches at first
//! [INFO ] Running with thread_num: 4, block_size: 4096
//! [INFO ] thread 3 bandwidth: 317 MB/s
//! [INFO ] thread 1 bandwidth: 289 MB/s
//! [INFO ] thread 0 bandwidth: 281 MB/s
//! [INFO ] thread 2 bandwidth: 279 MB/s
//! [INFO ] Total bandwidth: 1.11 GiB/s
//! ```
//!
//! ## What this library provides
//!
//! ### Macros to run external commands
//! - [`run_cmd!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.run_cmd.html) -> [`CmdResult`](https://docs.rs/cmd_lib/latest/cmd_lib/type.CmdResult.html)
//!
//! ```no_run
//! # use cmd_lib::run_cmd;
//! let msg = "I love rust";
//! run_cmd!(echo $msg)?;
//! run_cmd!(echo "This is the message: $msg")?;
//!
//! // pipe commands are also supported
//! let dir = "/var/log";
//! run_cmd!(du -ah $dir | sort -hr | head -n 10)?;
//!
//! // or a group of commands
//! // if any command fails, just return Err(...)
//! let file = "/tmp/f";
//! let keyword = "rust";
//! run_cmd! {
//!     cat ${file} | grep ${keyword};
//!     echo "bad cmd" >&2;
//!     ignore ls /nofile;
//!     date;
//!     ls oops;
//!     cat oops;
//! }?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! - [`run_fun!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.run_fun.html) -> [`FunResult`](https://docs.rs/cmd_lib/latest/cmd_lib/type.FunResult.html)
//!
//! ```
//! # use cmd_lib::run_fun;
//! let version = run_fun!(rustc --version)?;
//! eprintln!("Your rust version is {}", version);
//!
//! // with pipes
//! let n = run_fun!(echo "the quick brown fox jumped over the lazy dog" | wc -w)?;
//! eprintln!("There are {} words in above sentence", n);
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! ### Abstraction without overhead
//!
//! Since all the macros' lexical analysis and syntactic analysis happen at compile time, it can
//! basically generate code the same as calling `std::process` APIs manually. It also includes
//! command type checking, so most of the errors can be found at compile time instead of at
//! runtime. With tools like `rust-analyzer`, it can give you real-time feedback for broken
//! commands being used.
//!
//! You can use `cargo expand` to check the generated code.
//!
//! ### Intuitive parameters passing
//! When passing parameters to `run_cmd!` and `run_fun!` macros, if they are not part to rust
//! [String literals](https://doc.rust-lang.org/reference/tokens.html#string-literals), they will be
//! converted to string as an atomic component, so you don't need to quote them. The parameters will be
//! like `$a` or `${a}` in `run_cmd!` or `run_fun!` macros.
//!
//! ```no_run
//! # use cmd_lib::run_cmd;
//! let dir = "my folder";
//! run_cmd!(echo "Creating $dir at /tmp")?;
//! run_cmd!(mkdir -p /tmp/$dir)?;
//!
//! // or with group commands:
//! let dir = "my folder";
//! run_cmd!(echo "Creating $dir at /tmp"; mkdir -p /tmp/$dir)?;
//! # Ok::<(), std::io::Error>(())
//! ```
//! You can consider "" as glue, so everything inside the quotes will be treated as a single atomic component.
//!
//! If they are part of [Raw string literals](https://doc.rust-lang.org/reference/tokens.html#raw-string-literals),
//! there will be no string interpolation, the same as in idiomatic rust. However, you can always use `format!` macro
//! to form the new string. For example:
//! ```no_run
//! # use cmd_lib::run_cmd;
//! // string interpolation
//! let key_word = "time";
//! let awk_opts = format!(r#"/{}/ {{print $(NF-3) " " $(NF-1) " " $NF}}"#, key_word);
//! run_cmd!(ping -c 10 www.google.com | awk $awk_opts)?;
//! # Ok::<(), std::io::Error>(())
//! ```
//! Notice here `$awk_opts` will be treated as single option passing to awk command.
//!
//! If you want to use dynamic parameters, you can use `$[]` to access vector variable:
//! ```no_run
//! # use cmd_lib::run_cmd;
//! let gopts = vec![vec!["-l", "-a", "/"], vec!["-a", "/var"]];
//! for opts in gopts {
//!     run_cmd!(ls $[opts])?;
//! }
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! ### Redirection and Piping
//! Right now piping and stdin, stdout, stderr redirection are supported. Most parts are the same as in
//! [bash scripts](https://www.gnu.org/software/bash/manual/html_node/Redirections.html#Redirections).
//!
//! ### Logging
//!
//! This library provides convenient macros and builtin commands for logging. All messages which
//! are printed to stderr will be logged. It will also include the full running commands in the error
//! result.
//!
//! ```no_run
//! # use cmd_lib::*;
//! let dir: &str = "folder with spaces";
//! run_cmd!(mkdir /tmp/$dir; ls /tmp/$dir)?;
//! run_cmd!(mkdir /tmp/$dir; ls /tmp/$dir; rmdir /tmp/$dir)?;
//! // output:
//! // [INFO ] mkdir: cannot create directory ‘/tmp/folder with spaces’: File exists
//! // Error: Running ["mkdir" "/tmp/folder with spaces"] exited with error; status code: 1
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! It is using rust [log crate](https://crates.io/crates/log), and you can use your actual favorite
//! logger implementation. Notice that if you don't provide any logger, it will use env_logger to print
//! messages from process's stderr.
//!
//! You can also mark your `main()` function with `#[cmd_lib::main]`, which will log error from
//! main() by default. Like this:
//! ```console
//! [ERROR] FATAL: Running ["mkdir" "/tmp/folder with spaces"] exited with error; status code: 1
//! ```
//!
//! ### Builtin commands
//! #### cd
//! cd: set process current directory.
//! ```no_run
//! # use cmd_lib::run_cmd;
//! run_cmd! (
//!     cd /tmp;
//!     ls | wc -l;
//! )?;
//! # Ok::<(), std::io::Error>(())
//! ```
//! Notice that builtin `cd` will only change with current scope
//! and it will restore the previous current directory when it
//! exits the scope.
//!
//! Use `std::env::set_current_dir` if you want to change the current
//! working directory for the whole program.
//!
//! #### ignore
//!
//! Ignore errors for command execution.
//!
//! #### echo
//! Print messages to stdout.
//! ```console
//! -n     do not output the trailing newline
//! ```
//!
//! #### error, warn, info, debug, trace
//!
//! Print messages to logging with different levels. You can also use the normal logging macros,
//! if you don't need to do logging inside the command group.
//!
//! ```no_run
//! # use cmd_lib::*;
//! run_cmd!(error "This is an error message")?;
//! run_cmd!(warn "This is a warning message")?;
//! run_cmd!(info "This is an information message")?;
//! // output:
//! // [ERROR] This is an error message
//! // [WARN ] This is a warning message
//! // [INFO ] This is an information message
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! ### Low-level process spawning macros
//!
//! [`spawn!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.spawn.html) macro executes the whole command as a child process, returning a handle to it. By
//! default, stdin, stdout and stderr are inherited from the parent. The process will run in the
//! background, so you can run other stuff concurrently. You can call [`wait()`](https://docs.rs/cmd_lib/latest/cmd_lib/struct.CmdChildren.html#method.wait) to wait
//! for the process to finish.
//!
//! With [`spawn_with_output!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.spawn_with_output.html) you can get output by calling
//! [`wait_with_output()`](https://docs.rs/cmd_lib/latest/cmd_lib/struct.FunChildren.html#method.wait_with_output),
//! [`wait_with_all()`](https://docs.rs/cmd_lib/latest/cmd_lib/struct.FunChildren.html#method.wait_with_all)
//! or even do stream
//! processing with [`wait_with_pipe()`](https://docs.rs/cmd_lib/latest/cmd_lib/struct.FunChildren.html#method.wait_with_pipe).
//!
//! There are also other useful APIs, and you can check the docs for more details.
//!
//! ```no_run
//! # use cmd_lib::*;
//! # use std::io::{BufRead, BufReader};
//! let mut proc = spawn!(ping -c 10 192.168.0.1)?;
//! // do other stuff
//! // ...
//! proc.wait()?;
//!
//! let mut proc = spawn_with_output!(/bin/cat file.txt | sed s/a/b/)?;
//! // do other stuff
//! // ...
//! let output = proc.wait_with_output()?;
//!
//! spawn_with_output!(journalctl)?.wait_with_pipe(&mut |pipe| {
//!     BufReader::new(pipe)
//!         .lines()
//!         .filter_map(|line| line.ok())
//!         .filter(|line| line.find("usb").is_some())
//!         .take(10)
//!         .for_each(|line| println!("{}", line));
//! })?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! ### Macro to register your own commands
//! Declare your function with the right signature, and register it with [`use_custom_cmd!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.use_custom_cmd.html) macro:
//!
//! ```
//! # use cmd_lib::*;
//! # use std::io::Write;
//! fn my_cmd(env: &mut CmdEnv) -> CmdResult {
//!     let args = env.get_args();
//!     let (res, stdout, stderr) = spawn_with_output! {
//!         orig_cmd $[args]
//!             --long-option xxx
//!             --another-option yyy
//!     }?
//!     .wait_with_all();
//!     writeln!(env.stdout(), "{}", stdout)?;
//!     writeln!(env.stderr(), "{}", stderr)?;
//!     res
//! }
//!
//! use_custom_cmd!(my_cmd);
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! ### Macros to define, get and set thread-local global variables
//! - [`tls_init!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.tls_init.html) to define thread local global variable
//! - [`tls_get!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.tls_get.html) to get the value
//! - [`tls_set!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.tls_set.html) to set the value
//! ```
//! # use cmd_lib::{ tls_init, tls_get, tls_set };
//! tls_init!(DELAY, f64, 1.0);
//! const DELAY_FACTOR: f64 = 0.8;
//! tls_set!(DELAY, |d| *d *= DELAY_FACTOR);
//! let d = tls_get!(DELAY);
//! // check more examples in examples/tetris.rs
//! ```
//!
//! ## Other Notes
//!
//! ### Environment Variables
//!
//! You can use [std::env::var](https://doc.rust-lang.org/std/env/fn.var.html) to fetch the environment variable
//! key from the current process. It will report error if the environment variable is not present, and it also
//! includes other checks to avoid silent failures.
//!
//! To set environment variables in **single-threaded programs**, you can use [std::env::set_var] and
//! [std::env::remove_var]. While those functions **[must not be called]** if any other threads might be running, you can
//! always set environment variables for one command at a time, by putting the assignments before the command:
//!
//! ```no_run
//! # use cmd_lib::run_cmd;
//! run_cmd!(FOO=100 /tmp/test_run_cmd_lib.sh)?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! ### Security Notes
//! Using macros can actually avoid command injection, since we do parsing before variable substitution.
//! For example, below code is fine even without any quotes:
//! ```
//! # use cmd_lib::{run_cmd, CmdResult};
//! # use std::path::Path;
//! fn cleanup_uploaded_file(file: &Path) -> CmdResult {
//!     run_cmd!(/bin/rm -f /var/upload/$file)
//! }
//! ```
//! It is not the case in bash, which will always do variable substitution at first.
//!
//! ### Glob/Wildcard
//!
//! This library does not provide glob functions, to avoid silent errors and other surprises.
//! You can use the [glob](https://github.com/rust-lang-nursery/glob) package instead.
//!
//! ### Thread Safety
//!
//! This library tries very hard to not set global state, so parallel `cargo test` can be executed just fine.
//! That said, there are some limitations to be aware of:
//!
//! - [std::env::set_var] and [std::env::remove_var] **[must not be called]** in a multi-threaded program
//! - [`tls_init!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.tls_init.html),
//!   [`tls_get!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.tls_get.html), and
//!   [`tls_set!`](https://docs.rs/cmd_lib/latest/cmd_lib/macro.tls_set.html) create *thread-local* variables, which
//!   means each thread will have its own independent version of the variable
//! - [`set_debug`](https://docs.rs/cmd_lib/latest/cmd_lib/fn.set_debug.html) and
//!   [`set_pipefail`](https://docs.rs/cmd_lib/latest/cmd_lib/fn.set_pipefail.html) are *global* and affect all threads;
//!   there is currently no way to change those settings without affecting other threads
//!
//! [std::env::set_var]: https://doc.rust-lang.org/std/env/fn.set_var.html
//! [std::env::remove_var]: https://doc.rust-lang.org/std/env/fn.remove_var.html
//! [must not be called]: https://doc.rust-lang.org/nightly/edition-guide/rust-2024/newly-unsafe-functions.html#stdenvset_var-remove_var

pub use cmd_lib_macros::{
    cmd_die, main, run_cmd, run_fun, spawn, spawn_with_output, use_custom_cmd,
};
/// Return type for [`run_fun!()`] macro.
pub type FunResult = std::io::Result<String>;
/// Return type for [`run_cmd!()`] macro.
pub type CmdResult = std::io::Result<()>;
pub use child::{CmdChildren, FunChildren};
pub use io::{CmdIn, CmdOut};
#[doc(hidden)]
pub use log as inner_log;
#[doc(hidden)]
pub use logger::try_init_default_logger;
#[doc(hidden)]
pub use process::{register_cmd, AsOsStr, Cmd, CmdString, Cmds, GroupCmds, Redirect};
pub use process::{set_debug, set_pipefail, CmdEnv, ScopedDebug, ScopedPipefail};

mod builtins;
mod child;
mod io;
mod logger;
mod process;
mod thread_local;
