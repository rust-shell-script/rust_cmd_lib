extern crate cmd_lib;
use cmd_lib::{run_cmd, run_fun, env_set, Env};

#[test]
fn test_run_cmd() {
    assert!(run_cmd!(touch /tmp/xxf; rm /tmp/xxf).is_ok());
}

#[test]
fn test_run_cmds() {
    assert!(run_cmd! {
        lcd /tmp;
        ls | wc -l;
    }.is_ok());
}

#[test]
fn test_run_fun() {
    assert!(run_fun!(uptime).is_ok());
}

#[test]
fn test_args_passing() {
    let dir: &str = "folder";
    assert!(run_cmd!(rm -rf /tmp/$dir).is_ok());
    assert!(run_cmd!(mkdir /tmp/$dir; ls /tmp/$dir).is_ok());
    assert!(run_cmd!(|dir| mkdir /tmp/"$dir"; ls /tmp/"$dir"; rmdir /tmp/"$dir").is_err());
    assert!(run_cmd!(|dir| mkdir "/tmp/$dir"; ls "/tmp/$dir"; rmdir "/tmp/$dir").is_err());
    assert!(run_cmd!(|dir| rmdir "/tmp/$dir").is_ok());
}

#[test]
fn test_args_with_spaces() {
    let dir: &str = "folder with spaces";
    assert!(run_cmd!(rm -rf /tmp/$dir).is_ok());
    assert!(run_cmd!(mkdir /tmp/$dir; ls /tmp/$dir).is_ok());
    assert!(run_cmd!(|dir| mkdir /tmp/"$dir"; ls /tmp/"$dir"; rmdir /tmp/"$dir").is_err());
    assert!(run_cmd!(|dir| mkdir "/tmp/$dir"; ls "/tmp/$dir"; rmdir "/tmp/$dir").is_err());
    assert!(run_cmd!(|dir| rmdir "/tmp/$dir").is_ok());
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
    let a = 1;
    assert!(run_cmd!(sleep $a).is_ok());
}

#[test]
fn test_env_set() {
    env_set!(PWD = "/var/tmp");
    assert_eq!(std::env::var("RUST_CMD_LIB_PWD".to_owned()), Ok("/var/tmp".to_owned()));
}
