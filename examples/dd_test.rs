// get disk read bandwidth with multiple threads
//
// Usage: dd_test [-b <block_size>] [-t <thread_num>] -f <file>
//
// e.g:
// ➜  rust_cmd_lib git:(master) ✗ cargo run --example dd_test -- -b 4096 -f /dev/nvme0n1 -t 4
//     Finished dev [unoptimized + debuginfo] target(s) in 0.06s
//      Running `target/debug/examples/dd_test -b 4096 -f /dev/nvme0n1 -t 4`
// Dropping caches at first
// Running "sudo bash -c echo 3 > /proc/sys/vm/drop_caches" ...
// Running with thread_num: 4, block_size: 4096
// Running "sudo bash -c dd if=/dev/nvme0n1 of=/dev/null bs=4096 skip=0 count=655360 2>&1" ...
// Running "sudo bash -c dd if=/dev/nvme0n1 of=/dev/null bs=4096 skip=655360 count=655360 2>&1" ...
// Running "sudo bash -c dd if=/dev/nvme0n1 of=/dev/null bs=4096 skip=1310720 count=655360 2>&1" ...
// Running "sudo bash -c dd if=/dev/nvme0n1 of=/dev/null bs=4096 skip=1966080 count=655360 2>&1" ...
// pid 22161 bandwidth: 267 MB/s
// pid 22162 bandwidth: 266 MB/s
// pid 22163 bandwidth: 274 MB/s
// pid 22164 bandwidth: 304 MB/s
// Total bandwidth: 1111 MB/s

use cmd_lib::{die, run_cmd, run_fun, spawn_with_output, use_builtin_cmd, CmdResult};
use std::env;

const DATA_SIZE: i64 = 10 * 1024 * 1024 * 1024; // 10GB data

fn main() -> CmdResult {
    use_builtin_cmd!(info);
    let mut opts = env::args().skip(1);
    let mut file = String::new();
    let mut block_size: i32 = 4096;
    let mut thread_num: i32 = 1;
    while let Some(opt) = opts.next() {
        match opt.as_str() {
            "-b" => {
                block_size = opts
                    .next()
                    .unwrap_or_else(|| die_with_usage("missing block"))
                    .parse()
                    .unwrap_or_else(|_| die_with_usage("invalid block"))
            }
            "-f" => {
                file = opts
                    .next()
                    .unwrap_or_else(|| die_with_usage("missing file"))
            }
            "-t" => {
                thread_num = opts
                    .next()
                    .unwrap_or_else(|| die_with_usage("missing thread number"))
                    .parse()
                    .unwrap_or_else(|_| die_with_usage("invalid thread number"))
            }
            _ => die_with_usage(""),
        }
    }

    if file.is_empty() {
        die_with_usage("");
    }

    cmd_lib::set_debug(true);
    run_cmd! (
        info "Dropping caches at first";
        sudo bash -c "echo 3 > /proc/sys/vm/drop_caches"
    )?;

    run_cmd!(info "Running with thread_num: $thread_num, block_size: $block_size")?;
    let cnt: i32 = (DATA_SIZE / thread_num as i64 / block_size as i64) as i32;
    let mut procs = vec![];
    for i in 0..thread_num {
        let off = cnt * i;
        procs.push(spawn_with_output!(sudo bash -c "dd if=$file of=/dev/null bs=$block_size skip=$off count=$cnt 2>&1")?);
    }

    let mut total_bandwidth = 0;
    cmd_lib::set_debug(false);
    for proc in procs {
        let pid = proc.id();
        let output = proc.wait_with_output()?;
        if !output.status.success() {
            die!("process exit with error: {:?}", output.status);
        }
        let output = String::from_utf8_lossy(&output.stdout).to_string();
        let bandwidth = run_fun!(echo $output | awk r"/MB/ {print $10}")?;
        total_bandwidth += bandwidth.parse::<i32>().unwrap();
        println!("pid {} bandwidth: {} MB/s", pid, bandwidth);
    }

    println!("Total bandwidth: {} MB/s", total_bandwidth);

    Ok(())
}

fn die_with_usage(msg: &str) -> ! {
    let prog = env::args().next().unwrap();
    eprintln!("{}", msg);
    eprintln!(
        "Usage: {} [-b <block_size>] [-t <thread_num>] -f <file>",
        run_fun!(basename $prog).unwrap()
    );
    std::process::exit(1)
}
