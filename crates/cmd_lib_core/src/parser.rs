use std::collections::{VecDeque, HashMap};
use crate::process::{GroupCmds, Cmds, Cmd, FdOrFile};

#[doc(hidden)]
pub struct Parser {
    str_lits: Option<VecDeque<String>>,
    sym_table: Option<HashMap<&'static str, String>>,

    file: &'static str,
    line: u32,

    src: String,
}

impl Parser {
    pub fn new<S: Into<String>>(src: S) -> Self {
        Self {
            str_lits: None,
            sym_table: None,
            file: "",
            line: 0,
            src: src.into(),
        }
    }

    pub fn with_lits(&mut self, str_lits: VecDeque<String>) -> &mut Self {
        self.str_lits = Some(str_lits);
        self
    }

    pub fn with_sym_table(&mut self, sym_table: HashMap<&'static str, String>) -> &mut Self {
        self.sym_table = Some(sym_table);
        self
    }

    pub fn with_location(&mut self, file: &'static str, line: u32) -> &mut Self {
        self.file = file;
        self.line = line;
        self
    }

    fn resolve_name(&self, src: String) -> String {
        if self.sym_table.is_none() {
            return src;
        }

        let mut output = String::new();
        let input: Vec<char> = src.chars().collect();
        let len = input.len();

        let mut i = 0;
        while i < len {
            if input[i] == '$' && (i == 0 || input[i - 1] != '\\') {
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
                        panic!("invalid name {}, {}:{}\n{}", var, self.file, self.line, src);
                    }
                } else {
                    i -= 1; // back off 1 char
                }
                match self.sym_table.as_ref().unwrap().get(var.as_str()) {
                    None => panic!("resolve {} failed, {}:{}\n{}", var, self.file, self.line, src),
                    Some(v) => output += v,
                };
            } else {
                output.push(input[i]);
            }
            i += 1;
        }

        output
    }

    pub fn parse(&mut self) -> GroupCmds {
        let mut ret = GroupCmds::new();
        let s: Vec<char> = self.src.chars().collect();
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

            let cmd = self.parse_cmd(&s, &mut i);
            if !cmd.0.is_empty() {
                ret.add(cmd.0, cmd.1);
            }

            // skip comments
            while i < len  && char::is_whitespace(s[i]) { i += 1; }
            if i == len { break; }
            if i + 1 < len && s[i] == '/' && s[i + 1] == '/' {
                i += 2;
                while i < len && s[i] != '\n' { i += 1; }
            }
        }
        ret
    }

    fn parse_cmd(&mut self, s: &Vec<char>, i: &mut usize) -> (Cmds, Option<Cmds>) {
        let mut ret = vec![Cmds::new(), Cmds::new()];
        let len = s.len();
        for j in 0..2 {
            while *i < len && s[*i] != ';' {
                while *i < len && char::is_whitespace(s[*i]) { *i += 1; }
                if *i == len { break; }

                let cmd = self.parse_pipe(s, i);
                if !cmd.is_empty() {
                    ret[j].pipe(cmd);
                }
                if *i < len && s[*i] == '|' {
                    break;
                }
            }
            if *i < len && s[*i] == '|' {
                assert_eq!(s[*i + 1], '|');
                *i += 2;    // skip "||" operator
            } else {
                break;
            }
        }
        if *i < len && s[*i] == ';' { *i += 1; }
        let (ret1, ret0) = (ret.pop().unwrap(), ret.pop().unwrap());
        (ret0, if ret1.is_empty() { None } else { Some(ret1) })
    }

    fn parse_pipe(&mut self, s: &Vec<char>, i: &mut usize) -> Cmd {
        let mut ret = Cmd::new();
        let len = s.len();
        while *i < len && s[*i] != '|' && s[*i] != ';' {
            while *i < len && char::is_whitespace(s[*i]) { *i += 1; }
            if *i == len { break; }
            let mut arg = String::new();
            while *i < len &&
                  !(s[*i] == '|' || s[*i] == ';' || char::is_whitespace(s[*i])) {
                if s[*i] == 'r' || s[*i] == 'b' ||
                   (s[*i] == '\"' && (*i == 0 || s[*i - 1] != '\\')) {
                    arg += &self.parse_str_lit(s, i);
                }

                if *i < len && s[*i] == '>' {
                    *i += 1;
                    if !arg.is_empty() {
                        if arg == "&" {     // "&> f" equals to ">&2 2>f"
                            ret.set_redirect(2, self.parse_redirect(s, i));
                            ret.set_redirect(1, FdOrFile::Fd(2, false));
                            arg.clear();
                        } else if let Ok(fd) = arg.parse::<i32>() {
                            if fd != 1 && fd != 2 {
                                panic!("fd redirect only support stdout(1) and stderr(2) {}:{}", self.file, self.line);
                            }
                            ret.set_redirect(fd, self.parse_redirect(s, i));
                            arg.clear();
                        } else {
                            ret.set_redirect(1, self.parse_redirect(s, i));
                        }
                    } else {
                        ret.set_redirect(1, self.parse_redirect(s, i));
                    }
                }

                if *i < len && s[*i] == '<' {
                    *i += 1;
                    ret.set_redirect(0, self.parse_redirect(s, i));
                }

                let arg1 = self.parse_normal_arg(s, i);
                arg += &self.resolve_name(arg1);
            }
            if !arg.is_empty() {
                ret.add_arg(arg);
            }
        }
        if *i < len && s[*i] == '|' {
            if *i + 1 < len && s[*i + 1] != '|' {
                *i += 1;
            }
        }
        ret
    }

    fn parse_normal_arg(&mut self, s: &Vec<char>, i: &mut usize) -> String {
        let mut arg = String::new();
        let len = s.len();
        while *i < len &&
              !(s[*i] == '|' || s[*i] == ';' || char::is_whitespace(s[*i])) {
            if s[*i] == '\"' && s[*i - 1] != '\\' { // normal string literal
                break;
            }

            if s[*i] == 'r' || s[*i] == 'b' {
                let mut j = *i + 1;
                while j < len && s[j] == '#' { j += 1; }
                if j < len && s[j] == '\"' {        // raw string literal
                    break;
                }
            }

            if s[*i] == '>' {                       // stdout redirect
                break;
            }

            if s[*i] == '<' {                       // stdin redirect
                break;
            }

            arg.push(s[*i]);
            *i += 1;
        }
        arg
    }

    fn parse_redirect(&mut self, s: &Vec<char>, i: &mut usize) -> FdOrFile {
        let mut append = false;
        let len = s.len();

        if *i < len && s[*i] == '>' {
            append = true;
            *i += 1;
        }

        if *i < len && s[*i] == '&' {
            let mut fd_str = String::new();
            *i += 1;
            while *i < len && s[*i].is_digit(10) {
                fd_str.push(s[*i]);
                *i += 1;
            }
            return FdOrFile::Fd(fd_str.parse().unwrap(), append);
        }

        while *i < len && char::is_whitespace(s[*i]) {
            *i += 1;
        }

        if s[*i] == '&' {
            panic!("syntax error near unexpected token `&' at {}:{}", self.file, self.line);
        }

        if s[*i] == 'r' || s[*i] == 'b' ||
           (s[*i] == '\"' && (*i == 0 || s[*i - 1] != '\\')) {
            let file = self.parse_str_lit(s, i);
            if !file.is_empty() {
                return FdOrFile::File(file, append);
            }
        }

        let file = self.parse_normal_arg(s, i);
        FdOrFile::File(self.resolve_name(file), append)
    }

    fn parse_str_lit(&mut self, s: &Vec<char>, i: &mut usize) -> String {
        let mut str_lit = String::new();
        let len = s.len();
        let mut is_str_lit = false;
        let mut is_raw = false;
        let mut cnt = 0;    // '#' counts for raw string literal
        if s[*i] == 'r' || s[*i] == 'b' {
            let mut j = *i + 1;
            while j < len && s[j] == '#' { j += 1; }
            if j < len && s[j] == '\"' {
                is_str_lit = true;
                is_raw = true;
                cnt = j - *i - 1;
                *i = j + 1;
            }
        } else if s[*i] == '\"' && (*i == 0 || s[*i - 1] != '\\') {
            is_str_lit = true;
            *i += 1;
        }

        if !is_str_lit {
            return "".to_string();
        }

        let mut found_end = false;
        while *i < len && !found_end {
            if s[*i] == '\"' {
                let mut cnt2 = cnt;
                let mut j = *i + 1;
                while j < len && cnt2 > 0 && s[j] == '#' {
                    cnt2 -= 1;
                    j += 1;
                }
                if cnt2 == 0 {
                    found_end = true;
                    *i = j;
                    break;
                }
            }
            str_lit.push(s[*i]);
            *i += 1;
        }
        if !found_end {
            panic!("invalid raw string literal at {}:{}", self.file, self.line);
        }

        if self.str_lits.is_none() {
            return str_lit;
        }

        str_lit = self.str_lits.as_mut().unwrap().pop_front().unwrap().to_string();
        if is_raw {
            return str_lit; // don't resolve names for raw string literals
        } else {
            return self.resolve_name(str_lit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_or_cmd() {
        assert!(Parser::new("ls /nofile || true; echo continue")
                .parse()
                .run_cmd()
                .is_ok());
    }

    #[test]
    fn test_parser_stdout_redirect() {
        Parser::new("echo rust > /tmp/echo_rust").parse();
        Parser::new("echo rust >&2").parse();
        assert!(Parser::new("rm /tmp/echo_rust").parse().run_cmd().is_ok());
    }
}

