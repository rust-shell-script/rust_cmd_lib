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
        let macro_name = stringify!($macro);
        let mut macro_str = String::new();
        let src = String::from(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), file!()));
        let target_line = line!() as usize;
        let file: Vec<char> = std::fs::read_to_string(src)
            .expect("error reading file")
            .chars()
            .collect();
        let len = file.len();
        let mut i: usize = 0;
        let mut line = 1;
        let mut level = 0;
        while i < len {
            if file[i] == '\n' {
                line += 1;
            }
            if line == target_line {
                let cmp_str: String = file[i..i + macro_name.len()].iter().collect();
                if cmp_str == macro_name {
                    i += macro_name.len() + 1;
                    while file[i] != '{' && file[i] != '(' {
                        i += 1;
                    }
                    i += 1;
                    level += 1;

                    let with_quote = file[i] == '"';
                    let mut in_single_quote = false;
                    let mut in_double_quote = false;
                    if with_quote {
                        in_double_quote = true;
                        i += 1;
                    }
                    loop {
                        if !in_single_quote && !in_double_quote {
                            if file[i] == '}' || file[i] == ')' {
                                level -= 1;
                            } else if file[i] == '{' || file[i] == '(' {
                                level += 1;
                            }

                            if level == 0 {
                                break;
                            }
                        }

                        if file[i] == '"' && !in_single_quote {
                            in_double_quote = !in_double_quote;
                        } else if file[i] == '\'' && !in_double_quote {
                            in_single_quote = !in_single_quote;
                        }

                        macro_str.push(file[i]);
                        i += 1;
                    }
                    if with_quote {
                        macro_str.pop();
                    }
                    break;
                }
            }
            i += 1;
        }
        macro_str
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_inside_fun() {
        macro_rules! run_cmd {
            ($($tts:tt)*) => {
                let src = source_text!(run_cmd);
                assert!(src == "cd /tmp; ls /f;");
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
                let expected = format!("\n{}cd /tmp;\n{}ls ${{f}};\n{}", indent1, indent1, indent2);
                if (src != expected) {
                    eprintln!("src:\n#{}#", src);
                    eprintln!("expected:\n#{}#", expected);
                    panic!("source text mismatch");
                }
            };
        }
        run_cmd_for_source_text!{
            cd /tmp;
            ls ${f};
        }
    }
}
