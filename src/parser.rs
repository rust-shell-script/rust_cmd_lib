pub(crate) fn parse_cmds(s: &str) -> String {
    let is_cmd_ended = |c| c == ';';
    parse_seps(s, is_cmd_ended)
}

pub(crate) fn parse_pipes(s: &str) -> String {
    let is_pipe_ended = |c| c == '|';
    parse_seps(s, is_pipe_ended)
}

pub(crate) fn parse_cmd_args(s: &str) -> String {
    parse_seps(s, char::is_whitespace)
}

pub(crate) fn parse_cmd_argv(s: String) -> Vec<String> {
    let cmd_argv = parse_argv(s);
    let mut ret = Vec::new();
    for arg in cmd_argv {
        let mut iter = arg.chars().peekable();
        let mut s = String::new();
        while let Some(c) = iter.next() {
            if c == '\\' && iter.peek() == Some(&'"') {
                s.push('\\'); s.push('"');
                iter.next();
                continue;
            }
            if c == '"' { continue; }
            s.push(c);
        }
        ret.push(s);
    }
    ret
}

fn parse_seps<F>(s: &str, func: F) -> String
    where F: Fn(char) -> bool {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut ret = String::new();
    let mut iter = s.chars().peekable();
    while let Some(c) = iter.next() {
        if c == '\\' && iter.peek() == Some(&'"') {
            ret.push('\\'); ret.push('"');
            iter.next();
            continue;
        }
        if c == '\\' && iter.peek() == Some(&'\'') {
            ret.push('\\'); ret.push('\'');
            iter.next();
            continue;
        }

        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
        } else if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
        }

        if (func(c)) && !in_single_quote && !in_double_quote {
            ret.push('\n');
        } else {
            ret.push(c);
        }
    }
    ret
}

pub(crate) fn parse_argv(s: String) -> Vec<String> {
    s.split("\n")
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_owned())
        .collect::<Vec<String>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_cmds() {
        let cmds_str = "ls -a; echo \"hello\";";
        let cmds_with_lines = parse_cmds(cmds_str);
        let expected = "ls -a\n echo \"hello\"\n";
        eprintln!("cmds parsed:\n#{}#", cmds_with_lines);
        eprintln!("expected:\n#{}#", expected);
        assert!(cmds_with_lines == expected);
    }

    #[test]
    fn test_parse_cmd_args() {
        let cmd_str = "mkdir   /tmp/\"my folder\"";
        let cmd_args = parse_cmd_args(cmd_str);
        eprintln!("cmd_args: {:#?}", cmd_args);

        let cmd_argv = parse_cmd_argv(cmd_args);
        eprintln!("cmd_argv: {:#?}", cmd_argv);

        assert!(cmd_argv[0] == "mkdir");
        assert!(cmd_argv[1] == "/tmp/my folder");
    }
}
