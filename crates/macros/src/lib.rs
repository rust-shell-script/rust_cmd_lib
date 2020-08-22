///
/// ## run_cmd! --> CmdResult
/// ```rust
/// #[macro_use]
/// use cmd_lib_macros::run_cmd;
///
/// let name = "rust";
/// run_cmd!(echo $name);
/// run_cmd!(|name| echo "hello, $name");
///
/// // pipe commands are also supported
/// run_cmd!(du -ah . | sort -hr | head -n 10);
///
/// // or a group of commands
/// // if any command fails, just return Err(...)
/// let file = "/tmp/f";
/// run_cmd!{
///     date;
///     ls -l $file;
/// };
/// ```
#[macro_export]
macro_rules! run_cmd {
   ($($cur:tt)*) => {
       cmd_lib_core::Parser::new($crate::source_text!(run_cmd).clone())
           .with_lits($crate::parse_string_literal!($($cur)*))
           .with_sym_table($crate::parse_sym_table!($($cur)*))
           .with_location(file!(), line!())
           .parse()
           .run_cmd()
   };
}

/// ## run_fun! --> FunResult
/// ```no_run
/// #[macro_use]
/// use cmd_lib_macros::run_fun;
/// let version = run_fun!(rustc --version).unwrap();
/// eprintln!("Your rust version is {}", version);
///
/// // with pipes
/// let files = run_fun!(du -ah . | sort -hr | head -n 10).unwrap();
/// eprintln!("files: {}", files);
/// ```
#[macro_export]
macro_rules! run_fun {
   ($($cur:tt)*) => {
       cmd_lib_core::Parser::new($crate::source_text!(run_fun).clone())
           .with_lits($crate::parse_string_literal!($($cur)*))
           .with_sym_table($crate::parse_sym_table!($($cur)*))
           .with_location(file!(), line!())
           .parse()
           .run_fun()
   };
}

