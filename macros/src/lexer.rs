use crate::parser::{ParseArg, Parser};
use proc_macro2::{Delimiter, Ident, Span, TokenStream, TokenTree};
use quote::{quote, ToTokens};

pub fn parse_cmds_from_stream(input: TokenStream) -> TokenStream {
    Lexer::from(input).scan().parse().into()
}

enum SepToken {
    Space,
    SemiColon,
    Or,
    Pipe,
}

#[derive(PartialEq)]
enum MarkerToken {
    Pipe,
    DollarSign,
    Ampersand,
    Fd(i32),
    None,
}

#[derive(PartialEq, Clone)]
enum RedirectFd {
    Stdin,
    Stdout,
    Stderr,
    StdoutErr,
}
impl RedirectFd {
    fn id(&self) -> i32 {
        match self {
            Self::Stdin => 0,
            Self::Stdout => 1,
            Self::Stderr | Self::StdoutErr => 2,
        }
    }
}

pub struct Lexer {
    input: TokenStream,
    args: Vec<ParseArg>,

    last_token: MarkerToken,
    last_arg_str: TokenStream,
    last_redirect: Option<(RedirectFd, bool)>,
}

impl Lexer {
    fn from(input: TokenStream) -> Self {
        Self {
            input,
            args: vec![],
            last_token: MarkerToken::None,
            last_arg_str: TokenStream::new(),
            last_redirect: None,
        }
    }

    fn last_is_pipe(&self) -> bool {
        self.last_token == MarkerToken::Pipe
    }

    fn last_is_dollar_sign(&self) -> bool {
        self.last_token == MarkerToken::DollarSign
    }

    fn set_last_token(&mut self, value: MarkerToken) {
        self.last_token = value;
    }

    fn reset_last_token(&mut self) {
        self.last_arg_str = TokenStream::new();
        self.last_token = MarkerToken::None;
    }

    fn set_redirect(&mut self, fd: RedirectFd) {
        if let Some((_, append)) = self.last_redirect {
            if append {
                panic!("wrong redirect format: more than append");
            }
            if fd == RedirectFd::Stdin {
                panic!("wrong input redirect format");
            }
            self.last_redirect = Some((fd, true));
        } else {
            if self.last_token == MarkerToken::Ampersand {
                self.last_redirect = Some((RedirectFd::StdoutErr, false));
                self.reset_last_token();
            } else {
                self.last_redirect = Some((fd, false));
            }
        }
    }

    fn last_arg_str_empty(&self) -> bool {
        self.last_arg_str.is_empty()
    }

