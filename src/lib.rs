//! # Rust command-line library
//!
//! Common rust command-line macros and utilities, to write shell-script like tasks
//! easily in rust programming language. Available at [crates.io](https://crates.io/crates/cmd_lib).
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
//! [rust cookbook](https://rust-lang-nursery.github.io/rust-cookbook/os/external.html), which is often a tedious
//! work.
//!
//! A lot of developers just choose shell(sh, bash, ...) scripts for such tasks, by using `<` to redirect input,
//! `>` to redirect output and '|' to pipe outputs. In my experience, this is **the only good parts** of shell script.
//! You can find all kinds of pitfalls and mysterious tricks to make other parts of shell script work. As the shell
//! scripts grow, they will ultimately be unmaintainable and no one wants to touch them any more.
//!
//! This cmd_lib library is trying to provide the redirection and piping capabilities, and other facilities to make writing
//! shell-script like tasks easily **without launching any shell**. For the
//! [rust cookbook examples](https://rust-lang-nursery.github.io/rust-cookbook/os/external.html),
//! they can usually be implemented as one line of rust macro with the help of this library, as in the
//! [examples/rust_cookbook_external.rs](https://github.com/rust-shell-script/rust_cmd_lib/blob/master/examples/rust_cookbook_external.rs).
//! Since they are rust code, you can always rewrite them in rust natively in the future, if necessary without spawning external commands.
//!
//! ## What this library provides
//!
//! ### Macros to run external commands
//! - run_cmd! --> CmdResult
//!
//! ```
//! # use cmd_lib::run_cmd;
//! let msg = "I love rust";
//! run_cmd!(echo $msg)?;
//! run_cmd!(echo "This is the message: $msg")?;
//!
//! // pipe commands are also supported
//! run_cmd!(du -ah . | sort -hr | head -n 10)?;
//!
//! // or a group of commands
//! // if any command fails, just return Err(...)
//! let file = "/tmp/f";
//! let keyword = "rust";
//! if run_cmd! {
//!     cat ${file} | grep ${keyword};
//!     echo "bad cmd" >&2;
//!     ls /nofile || true;
//!     date;
//!     ls oops;
//!     cat oops;
//! }.is_err() {
//!     // your error handling code
//! }
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! - run_fun! --> FunResult
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
//! ### Intuitive parameters passing
//! When passing parameters to `run_cmd!` and `run_fun!` macros, if they are not part to rust
//! [String literals](https://doc.rust-lang.org/reference/tokens.html#string-literals), they will be
//! converted to string as an atomic component, so you don't need to quote them. The parameters will be
//! like $a or ${a} in `run_cmd!` or `run_fun!` macros.
//!
//! ```
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
//!
//! If you want to use dynamic parameters, you can use $[] to access vector variable:
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
//! See examples at [examples/redirect.rs](https://github.com/rust-shell-script/rust_cmd_lib/blob/master/examples/redirect.rs)
//!
//! ### Macros to define, get and set global variables
//! - `proc_var!` to define thread local global variable
//! - `proc_var_get!` to get the value
//! - `proc_var_set!` to set the value
//! ```
//! # use cmd_lib::{ proc_var, proc_var_get, proc_var_set };
//! proc_var!(DELAY, f64, 1.0);
//! const DELAY_FACTOR: f64 = 0.8;
//! proc_var_set!(DELAY, |d| *d *= DELAY_FACTOR);
//! let d = proc_var_get!(DELAY);
//! // check more examples in examples/tetris.rs
//! ```
//!
//! ### Builtin commands
//! #### cd
//! cd: set process current directory
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
//! ### Macros to register your own commands
//! Declare your function with `export_cmd` attribute:
//!
//! ```
//! # use cmd_lib::{export_cmd, use_custom_cmd, run_cmd, run_fun, CmdArgs, CmdEnvs, FunResult};
//! #[export_cmd(my_cmd)]
//! fn foo(args: CmdArgs, _envs: CmdEnvs) -> FunResult {
//!     println!("msg from foo(), args: {:?}", args);
//!     Ok("bar".into())
//! }
//!
//! // To use it, just import it at first:
//! use_custom_cmd!(my_cmd);
//! run_cmd!(my_cmd)?;
//! println!("get result: {}", run_fun!(my_cmd)?);
//! # Ok::<(), std::io::Error>(())
//! ```
//! See examples in `examples/test_export_cmds.rs`
//!
//! ### Abstraction without overhead
//! Since all the macros' lexical analysis and syntactic analysis happen at compile time, it can
//! basically generate code the same as calling `std::process` APIs manually. It also includes
//! command type checking, so most of the errors can be found at compile time instead of at
//! runtime.
//!
//! ## Other Notes
//!
//! ### Environment Variables
//!
//! You can use [std::env::var](https://doc.rust-lang.org/std/env/fn.var.html) to fetch the environment variable
//! key from the current process. It will report error if the environment variable is not present, and it also
//! includes other checks to avoid silent failures.
//!
//! To set environment variables for the command only, you can put the assignments before the command.
//! Like this:
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
//! fn cleanup_uploaded_file(file: &str) -> CmdResult {
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
//! This library tries very hard to not set global states, so parallel `cargo test` can be executed just fine.
//! However, the process APIs are inherently not thread-safe, as a result I sometimes need to set
//! `RUST_TEST_THREADS=1` before running tests.

pub use cmd_lib_macros::{export_cmd, run_cmd, run_fun, use_custom_cmd};
pub type FunResult = std::io::Result<String>;
pub type CmdResult = std::io::Result<()>;
pub use process::{export_cmd, set_debug, Cmd, CmdArgs, CmdEnvs, Cmds, FdOrFile, GroupCmds};

mod proc_var;
mod process;
