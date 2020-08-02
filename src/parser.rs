use std::collections::{VecDeque, HashMap};
use crate::sym_table;

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
        if s.starts_with("\"") || s.starts_with("r") || s.starts_with("b") {
            $sl.push_back($cur.to_string());
        }
        $crate::parse_string_literal!{&$sl; $($other)*}
    };
    (&$sl:expr; $cur:tt $($other:tt)*) => {
        $crate::parse_string_literal!{&$sl; $($other)*}
    };
    ($cur:tt $($other:tt)*) => {{
        let mut __str_lits = std::collections::VecDeque::<String>::new();
        $crate::parse_string_literal!{&__str_lits; $cur $($other)*}
    }};
}

pub fn parse(s: &str,
             lits: &mut VecDeque<String>,
             sym_table: &HashMap<String, String>,
             file: &str,
             line: u32) -> Vec<Vec<Vec<String>>> {
    let mut ret = Vec::new();
    let s: Vec<char> = s.chars().collect();
    let len = s.len();
    let mut i = 0;

    // skip leading spaces
    while i < len  && char::is_whitespace(s[i]) { i += 1; }
    if i == len { return ret; }

    // skip variables declaration part
    if i < len && s[i] == '|' {
        i += 1;
        while i < len && s[i] != '|' { i += 1; }
        i += 1;
    }

    // real commands parsing starts
    while i < len {
        while i < len && char::is_whitespace(s[i]) { i += 1; }
        if i == len { break; }

        let cmd = parse_cmd(&s, &mut i, lits, sym_table, file, line);
        if !cmd.is_empty() {
            ret.push(cmd);
        }
    }
    ret
}

fn parse_cmd(s: &Vec<char>,
             i: &mut usize,
             lits: &mut VecDeque<String>,
             sym_table: &HashMap<String, String>,
             file: &str,
             line: u32) -> Vec<Vec<String>> {
    let mut ret = Vec::new();
    let len = s.len();
    while *i < len && s[*i] != ';' {
        while *i < len && char::is_whitespace(s[*i]) { *i += 1; }
        if *i == len { break; }

        let pipe = parse_pipe(s, i, lits, sym_table, file, line);
        if !pipe.is_empty() {
            ret.push(pipe);
        }
    }
    if *i < len && s[*i] == ';' { *i += 1; }
    ret
}

fn parse_pipe(s: &Vec<char>,
              i: &mut usize,
              lits: &mut VecDeque<String>,
              sym_table: &HashMap<String, String>,
              file: &str,
              line: u32) -> Vec<String> {
    let mut ret = Vec::new();
    let len = s.len();
    while *i < len && s[*i] != '|' && s[*i] != ';' {
        while *i < len && char::is_whitespace(s[*i]) { *i += 1; }
        if *i == len { break; }
        let mut arg = String::new();
        let mut is_ended = false;

        while *i < len && !is_ended {
            let mut cnt = 0;    // '#' counts for raw string literal
            if s[*i] == 'r' || s[*i] == 'b' {
                let mut j = *i + 1;
                while j < len && s[j] == '#' { j += 1; }
                if j < len && s[j] == '\"' {
                    cnt = j - *i - 1;
                    *i = j;
                }
            }

            let mut cnt2 = cnt;
            if s[*i] == '\"' {
                *i += 1;
                while *i < len && (s[*i] != '\"' || s[*i - 1] == '\\' || cnt2 > 0) {
                    if s[*i] == '\"' && s[*i - 1] != '\\' { cnt2 -= 1; }
                    *i += 1;
                }
                *i += 1;
                while *i < len && cnt > 0 {
                    eprintln!("s[{}]: {}", *i, s[*i]);
                    if s[*i] != '#' {
                        eprintln!("cnt: {}", cnt);
                        panic!("invalid raw string literal {}:{}", file, line);
                    }
                    *i += 1;
                    cnt -= 1;
                }
                arg.push_str(&lits.pop_front().unwrap());
            }

            while *i < len {
                if s[*i] == '|' || s[*i] == ';' || char::is_whitespace(s[*i]) {
                    is_ended = true;
                    break;
                }
                if s[*i] == '\"' && s[*i - 1] != '\\' {
                    break;
                }
                arg.push(s[*i]);
                *i += 1;
            }
        }
        if !arg.is_empty() {
            ret.push(sym_table::resolve_name(&arg, sym_table, file, line));
        }
    }
    if *i < len && s[*i] == '|' { *i += 1; }
    ret
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_string_literal() {
        let str_lits1 = parse_string_literal!(ls "/tmp" "/");
        assert_eq!(str_lits1, ["/tmp", "/"]);

        let str_lits2 = parse_string_literal!(ping -c 3 r"127.0.0.1");
        assert_eq!(str_lits2, ["127.0.0.1"]);

        let str_lits3 = parse_string_literal!(echo r#"rust"cmd_lib"#);
        assert_eq!(str_lits3, ["rust\"cmd_lib"]);
    }
}

