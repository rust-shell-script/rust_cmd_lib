use cmd_lib::*;

#[test]
#[rustfmt::skip]
fn test_run_single_cmds() {
    assert!(run_cmd!(touch /tmp/xxf).is_ok());
    assert!(run_cmd!(rm /tmp/xxf).is_ok());
}

#[test]
fn test_run_single_cmd_with_quote() {
    assert_eq!(
        run_fun!(echo "hello, rust" | sed r"s/rust/cmd_lib1/g").unwrap(),
        "hello, cmd_lib1"
    );
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
    assert!(run_cmd!(mkdir /tmp/"$dir"; ls /tmp/"$dir"; rmdir /tmp/"$dir").is_err());
    assert!(run_cmd!(mkdir "/tmp/$dir"; ls "/tmp/$dir"; rmdir "/tmp/$dir").is_err());
    assert!(run_cmd!(rmdir "/tmp/$dir").is_ok());
}

#[test]
fn test_args_with_spaces() {
    let dir: &str = "folder with spaces";
    assert!(run_cmd!(rm -rf /tmp/$dir).is_ok());
    assert!(run_cmd!(mkdir /tmp/"$dir"; ls /tmp/"$dir"; rmdir /tmp/"$dir").is_ok());
    assert!(run_cmd!(mkdir /tmp/$dir; ls /tmp/$dir).is_ok());
    assert!(run_cmd!(mkdir /tmp/"$dir"; ls /tmp/"$dir"; rmdir /tmp/"$dir").is_err());
    assert!(run_cmd!(mkdir "/tmp/$dir"; ls "/tmp/$dir"; rmdir "/tmp/$dir").is_err());
    assert!(run_cmd!(rmdir "/tmp/$dir").is_ok());
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
fn test_non_eng_args() {
    let msg = "你好！";
    assert!(run_cmd!(echo "$msg").is_ok());
    assert!(run_cmd!(echo $msg).is_ok());
    assert!(run_cmd!(echo ${msg}).is_ok());
}

#[test]
fn test_vars_in_str0() {
    assert_eq!(run_fun!(echo "$").unwrap(), "$");
}

#[test]
fn test_vars_in_str1() {
    assert_eq!(run_fun!(echo "$$").unwrap(), "$");
    assert_eq!(run_fun!(echo "$$a").unwrap(), "$a");
}

#[test]
fn test_vars_in_str2() {
    assert_eq!(run_fun!(echo "$ hello").unwrap(), "$ hello");
}

#[test]
fn test_vars_in_str3() {
    let msg = "hello";
    assert_eq!(run_fun!(echo "$msg").unwrap(), "hello");
    assert_eq!(run_fun!(echo "$ msg").unwrap(), "$ msg");
}

#[test]
/// ```compile_fail
/// run_cmd!(echo "${msg0}").unwrap();
/// assert_eq!(run_fun!(echo "${ msg }").unwrap(), "${ msg }");
/// assert_eq!(run_fun!(echo "${}").unwrap(), "${}");
/// assert_eq!(run_fun!(echo "${").unwrap(), "${");
/// assert_eq!(run_fun!(echo "${msg").unwrap(), "${msg");
/// assert_eq!(run_fun!(echo "$}").unwrap(), "$}");
/// assert_eq!(run_fun!(echo "${}").unwrap(), "${}");
/// assert_eq!(run_fun!(echo "${").unwrap(), "${");
/// assert_eq!(run_fun!(echo "${0}").unwrap(), "${0}");
/// assert_eq!(run_fun!(echo "${ 0 }").unwrap(), "${ 0 }");
/// assert_eq!(run_fun!(echo "${0msg}").unwrap(), "${0msg}");
/// assert_eq!(run_fun!(echo "${msg 0}").unwrap(), "${msg 0}");
/// assert_eq!(run_fun!(echo "${msg 0}").unwrap(), "${msg 0}");
/// ```
fn test_vars_in_str4() {}

#[test]
fn test_tls_set() {
    tls_init!(V, Vec<String>, vec![]);
    tls_set!(V, |v| v.push("a".to_string()));
    tls_set!(V, |v| v.push("b".to_string()));
    assert_eq!(tls_get!(V)[0], "a");
}

#[test]
fn test_pipe() {
    use_builtin_cmd!(echo);
    assert!(run_cmd!(echo "xx").is_ok());
    assert_eq!(run_fun!(echo "xx").unwrap(), "xx");
    assert!(run_cmd!(echo xx | wc).is_ok());
    assert!(run_cmd!(echo xx | wc | wc | wc | wc).is_ok());
    assert!(run_cmd!(seq 1 10000000 | head -1).is_err());

    assert!(run_cmd!(false | wc).is_err());
    assert!(run_cmd!(echo xx | false | wc | wc | wc).is_err());

    set_pipefail(false);
    assert!(run_cmd!(du -ah . | sort -hr | head -n 10).is_ok());
    set_pipefail(true);

    let wc_cmd = "wc";
    assert!(run_cmd!(ls | $wc_cmd).is_ok());
}

#[test]
/// ```compile_fail
/// run_cmd!(ls > >&1).unwrap();
/// run_cmd!(ls >>&1).unwrap();
/// run_cmd!(ls >>&2).unwrap();
/// ```
fn test_redirect() {
    let tmp_file = "/tmp/f";
    assert!(run_cmd!(echo xxxx > $tmp_file).is_ok());
    assert!(run_cmd!(echo yyyy >> $tmp_file).is_ok());
    assert!(run_cmd!(
        ignore ls /x 2>/tmp/lsx.log;
        echo "dump file:";
        cat /tmp/lsx.log;
        rm /tmp/lsx.log;
    )
    .is_ok());
    assert!(run_cmd!(ignore ls /x 2>/dev/null).is_ok());
    assert!(run_cmd!(ignore ls /x &>$tmp_file).is_ok());
    assert!(run_cmd!(wc -w < $tmp_file).is_ok());
    assert!(run_cmd!(ls 1>&1).is_ok());
    assert!(run_cmd!(ls 2>&2).is_ok());
    let tmp_log = "/tmp/echo_test.log";
    assert_eq!(run_fun!(ls &>$tmp_log).unwrap(), "");
    assert!(run_cmd!(rm -f $tmp_file $tmp_log).is_ok());
}

#[test]
fn test_proc_env() {
    let output = run_fun!(FOO=100 printenv | grep FOO).unwrap();
    assert_eq!(output, "FOO=100");
}

#[test]
fn test_export_cmd() {
    use std::io::Write;
    #[export_cmd(my_cmd)]
    fn foo(env: &mut CmdEnv) -> CmdResult {
        let msg = format!("msg from foo(), args: {:?}", env.args());
        writeln!(env.stderr(), "{}", msg)?;
        writeln!(env.stdout(), "bar")
    }

    #[export_cmd(my_cmd2)]
    fn foo2(env: &mut CmdEnv) -> CmdResult {
        let msg = format!("msg from foo2(), args: {:?}", env.args());
        writeln!(env.stderr(), "{}", msg)?;
        writeln!(env.stdout(), "bar2")
    }
    use_custom_cmd!(my_cmd, my_cmd2);
    assert!(run_cmd!(echo "from" "builtin").is_ok());
    assert!(run_cmd!(my_cmd arg1 arg2).is_ok());
    assert!(run_cmd!(my_cmd2).is_ok());
}

#[test]
fn test_escape() {
    let xxx = 42;
    assert_eq!(
        run_fun!(/bin/echo "\"a你好${xxx}世界b\"").unwrap(),
        "\"a你好42世界b\""
    );
}

#[test]
fn test_current_dir() {
    let path = run_fun!(ls /; cd /tmp; pwd).unwrap();
    assert_eq!(
        std::fs::canonicalize(&path).unwrap(),
        std::fs::canonicalize("/tmp").unwrap()
    );
}

#[test]
/// ```compile_fail
/// run_cmd!(ls / /x &>>> /tmp/f).unwrap();
/// run_cmd!(ls / /x &> > /tmp/f).unwrap();
/// run_cmd!(ls / /x > > /tmp/f).unwrap();
/// run_cmd!(ls / /x >> > /tmp/f).unwrap();
/// ```
fn test_redirect_fail() {}

#[test]
fn test_buitin_stdout_redirect() {
    use_builtin_cmd!(echo);
    let f = "/tmp/builtin";
    let msg = run_fun!(echo xx &> $f).unwrap();
    assert_eq!(msg, "");
    assert_eq!("xx", run_fun!(cat $f).unwrap());
    run_cmd!(rm -f $f).unwrap();
}

#[test]
fn test_path_as_var() {
    let dir = std::path::Path::new("/");
    assert_eq!("/", run_fun!(cd $dir; pwd).unwrap());

    let dir2 = std::path::PathBuf::from("/");
    assert_eq!("/", run_fun!(cd $dir2; pwd).unwrap());
}
