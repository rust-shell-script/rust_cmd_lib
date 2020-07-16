extern crate cmd_lib;

use cmd_lib::{run_cmd, run_fun, sh};

#[test]
fn test_run_cmd() {
    let _ = run_cmd!(date);
}

#[test]
fn test_run_cmds() {
    let _ = run_cmd! {
        cd /tmp;
        ls;
    }
    .unwrap();
}

#[test]
fn test_run_fun() {
    let uptime = run_fun!(uptime).unwrap();
    eprintln!("uptime: {}", uptime);
}

#[test]
fn test_sh() {
    sh! {
        fn foo() {
            println!("this is foo");
        }
    }
    foo();
}