    fn add_arg_with_token(&mut self, token: SepToken) {
        if let Some((fd, append)) = self.last_redirect.clone() {
            let last_arg_str = self.last_arg_str.clone();
            let fd_id = fd.id();
            self.args.push(ParseArg::ParseRedirectFile(
                fd_id,
                quote!(#last_arg_str),
                append,
            ));
            if fd == RedirectFd::StdoutErr {
                self.args
                    .push(ParseArg::ParseRedirectFile(1, quote!(#last_arg_str), true));
            }
            self.last_redirect = None;
        } else {
            if !self.last_arg_str_empty() {
                let last_arg_str = self.last_arg_str.clone();
                let last_arg = ParseArg::ParseArgStr(quote!(#last_arg_str));
                self.args.push(last_arg);
            }
        }
        match token {
            SepToken::Space => {}
            SepToken::SemiColon => self.args.push(ParseArg::ParseSemicolon),
            SepToken::Or => {
                self.args.pop();
                self.args.push(ParseArg::ParseOr);
            }
            SepToken::Pipe => self.args.push(ParseArg::ParsePipe),
        }
        self.reset_last_token();
    }

    fn add_fd_redirect_arg(&mut self, old_fd: i32, new_fd_stream: TokenStream, append: bool) {
        self.args.push(ParseArg::ParseRedirectFd(
            old_fd,
            quote!(#new_fd_stream),
            append,
        ));
        self.last_redirect = None;
        self.reset_last_token();
    }

    fn extend_last_arg(&mut self, stream: TokenStream) {
        if self.last_arg_str_empty() {
            self.last_arg_str = quote!(String::new());
        }
        self.last_arg_str.extend(quote!(+ #stream));
        self.last_token = MarkerToken::None;
    }

    fn scan(mut self) -> Parser {
        let mut end = 0;
        for t in self.input.clone() {
            let (_start, _end) = Self::span_location(&t.span());
            if end != 0 && end < _start {
                // new argument with spacing
                if !self.last_arg_str_empty() {
                    self.add_arg_with_token(SepToken::Space);
                }
            }
            end = _end;

            let src = t.to_string();
            if self.last_is_dollar_sign() {
                if let TokenTree::Group(g) = t.clone() {
                    if g.delimiter() != Delimiter::Brace && g.delimiter() != Delimiter::Bracket {
                        panic!(
                            "invalid grouping: found {:?}, only Brace/Bracket is allowed",
                            g.delimiter()
                        );
                    }
                    let mut found_var = false;
                    for tt in g.stream() {
                        if let TokenTree::Ident(var) = tt {
                            if found_var {
                                panic!("more than one variable in grouping");
                            }
                            if g.delimiter() == Delimiter::Brace {
                                self.extend_last_arg(quote!(&#var.to_string()));
                            } else {
                                if !self.last_arg_str_empty() {
                                    panic!("vector variable can only be used alone");
                                }
                                self.args.push(ParseArg::ParseArgVec(quote!(#var)));
                                self.reset_last_token();
                            }
                            found_var = true;
                        } else {
                            panic!("invalid grouping: extra tokens");
                        }
                    }
                    continue;
                } else if let TokenTree::Ident(var) = t {
                    self.extend_last_arg(quote!(&#var.to_string()));
                    continue;
                }
            }

            if let TokenTree::Group(_) = t {
                panic!("grouping is only allowed for variable");
            } else if let TokenTree::Literal(lit) = t {
                let s = lit.to_string();
                if s.starts_with("\"") || s.starts_with("r") {
                    if s.starts_with("\"") {
                        self.parse_vars(&s[1..s.len() - 1]);
                    } else {
                        self.extend_last_arg(quote!(#lit));
                    }
                } else {
                    if self.last_token == MarkerToken::Ampersand {
                        if &s != "1" && &s != "2" {
                            panic!("only &1 or &2 is allowed");
                        }
                        if let Some((fd, append)) = self.last_redirect.clone() {
                            self.add_fd_redirect_arg(fd.id(), lit.to_token_stream(), append);
                        } else {
                            panic!("& is only allowed for redirect");
                        }
                        continue;
                    }
                    self.extend_last_arg(quote!(&#lit.to_string()));
                    if &s == "1" {
                        self.last_token = MarkerToken::Fd(1);
                    } else if &s == "2" {
                        self.last_token = MarkerToken::Fd(2);
                    }
                }
            } else {
                if let TokenTree::Punct(p) = t {
                    let ch = p.as_char();
                    if ch == '$' {
                        self.set_last_token(MarkerToken::DollarSign);
                        continue;
                    } else if ch == ';' {
                        self.add_arg_with_token(SepToken::SemiColon);
                        continue;
                    } else if ch == '|' {
                        if self.last_is_pipe() {
                            self.add_arg_with_token(SepToken::Or);
                            self.set_last_token(MarkerToken::None);
                        } else {
                            self.add_arg_with_token(SepToken::Pipe);
                            self.set_last_token(MarkerToken::Pipe);
                        }
                        continue;
                    } else if ch == '>' {
                        if let MarkerToken::Fd(fd) = self.last_token {
                            self.set_redirect(if fd == 2 {
                                RedirectFd::Stderr
                            } else {
                                RedirectFd::Stdout
                            });
                            self.reset_last_token();
                        } else {
                            self.set_redirect(RedirectFd::Stdout);
                        }
                        continue;
                    } else if ch == '<' {
                        self.set_redirect(RedirectFd::Stdin);
                        continue;
                    } else if ch == '&' {
                        self.set_last_token(MarkerToken::Ampersand);
                        continue;
                    }
                }

                self.extend_last_arg(quote!(&#src.to_string()));
            }
        }
        self.add_arg_with_token(SepToken::Space);
        Parser::from_args(self.args)
    }

    // helper function to get (start, end) of Span
    fn span_location(span: &Span) -> (usize, usize) {
        let s = format!("{:?}", span);
        let mut start = 0;
        let mut end = 0;
        let mut parse_second = false;
        for c in s.chars().skip(6) {
            if c == '.' {
                parse_second = true;
            } else if c.is_ascii_digit() {
                let digit = c.to_digit(10).unwrap() as usize;
                if !parse_second {
                    start = start * 10 + digit;
                } else {
                    end = end * 10 + digit;
                }
            }
        }
        (start, end)
    }

    fn parse_vars(&mut self, src: &str) {
        let input: Vec<char> = src.chars().collect();
        let len = input.len();

        let mut i = 0;
        while i < len {
            if input[i] == '$' && (i == 0 || input[i - 1] != '\\') {
                i += 1;
                let with_brace = i < len && input[i] == '{';
                let mut var = String::new();
                if with_brace {
                    i += 1;
                }
                while i < len && (input[i].is_ascii_alphanumeric() || (input[i] == '_')) {
                    if var.is_empty() && input[i].is_ascii_digit() {
                        break;
                    }
                    var.push(input[i]);
                    i += 1;
                }
                if with_brace {
                    if i == len || input[i] != '}' {
                        panic!("bad substitution");
                    }
                } else {
                    i -= 1; // back off 1 char
                }
                if !var.is_empty() {
                    let var = syn::parse_str::<Ident>(&var).unwrap();
                    self.extend_last_arg(quote!(&#var.to_string()));
                } else {
                    self.extend_last_arg(quote!(&'$'.to_string()));
                }
            } else {
                let ch = input[i];
                self.extend_last_arg(quote!(&#ch.to_string()));
            }
            i += 1;
        }
    }
}
