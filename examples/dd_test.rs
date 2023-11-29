// get disk read bandwidth with multiple threads
//
// Usage: dd_test [-b <block_size>] [-t <thread_num>] -f <file>
//
// e.g:
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
use byte_unit::Byte;
use cmd_lib::*;
use rayon::prelude::*;
use std::time::Instant;
use structopt::StructOpt;

const DATA_SIZE: u64 = 10 * 1024 * 1024 * 1024; // 10GB data

#[derive(StructOpt)]
#[structopt(name = "dd_test", about = "Get disk read bandwidth.")]
struct Opt {
    #[structopt(short, default_value = "4096")]
    block_size: u64,
    #[structopt(short, default_value = "1")]
    thread_num: u64,
    #[structopt(short)]
    file: String,
}

#[cmd_lib::main]
fn main() -> CmdResult {
    let Opt {
        block_size,
        thread_num,
        file,
    } = Opt::from_args();

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
            | awk r#"/copied/{print $(NF-1) " " $NF}"#
        )
        .unwrap_or_else(|_| cmd_die!("thread $i failed"));
        info!("thread {i} bandwidth: {bandwidth}");
    });
    let total_bandwidth =
        Byte::from_bytes((DATA_SIZE / now.elapsed().as_secs()) as u128).get_appropriate_unit(true);
    info!("Total bandwidth: {total_bandwidth}/s");

    Ok(())
}
