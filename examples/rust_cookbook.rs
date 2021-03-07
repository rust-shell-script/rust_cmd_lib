//
// Rewrite examples with rust_cmd_lib from
// https://rust-lang-nursery.github.io/rust-cookbook/os/external.html
//
use cmd_lib::{run_cmd, run_fun, CmdResult};
fn main() -> CmdResult {
    cmd_lib::set_debug(true); // to print commands

    // Run an external command and process stdout
    run_cmd!(git log --oneline | head -5 || true)?;

    // Run an external command passing it stdin and check for an error code
    run_cmd!(echo "import this; copyright(); credits(); exit()" | python)?;

    // Run piped external commands
    let directory = std::env::current_dir()?;
    println!(
        "Top 10 biggest files and directories in '{}':\n{}",
        directory.display(),
        run_fun!(du -ah . | sort -hr | head -n 10 || true).unwrap()
    );

    // Redirect both stdout and stderr of child process to the same file
    run_cmd!(ls . oops &>out.txt || true)?;
    run_cmd!(rm -f out.txt)?;

    // Continuously process child process' outputs
    run_cmd!(ping -c 5 www.google.com | awk r#"/time/ {print $(NF-3) " " $(NF-1) " " $NF}"#)?;

    Ok(())
}
