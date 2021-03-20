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
use clap::*;
use cmd_lib::*;
use rayon::prelude::*;

const DATA_SIZE: i64 = 10 * 1024 * 1024 * 1024; // 10GB data

fn main() -> CmdResult {
    use_builtin_cmd!(echo, info);
    let matches = clap_app!(dd_test_with_crates =>
        (@arg block_size: -b +takes_value "Set block size")
        (@arg thread_num: -t +takes_value "Set thread number")
        (@arg file: -f +takes_value +required "Set file path")
    )
    .get_matches();
    let block_size = value_t!(matches.value_of("block_size"), i32).unwrap_or_else(|e| {
        if e.kind == ErrorKind::ArgumentNotFound {
            4096
        } else {
            e.exit()
        }
    });
    let thread_num = value_t!(matches.value_of("thread_num"), i32).unwrap_or_else(|e| {
        if e.kind == ErrorKind::ArgumentNotFound {
            1
        } else {
            e.exit()
        }
    });
    let file = value_t!(matches.value_of("file"), String).unwrap_or_else(|e| e.exit());

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
            run_cmd!(info "thread $i bandwidth: $bandwidth MB/s").unwrap();
            bandwidth
        })
        .sum();
    run_cmd!(info "Total bandwidth: $total_bandwidth MB/s")?;

    Ok(())
}
