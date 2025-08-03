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
fn test_pipe() -> CmdResult {
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

    // test `ignore` command and pipefail mode
    // FIXME: make set_pipefail() thread safe, then move this to a separate test_ignore_and_pipefail()
    struct TestCase {
        /// Run the test case, returning whether the result `.is_ok()`.
        code: fn() -> bool,
        /// Stringified version of `code`, for identifying assertion failures.
        code_str: &'static str,
        /// Do we expect `.is_ok()` when pipefail is on?
        expected_ok_pipefail_on: bool,
        /// Do we expect `.is_ok()` when pipefail is off?
        expected_ok_pipefail_off: bool,
    }
    /// Make a function for [TestCase::code].
    ///
    /// Usage: `code!((macro!(command)).extra)`
    /// - `(macro!(command)).extra` is an expression of type CmdResult
    macro_rules! code {
        (($macro:tt $bang:tt ($($command:tt)+)) $($after:tt)*) => {
            || $macro$bang($($command)+)$($after)*.is_ok()
        };
    }
    /// Make a string for [TestCase::code_str].
    ///
    /// Usage: `code_str!((macro!(command)).extra)`
    /// - `(macro!(command)).extra` is an expression of type CmdResult
    macro_rules! code_str {
        (($macro:tt $bang:tt ($($command:tt)+)) $($after:tt)*) => {
            stringify!($macro$bang($($command)+)$($after)*.is_ok())
        };
    }
    /// Make a [TestCase].
    /// Usage: `test_case!(true/false, true/false, (macro!(command)).extra)`
    /// - the first `true/false` is TestCase::expected_ok_pipefail_on
    /// - the second `true/false` is TestCase::expected_ok_pipefail_off
    /// - `(macro!(command)).extra` is an expression of type CmdResult
    macro_rules! test_case {
        ($expected_ok_pipefail_on:expr, $expected_ok_pipefail_off:expr, ($macro:tt $bang:tt ($($command:tt)+)) $($after:tt)*) => {
            TestCase {
                code: code!(($macro $bang ($($command)+)) $($after)*),
                code_str: code_str!(($macro $bang ($($command)+)) $($after)*),
                expected_ok_pipefail_on: $expected_ok_pipefail_on,
                expected_ok_pipefail_off: $expected_ok_pipefail_off,
            }
        };
    }
    /// Generate test cases for the given entry point.
    /// For each test case, every entry point should yield the same results.
    macro_rules! test_cases_for_entry_point {
        (($macro:tt $bang:tt (...)) $($after:tt)*) => {
            &[
                // Use result of last command in pipeline, if all others exit successfully.
                test_case!(true, true, ($macro $bang (true)) $($after)*),
                test_case!(false, false, ($macro $bang (false)) $($after)*),
                test_case!(true, true, ($macro $bang (true | true)) $($after)*),
                test_case!(false, false, ($macro $bang (true | false)) $($after)*),
                // Use failure of other commands, if pipefail is on.
                test_case!(false, true, ($macro $bang (false | true)) $($after)*),
                // Use failure of last command in pipeline.
                test_case!(false, false, ($macro $bang (false | false)) $($after)*),
                // Ignore all failures, when using `ignore` command.
                test_case!(true, true, ($macro $bang (ignore true)) $($after)*),
                test_case!(true, true, ($macro $bang (ignore false)) $($after)*),
                test_case!(true, true, ($macro $bang (ignore true | true)) $($after)*),
                test_case!(true, true, ($macro $bang (ignore true | false)) $($after)*),
                test_case!(true, true, ($macro $bang (ignore false | true)) $($after)*),
                test_case!(true, true, ($macro $bang (ignore false | false)) $($after)*),
                // Built-ins should work too, without locking up.
                test_case!(true, true, ($macro $bang (echo)) $($after)*),
                test_case!(true, true, ($macro $bang (echo | true)) $($after)*),
                test_case!(false, false, ($macro $bang (echo | false)) $($after)*),
                test_case!(true, true, ($macro $bang (true | echo)) $($after)*),
                test_case!(false, true, ($macro $bang (false | echo)) $($after)*),
                test_case!(true, true, ($macro $bang (cd /)) $($after)*),
                test_case!(true, true, ($macro $bang (cd / | true)) $($after)*),
                test_case!(false, false, ($macro $bang (cd / | false)) $($after)*),
                test_case!(true, true, ($macro $bang (true | cd /)) $($after)*),
                test_case!(false, true, ($macro $bang (false | cd /)) $($after)*),
            ]
        };
    }

    let test_cases: &[&[TestCase]] = &[
        test_cases_for_entry_point!((run_cmd!(...))),
        test_cases_for_entry_point!((run_fun!(...)).map(|_stdout| ())),
        test_cases_for_entry_point!((spawn!(...)).unwrap().wait()),
        test_cases_for_entry_point!((spawn_with_output!(...)).unwrap().wait_with_all().0),
        test_cases_for_entry_point!((spawn_with_output!(...))
            .unwrap()
            .wait_with_output()
            .map(|_stdout| ())),
        test_cases_for_entry_point!((spawn_with_output!(...))
            .unwrap()
            .wait_with_raw_output(&mut vec![])),
        test_cases_for_entry_point!((spawn_with_output!(...))
            .unwrap()
            .wait_with_borrowed_pipe(&mut |_stdout| {})),
    ];

    macro_rules! check_eq {
        ($left:expr, $right:expr, $($rest:tt)+) => {{
            let left = $left;
            let right = $right;
            if left != right {
                eprintln!("assertion failed ({} != {}): {}", left, right, format!($($rest)+));
                false
            } else {
                true
            }
        }};
    }

    let mut ok = true;
    for case in test_cases.iter().flat_map(|items| items.iter()) {
        ok &= check_eq!(
            (case.code)(),
            case.expected_ok_pipefail_on,
            "{} when pipefail is on",
            case.code_str
        );
        set_pipefail(false);
        ok &= check_eq!(
            (case.code)(),
            case.expected_ok_pipefail_off,
            "{} when pipefail is off",
            case.code_str
        );
        set_pipefail(true);
    }

    assert!(ok);

    // test that illustrates the bugs in wait_with_pipe()
    // FIXME: make set_pipefail() thread safe, then move this to a separate test function
    assert!(spawn_with_output!(false)?.wait_with_all().0.is_err());
    assert!(spawn_with_output!(false)?.wait_with_output().is_err());
    assert!(spawn_with_output!(false)?
        .wait_with_raw_output(&mut vec![])
        .is_err());

    // wait_with_pipe() can’t check the exit status of the last child
    assert!(spawn_with_output!(false)?
        .wait_with_pipe(&mut |_stdout| {})
        .is_ok());

    // wait_with_pipe() kills the last child when the provided function returns
    assert!(spawn_with_output!(sh -c "while :; do :; done")?
        .wait_with_pipe(&mut |_stdout| {})
        .is_ok());

    // wait_with_borrowed_pipe() checks the exit status of the last child, even if pipefail is disabled
    set_pipefail(false);
    assert!(spawn_with_output!(true | false)?
        .wait_with_borrowed_pipe(&mut |_stdout| {})
        .is_err());
    assert!(spawn_with_output!(true | true)?
        .wait_with_borrowed_pipe(&mut |_stdout| {})
        .is_ok());
    assert!(spawn_with_output!(false)?
        .wait_with_borrowed_pipe(&mut |_stdout| {})
        .is_err());
    assert!(spawn_with_output!(true)?
        .wait_with_borrowed_pipe(&mut |_stdout| {})
        .is_ok());
    set_pipefail(true);
    // wait_with_borrowed_pipe() checks the exit status of the other children, unless pipefail is disabled
    set_pipefail(false);
    assert!(spawn_with_output!(false | true)?
        .wait_with_borrowed_pipe(&mut |_stdout| {})
        .is_ok());
    set_pipefail(true);
    assert!(spawn_with_output!(false | true)?
        .wait_with_borrowed_pipe(&mut |_stdout| {})
        .is_err());
    assert!(spawn_with_output!(true | true)?
        .wait_with_borrowed_pipe(&mut |_stdout| {})
        .is_ok());
    // wait_with_borrowed_pipe() handles `ignore`
    assert!(spawn_with_output!(ignore false | true)?
        .wait_with_borrowed_pipe(&mut |_stdout| {})
        .is_ok());
    assert!(spawn_with_output!(ignore true | false)?
        .wait_with_borrowed_pipe(&mut |_stdout| {})
        .is_ok());
    assert!(spawn_with_output!(ignore false)?
        .wait_with_borrowed_pipe(&mut |_stdout| {})
        .is_ok());

    Ok(())
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
    fn my_cmd(env: &mut CmdEnv) -> CmdResult {
        let msg = format!("msg from foo(), args: {:?}", env.get_args());
        writeln!(env.stderr(), "{}", msg)?;
        writeln!(env.stdout(), "bar")
    }

    fn my_cmd2(env: &mut CmdEnv) -> CmdResult {
        let msg = format!("msg from foo2(), args: {:?}", env.get_args());
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
        run_fun!(echo "\"a你好${xxx}世界b\"").unwrap(),
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

#[test]
fn test_empty_arg() {
    let opt = "";
    assert!(run_cmd!(ls $opt).is_ok());
}

#[test]
fn test_env_var_with_equal_sign() {
    assert!(run_cmd!(A="-c B=c" echo).is_ok());
}
