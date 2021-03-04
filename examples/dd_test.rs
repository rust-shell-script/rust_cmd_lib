use cmd_lib::{die, run_cmd, run_fun, spawn, use_builtin_cmd, CmdResult};
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
        procs.push(spawn!(sudo dd if=$file of=/dev/null bs=$block_size skip=$off count=$cnt)?);
    }

    for proc in procs.iter_mut() {
        let status = proc.wait()?;
        if !status.success() {
            die!("process exit with error: {:?}", status);
        }
    }

    Ok(())
}

fn die_with_usage(msg: &str) -> ! {
    let prog = env::args().next().unwrap();
    eprintln!("{}", msg);
    eprintln!(
        "Usage: {} [-b <block>] [-t <thread_num>] -f <file>",
        run_fun!(basename $prog).unwrap()
    );
    std::process::exit(1)
}
