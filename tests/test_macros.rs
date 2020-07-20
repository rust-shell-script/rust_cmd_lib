extern crate cmd_lib;
use cmd_lib::{run_cmd, run_fun};

#[test]
fn test_run_cmd() {
    let _ = run_cmd!(date);
}

#[test]
fn test_run_cmds() {
    let _ = run_cmd! {
        lcd /tmp;
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
fn test_args_passing() {
    let dir: &str = "folder";
    assert!(run_cmd!(mkdir /tmp/$dir; ls /tmp/$dir; rmdir /tmp/$dir).is_ok());
    assert!(run_cmd!(|dir| mkdir /tmp/"$dir"; ls /tmp/"$dir"; rmdir /tmp/"$dir").is_ok());
    assert!(run_cmd!(|dir| mkdir "/tmp/$dir"; ls "/tmp/$dir"; rmdir "/tmp/$dir").is_ok());
}

#[test]
fn test_args_with_spaces() {
    let dir: &str = "folder with spaces";
    assert!(run_cmd!(mkdir /tmp/$dir; ls /tmp/$dir; rmdir /tmp/$dir).is_ok());
    assert!(run_cmd!(|dir| mkdir /tmp/"$dir"; ls /tmp/"$dir"; rmdir /tmp/"$dir").is_ok());
    assert!(run_cmd!(|dir| mkdir "/tmp/$dir"; ls "/tmp/$dir"; rmdir "/tmp/$dir").is_ok());
}

#[test]
fn test_args_with_spaces_check_result() {
    let dir: &str = "folder with spaces2";
    assert!(run_cmd!(mkdir /tmp/$dir).is_ok());
    assert!(run_cmd!(ls "/tmp/folder with spaces2").is_ok());
    assert!(run_cmd!(rmdir /tmp/$dir).is_ok());
}

#[test]
fn test_non_string_args() {
    let a = 3;
    assert!(run_cmd!(sleep $a).is_ok());
}
