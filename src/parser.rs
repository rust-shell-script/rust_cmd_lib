pub fn parse_args(s: &str) -> String {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    s.chars()
        .map(|c| {
            if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
                c
            } else if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
                c
            } else if !in_single_quote && !in_double_quote && char::is_whitespace(c) {
                '\n'
            } else {
                c
            }
        })
        .collect()
}

pub fn parse_cmds(s: &str) -> String {
    parse_seps(s, ';')
}

pub fn parse_pipes(s: &str) -> String {
    parse_seps(s, '|')
}

fn parse_seps(s: &str, sep: char) -> String {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    s.chars()
        .map(|c| {
            if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
            } else if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
            }

            if c == sep && !in_single_quote && !in_double_quote {
                '\n'
            } else {
                c
            }
        })
        .collect()
}

pub fn parse_argv(s: String) -> Vec<String> {
    s.split("\n")
        .map(|s| s.trim_matches(|c| c == ' ' || c == '\"'))
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_owned())
        .collect::<Vec<String>>()
}