// Hack here to return orignal macro string
// In the future, use proc macro or wait for std provide such a macro
//
// As for 1.45, the proc_macro::Span::source_text is still unstable:
// https://doc.rust-lang.org/proc_macro/struct.Span.html#method.source_text
//
#[doc(hidden)]
#[macro_export]
macro_rules! source_text {
    ($macro:ident) => {{
        let __st_macro_name = stringify!($macro);
        let mut __st_macro_str = String::new();
        let __st_target_line = line!() as usize;
        let __st_file: Vec<char> = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", file!()))
            .chars()
            .collect();
        let __st_len = __st_file.len();
        let mut __st_i: usize = 0;
        let mut __st_line = 1;
        let mut __st_level = 0;
        while __st_i < __st_len {
            if __st_file[__st_i] == '\n' {
                __st_line += 1;
            }
            if __st_line == __st_target_line {
                let __st_cmp_str: String =
                    __st_file[__st_i..__st_i + __st_macro_name.len()]
                    .iter()
                    .collect();
                if __st_cmp_str == __st_macro_name {
                    __st_i += __st_macro_name.len() + 1;
                    while __st_file[__st_i] != '{' && __st_file[__st_i] != '(' {
                        __st_i += 1;
                    }
                    __st_i += 1;
                    __st_level += 1;

                    let __st_with_quote = __st_file[__st_i] == '"';
                    let mut __st_in_single_quote = false;
                    let mut __st_in_double_quote = false;
                    if __st_with_quote {
                        __st_in_double_quote = true;
                        __st_i += 1;
                    }
                    loop {
                        if !__st_in_single_quote && !__st_in_double_quote {
                            if __st_file[__st_i] == '}' || __st_file[__st_i] == ')' {
                                __st_level -= 1;
                            } else if __st_file[__st_i] == '{' || __st_file[__st_i] == '(' {
                                __st_level += 1;
                            }

                            if __st_level == 0 {
                                break;
                            }
                        }

                        if __st_file[__st_i] == '"' && !__st_in_single_quote {
                            __st_in_double_quote = !__st_in_double_quote;
                        } else if __st_file[__st_i] == '\'' && !__st_in_double_quote {
                            __st_in_single_quote = !__st_in_single_quote;
                        }

                        __st_macro_str.push(__st_file[__st_i]);
                        __st_i += 1;
                    }
                    if __st_with_quote {
                        __st_macro_str.pop();
                    }
                    break;
                }
            }
            __st_i += 1;
        }
        __st_macro_str
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! parse_string_literal {
    (&$sl:expr;) => {
        $sl
    };
    (&$sl:expr; - $($other:tt)*) => {
        $crate::parse_string_literal!{&$sl; $($other)*}
    };
    (&$sl:expr; $cur:literal $($other:tt)*) => {
        let s = stringify!($cur);
        // only save string literals
        if s.starts_with("\"") || s.starts_with("r") {
            $sl.push_back($cur.to_string());
        }
        $crate::parse_string_literal!{&$sl; $($other)*}
    };
    (&$sl:expr; $cur:tt $($other:tt)*) => {
        $crate::parse_string_literal!{&$sl; $($other)*}
    };
    ($cur:tt $($other:tt)*) => {{
        let mut __str_lits = std::collections::VecDeque::new();
        $crate::parse_string_literal!{&__str_lits; $cur $($other)*}
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! parse_sym_table {
    (&$st:expr;) => {
        $st
    };
    (&$st:expr; [$] {$cur:ident} $($other:tt)*) => {
        $st.insert(stringify!($cur), $cur.to_string());
        $crate::parse_sym_table!{&$st; $($other)*}
    };
    (&$st:expr; [$] $cur:ident $($other:tt)*) => {
        $st.insert(stringify!($cur), $cur.to_string());
        $crate::parse_sym_table!{&$st; $($other)*}
    };
    (&$st:expr; [$cur:tt] $($other:tt)*) => {
        $crate::parse_sym_table!{&$st; $($other)*}
    };
    (&$st:expr; $cur:tt $($other:tt)*) => {
        $crate::parse_sym_table!{&$st; [$cur] $($other)*}
    };
    // start: block tokenstream
    (|$arg0:ident $(,$arg:ident)*| $cur:tt $($other:tt)*) => {{
        let mut __sym_table = std::collections::HashMap::new();
        __sym_table.insert(stringify!($arg0), $arg0.to_string());
        $(__sym_table.insert(stringify!($arg), $arg.to_string());)*
        $crate::parse_sym_table!{&__sym_table; [$cur] $($other)*}
    }};
    ($cur:tt $($other:tt)*) => {{
        let mut __sym_table = std::collections::HashMap::new();
        $crate::parse_sym_table!{&__sym_table; [$cur] $($other)*}
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_inside_fun() {
        macro_rules! run_cmd {
            ($($tts:tt)*) => {
                let src = source_text!(run_cmd);
                assert_eq!(src, "cd /tmp; ls /f;");
            };
        }
        run_cmd!(cd /tmp; ls /f;);
    }

    #[test]
    fn test_with_new_lines() {
        macro_rules! run_cmd_for_source_text {
            ($($tts:tt)*) => {
                let src = source_text!(run_cmd);
                let indent1 = " ".repeat(12);
                let indent2 = " ".repeat(8);
                assert_eq!(src, format!("\n{}cd /tmp;\n{}ls ${{f}};\n{}", indent1, indent1, indent2))
            };
        }
        run_cmd_for_source_text!{
            cd /tmp;
            ls ${f};
        }
    }

    #[test]
    fn test_parse_sym_table() {
        let file = "/tmp/file";
        let sym_table1 = parse_sym_table!(ls $file);
        let sym_table2 = parse_sym_table!(ls ${file});
        let sym_table3 = parse_sym_table!(|file| echo "opening ${file}");
        let sym_table4 = parse_sym_table!(|file| echo r"opening ${file}");
        assert_eq!(sym_table1["file"], file);
        assert_eq!(sym_table2["file"], file);
        assert_eq!(sym_table3["file"], file);
        assert_eq!(sym_table4["file"], file);
    }
}

