// get disk read bandwidth with multiple threads
//
// Usage: dd_test [-b <block_size>] [-t <thread_num>] -f <file>
//
// e.g:
// ➜  rust_cmd_lib git:(master) ✗ cargo run --example dd_test_with_crates -- -b 4096 -f /dev/nvme0n1 -t 4
//     Finished dev [unoptimized + debuginfo] target(s) in 0.03s
//      Running `target/debug/examples/dd_test_with_crates -b 4096 -f /dev/nvme0n1 -t 4`
// Dropping caches at first
// Running with thread_num: 4, block_size: 4096
// thread 3 bandwidth: 279 MB/s
// thread 2 bandwidth: 273 MB/s
// thread 0 bandwidth: 273 MB/s
// thread 1 bandwidth: 269 MB/s
// Total bandwidth: 1094 MB/s
use cmd_lib::{run_cmd, run_fun, use_builtin_cmd, CmdResult};
use rayon::prelude::*;
use std::env;

const DATA_SIZE: i64 = 10 * 1024 * 1024 * 1024; // 10GB data

fn main() -> CmdResult {
    use_builtin_cmd!(echo, info);
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
            opt => die_with_usage(format!("invalid option: {}", opt).as_str()),
        }
    }

    if file.is_empty() {
        die_with_usage("file name is empty");
    }

    run_cmd! (
        info "Dropping caches at first";
        sudo bash -c "echo 3 > /proc/sys/vm/drop_caches";
        info "Running with thread_num: $thread_num, block_size: $block_size";
    )?;
    let cnt: i32 = (DATA_SIZE / thread_num as i64 / block_size as i64) as i32;
    let total_bandwidth: i32 = (0..thread_num)
        .into_par_iter()
        .map(|i| {
            let off = cnt * i;
            let bandwidth = run_fun!(
                sudo bash -c "dd if=$file of=/dev/null bs=$block_size skip=$off count=$cnt 2>&1"
                | awk r"/MB/ {print $10}"
            )
            .unwrap()
            .parse::<i32>()
            .unwrap();
            eprintln!("thread {} bandwidth: {} MB/s", i, bandwidth);
            bandwidth
        })
        .sum();
    run_cmd!(info "Total bandwidth: $total_bandwidth MB/s")?;

    Ok(())
}

fn die_with_usage(msg: &str) -> ! {
    let arg0 = env::args().next().unwrap();
    let prog = run_fun!(basename $arg0).unwrap();
    run_cmd! (
        info $msg;
        info "Usage: $prog [-b <block_size>] [-t <thread_num>] -f <file>"
    )
    .unwrap();
    std::process::exit(1)
}
