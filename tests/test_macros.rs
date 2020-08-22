extern crate cmd_lib;
use cmd_lib::{proc_env_set, proc_var, proc_var_set, proc_var_get, run_cmd, run_fun};

#[test]
#[rustfmt::skip]
fn test_run_single_cmds() {
    assert!(run_cmd!(touch /tmp/xxf).is_ok());
    assert!(run_cmd!(rm /tmp/xxf).is_ok());
}

#[test]
#[rustfmt::skip]
fn test_run_single_cmd_with_quote() {
    assert_eq!(
        run_fun!(echo "hello, rust" | sed r"s/rust/cmd_lib1/g").unwrap(),
        "hello, cmd_lib1"
    );
}

#[test]
fn test_run_builtin_cmds() {
    assert!(run_cmd! {
        cd /tmp;
        ls | wc -l;
    }
    .is_ok());
}

#[test]
fn test_cd_fails() {
    assert!(run_cmd! {
        cd /bad_dir;
        ls | wc -l;
    }
    .is_err());
}

#[test]
fn test_or_cmd() {
    assert!(run_cmd! {
        ls /nofile || true;
        echo "continue";
    }
    .is_ok());
}

#[test]
fn test_run_cmds() {
    assert!(run_cmd! {
        cd /tmp;
        touch xxff;
        ls | wc -l;
        rm xxff;
    }
    .is_ok());
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
    assert!(run_cmd!(|dir| mkdir /tmp/"$dir"; ls /tmp/"$dir"; rmdir /tmp/"$dir").is_ok());
    assert!(run_cmd!(mkdir /tmp/$dir; ls /tmp/$dir).is_ok());
    assert!(run_cmd!(|dir| mkdir /tmp/"$dir"; ls /tmp/"$dir"; rmdir /tmp/"$dir").is_err());
    assert!(run_cmd!(|dir| mkdir "/tmp/$dir"; ls "/tmp/$dir"; rmdir "/tmp/$dir").is_err());
    assert!(run_cmd!(|dir| rmdir "/tmp/$dir").is_ok());
}

#[test]
fn test_args_with_spaces_check_result() {
    let dir: &str = "folder with spaces2";
    assert!(run_cmd!(rm -rf /tmp/$dir).is_ok());
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
    let pwd = run_fun!(pwd).unwrap();
    assert_eq!(
        pwd,
        std::fs::canonicalize("/var/tmp").unwrap().to_str().unwrap()
    );
}

#[test]
fn test_proc_var_set() {
    proc_var!(V, Vec<String>, vec![]);
    proc_var_set!(V, |v| v.push("a".to_string()));
    proc_var_set!(V, |v| v.push("b".to_string()));
    assert_eq!(proc_var_get!(V)[0], "a");
}
