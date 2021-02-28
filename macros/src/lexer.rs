use proc_macro2::{Delimiter, Ident, Span, TokenStream, TokenTree, Group};
use quote::{quote, ToTokens};

pub fn parse_cmds_from_stream(input: TokenStream) -> TokenStream {
    let args = Lexer::from(input).scan();
    let mut ret = quote! ( ::cmd_lib::Parser::default() );

    for arg in args {
        ret.extend(quote!(.arg));
        ret.extend(Group::new(Delimiter::Parenthesis, arg).to_token_stream());
    }
    ret.extend(quote!(.parse()));
    ret
}

enum SepToken {
    Space,
    SemiColon,
    Or,
    Pipe,
}

#[derive(Default)]
pub struct Lexer {
    input: TokenStream,
    args: Vec<TokenStream>,

    last_is_dollar_sign: bool,
    last_is_pipe: bool,
    last_arg_str: TokenStream,
}

impl Lexer {
    fn from(input: TokenStream) -> Self {
        Self {
            input,
            args: vec![],
            last_is_dollar_sign: false,
            last_is_pipe: false,
            last_arg_str: TokenStream::new(),
        }
    }

    fn reset(&mut self) {
        self.last_is_dollar_sign = false;
        self.last_is_pipe = false;
        self.last_arg_str = TokenStream::new();
    }

    fn set_last_dollar_sign(&mut self, value: bool) {
        self.last_is_dollar_sign = value;
        self.last_is_pipe = false;
    }

    fn set_last_pipe(&mut self, value: bool) {
        self.last_is_pipe = value;
        self.last_is_dollar_sign = false;
    }

    fn last_arg_str_empty(&self) -> bool {
        self.last_arg_str.is_empty()
    }

    fn add_arg_with_token(&mut self, token: SepToken) {
        if !self.last_arg_str_empty() {
            let mut last_arg = quote!(::cmd_lib::ParseArg::ParseArgStr);
            last_arg.extend(Group::new(Delimiter::Parenthesis, self.last_arg_str.clone()).to_token_stream());
            self.args.push(last_arg);
        }
        match token {
            SepToken::Space => {},
            SepToken::SemiColon => self.args.push(quote!(::cmd_lib::ParseArg::ParseSemicolon)),
            SepToken::Or => {
                self.args.pop();
                self.args.push(quote!(::cmd_lib::ParseArg::ParseOr));
            },
            SepToken::Pipe => self.args.push(quote!(::cmd_lib::ParseArg::ParsePipe)),
        }
        self.reset();
    }

    fn extend_last_arg(&mut self, stream: TokenStream) {
        if self.last_arg_str_empty() {
            self.last_arg_str = quote!(String::new());
        }
        self.last_arg_str.extend(quote!(+));
        self.last_arg_str.extend(stream);
        self.last_is_dollar_sign = false;
        self.last_is_pipe = false;
    }

    fn scan(mut self) -> Vec<TokenStream> {
        let mut end = 0;
        for t in self.input.clone() {
            let (_start, _end) = Self::span_location(&t.span());
            if end != 0 && end < _start { // new argument with spacing
                self.add_arg_with_token(SepToken::Space);
            }
            end = _end;

            let src = t.to_string();
            if self.last_is_dollar_sign {
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
                                self.args.push(quote! (
                                    ::cmd_lib::ParseArg::ParseArgVec(
                                        #var.iter().map(|s| s.to_string()).collect::<Vec<String>>()))
                                );
                                self.reset();
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
                        self.parse_vars(&s[1..s.len()-1]);
                    } else {
                        self.extend_last_arg(quote!(#lit));
                    }
                } else {
                    self.extend_last_arg(quote!(&#lit.to_string()));
                }
            } else {
                if let TokenTree::Punct(p) = t {
                    let ch = p.as_char();
                    if ch == '$' {
                        self.set_last_dollar_sign(true);
                        continue;
                    } else if ch == ';' {
                        self.add_arg_with_token(SepToken::SemiColon);
                        continue;
                    } else if ch == '|' {
                        if self.last_is_pipe {
                            self.add_arg_with_token(SepToken::Or);
                        } else {
                            self.add_arg_with_token(SepToken::Pipe);
                        }
                        self.set_last_pipe(!self.last_is_pipe);
                        continue;
                    }
                }

                self.extend_last_arg(quote!(&#src.to_string()));
            }
        }
        self.add_arg_with_token(SepToken::Space);
        self.args
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
