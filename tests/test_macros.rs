extern crate cmd_lib;
use cmd_lib::{run_cmd, run_fun, proc_env_set, Env};

#[test]
fn test_run_single_cmd() {
    assert!(run_cmd!(touch /tmp/xxf).is_ok());
    assert!(run_cmd!(rm /tmp/xxf).is_ok());
}

#[test]
fn test_run_builtin_cmds() {
    assert!(run_cmd! {
        cd /tmp;
        ls | wc -l;
    }.is_ok());
}

#[test]
fn test_cd_fails() {
    assert!(run_cmd! {
        cd /bad_dir;
        ls | wc -l;
    }.is_err());
}

#[test]
fn test_run_cmds() {
    assert!(run_cmd! {
        cd /tmp;
        touch xxff;
        ls | wc -l;
        rm xxff;
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
fn test_proc_env_set() {
    proc_env_set!(PWD = "/var/tmp");
    assert_eq!(std::env::var("CMD_LIB_PWD".to_owned()), Ok("/var/tmp".to_owned()));
}
