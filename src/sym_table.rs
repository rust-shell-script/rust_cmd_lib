use std::collections::HashMap;

#[doc(hidden)]
#[macro_export]
macro_rules! parse_sym_table {
    (&$st:expr;) => {
        $st
    };
    (&$st:expr; [$] {$cur:ident} $($other:tt)*) => {
        $st.insert(stringify!($cur).to_owned(), $cur.to_string());
        $crate::parse_sym_table!{&$st; $($other)*}
    };
    (&$st:expr; [$] $cur:ident $($other:tt)*) => {
        $st.insert(stringify!($cur).to_owned(), $cur.to_string());
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
        __sym_table.insert(stringify!($arg0).to_owned(), $arg0.to_string());
        $(__sym_table.insert(stringify!($arg).to_owned(), $arg.to_string());)*
        $crate::parse_sym_table!{&__sym_table; [$cur] $($other)*}
    }};
    ($cur:tt $($other:tt)*) => {{
        let mut __sym_table = std::collections::HashMap::new();
        $crate::parse_sym_table!{&__sym_table; [$cur] $($other)*}
    }};
}

#[doc(hidden)]
pub(crate) fn resolve_name(src: &str, sym_table: &HashMap<String, String>, file: &str, line: u32) -> String {
    let mut output = String::new();
    let input: Vec<char> = src.chars().collect();
    let len = input.len();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    let mut i = 0;
    while i < len {
        if i == 0 {
            // skip variable declaration part
            while input[i] == ' ' || input[i] == '\t' || input[i] == '\n' {
                i += 1;
            }
            if input[i] == '|' {
                i += 1;
                while i < len && input[i] != '|' {
                    i += 1;
                }
                i += 1;
            }
            while input[i] == ' ' || input[i] == '\t' || input[i] == '\n' {
                i += 1;
            }
        }

        if input[i] == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
        } else if input[i] == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
        }

        if !in_single_quote && i < len - 1 && input[i] == '$' {
            i += 1;
            let with_bracket = i < len && input[i] == '{';
            let mut var = String::new();
            if with_bracket { i += 1; }
            while i < len
                && ((input[i] >= 'a' && input[i] <= 'z')
                    || (input[i] >= 'A' && input[i] <= 'Z')
                    || (input[i] >= '0' && input[i] <= '9')
                    || (input[i] == '_'))
            {
                var.push(input[i]);
                i += 1;
            }
            if with_bracket {
                if input[i] != '}' {
                    panic!("invalid name {}, {}:{}\n{}", var, file, line, src);
                }
            } else {
                i -= 1; // back off 1 char
            }
            match sym_table.get(&var) {
                None => {
                    panic!("resolve {} failed, {}:{}\n{}", var, file, line, src);
                }
                Some(v) => {
                    if in_double_quote {
                        output += v;
                    } else {
                        output += "\"";
                        output += v;
                        output += "\"";
                    }
                }
            }
        } else {
            output.push(input[i]);
        }
        i += 1;
    }

    output
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_sym_table() {
        let file = "/tmp/file";
        let sym_table1 = parse_sym_table!(ls $file);
        let sym_table2 = parse_sym_table!(ls ${file});
        let sym_table3 = parse_sym_table!(|file| echo "opening ${file}");
        assert!(sym_table1["file"] == file);
        assert!(sym_table2["file"] == file);
        assert!(sym_table3["file"] == file);
    }

    #[test]
    fn test_resolve_name() {
        use crate::source_text;
        macro_rules! get_cmd_for_sym_table {
            ($($tts:tt)*) => {{
                let src = source_text!(get_cmd_for_sym_table);
                let sym_table = parse_sym_table!($($tts)*);
                $crate::sym_table::resolve_name(&src, &sym_table, &file!(), line!())
            }};
        }
        let file1 = "/tmp/resolve";
        let cmd1 = get_cmd_for_sym_table!(touch $file1);
        eprintln!("{}", cmd1);
        assert!(cmd1 == "touch \"/tmp/resolve\"");

        let folder1 = "my folder";
        let cmd2 = get_cmd_for_sym_table!(mkdir $folder1);
        eprintln!("{}", cmd2);
        assert!(cmd2 == "mkdir \"my folder\"");

        let name = "rust";
        let project = "rust-shell-script";
        let cmd3 = get_cmd_for_sym_table!(|name, project| echo "hello, $name from $project");
        eprintln!("{}", cmd3);
        assert!(cmd3 == "echo \"hello, rust from rust-shell-script\"");
    }
}
