//
// Rewrite examples with rust_cmd_lib from
// https://rust-lang-nursery.github.io/rust-cookbook/os/external.html
//
use cmd_lib::*;
use std::io::{BufRead, BufReader};

#[cmd_lib::main]
fn main() -> CmdResult {
    cmd_lib::set_pipefail(false); // do not fail due to pipe errors

    // Run an external command and process stdout
    run_cmd!(git log --oneline | head -5)?;

    // Run an external command passing it stdin and check for an error code
    run_cmd!(echo "import this; copyright(); credits(); exit()" | python)?;

    // Run piped external commands
    let directory = std::env::current_dir()?;
    println!(
        "Top 10 biggest files and directories in '{}':\n{}",
        directory.display(),
        run_fun!(du -ah . | sort -hr | head -n 10)?
    );

    // Redirect both stdout and stderr of child process to the same file
    run_cmd!(ignore ls . oops &>out.txt)?;
    run_cmd!(rm -f out.txt)?;

    // Continuously process child process' outputs
    spawn_with_output!(journalctl)?.wait_with_pipe(&mut |pipe| {
        BufReader::new(pipe)
            .lines()
            .filter_map(|line| line.ok())
            .filter(|line| line.find("usb").is_some())
            .take(10)
            .for_each(|line| println!("{}", line));
    })?;

    Ok(())
}
