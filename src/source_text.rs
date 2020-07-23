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
        let __st_src = String::from(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), file!()));
        let __st_target_line = line!() as usize;
        let __st_file: Vec<char> = std::fs::read_to_string(__st_src)
            .expect("error reading file")
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
}
