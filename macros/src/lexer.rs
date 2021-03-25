use crate::parser::{ParseArg, Parser};
use proc_macro2::{Delimiter, Ident, Literal, Span, TokenStream, TokenTree};
use proc_macro_error::abort;
use quote::quote;
use std::iter::Peekable;

// Parse string literal to tokenstream, used by most of the macros
//
// - support ${var} or $var for interpolation
//   - to escape '$' itself, use "$$"
// - support normal rust character escapes:
//   https://doc.rust-lang.org/reference/tokens.html#ascii-escapes
pub fn parse_str_lit(lit: &Literal) -> TokenStream {
    let s = lit.to_string();
    if !s.starts_with('\"') {
        return quote!(#lit);
    }
    let mut iter = s[1..s.len() - 1] // To trim outside ""
        .chars()
        .peekable();
    let mut output = quote!("");
    let mut last_part = String::new();
    fn extend_last_part(last_part: &mut String, ch: char) {
        if last_part.is_empty() {
            last_part.push('"'); // start new string literal
        }
        last_part.push(ch);
    }
    fn parse_last_part(last_part: &mut String, output: &mut TokenStream) {
        if !last_part.is_empty() {
            last_part.push('"'); // seal it
            let l = syn::parse_str::<Literal>(&last_part).unwrap();
            output.extend(quote!(+ #l));
            last_part.clear();
        }
    }

    while let Some(ch) = iter.next() {
        if ch == '$' {
            if iter.peek() == Some(&'$') {
                iter.next();
                extend_last_part(&mut last_part, '$');
                continue;
            }

            parse_last_part(&mut last_part, &mut output);
            let mut with_brace = false;
            if iter.peek() == Some(&'{') {
                with_brace = true;
                iter.next();
            }
            let mut var = String::new();
            while let Some(&c) = iter.peek() {
                if !c.is_ascii_alphanumeric() && c != '_' {
                    break;
                }
                if var.is_empty() && c.is_ascii_digit() {
                    break;
                }
                var.push(c);
                iter.next();
            }
            if with_brace {
                if iter.peek() != Some(&'}') {
                    abort!(lit.span(), "bad substitution");
                } else {
                    iter.next();
                }
            }
            if !var.is_empty() {
                let var = syn::parse_str::<Ident>(&var).unwrap();
                output.extend(quote!(+ &#var.to_string()));
            } else {
                output.extend(quote!(+ &'$'.to_string()));
            }
        } else {
            extend_last_part(&mut last_part, ch);
        }
    }
    parse_last_part(&mut last_part, &mut output);
    output
}

enum SepToken {
    Space,
    SemiColon,
    Or,
    Pipe,
}

enum RedirectFd {
    Stdin,
    Stdout { append: bool },
    Stderr { append: bool },
    StdoutErr { append: bool },
}

#[derive(Default)]
pub struct Lexer {
    args: Vec<ParseArg>,
    last_arg_str: TokenStream,
    last_redirect: Option<(RedirectFd, Span)>,
}
impl Lexer {
    pub fn scan(mut self, input: TokenStream) -> Parser {
        let mut iter = TokenStreamPeekable {
            peekable: input.into_iter().peekable(),
            span: None,
        };
        let mut allow_or_token = true;
        while let Some(item) = iter.next() {
            match item {
                TokenTree::Group(_) => {
                    abort!(iter.span(), "grouping is only allowed for variables");
                }
                TokenTree::Literal(lit) => {
                    self.scan_literal(lit, &mut iter);
                }
                TokenTree::Ident(ident) => {
                    let s = ident.to_string();
                    self.extend_last_arg(quote!(&#s));
                }
                TokenTree::Punct(punct) => {
                    let ch = punct.as_char();
                    if ch == ';' {
                        self.add_arg_with_token(SepToken::SemiColon);
                        allow_or_token = true;
                    } else if ch == '|' {
                        self.scan_pipe_or(&mut allow_or_token, &mut iter);
                    } else if ch == '<' {
                        self.set_redirect(iter.span(), RedirectFd::Stdin);
                    } else if ch == '>' {
                        self.scan_redirect_out(&mut iter, 1);
                    } else if ch == '&' {
                        self.scan_ampersand(&mut iter);
                    } else if ch == '$' {
                        self.scan_dollar(&mut iter);
                    } else {
                        self.extend_last_arg(quote!(&#ch.to_string()));
                    }
                }
            }

            if iter.peek_no_gap().is_none() && !self.last_arg_str.is_empty() {
                self.add_arg_with_token(SepToken::Space);
            }
        }
        self.add_arg_with_token(SepToken::Space);
        Parser::from_args(self.args)
    }

    fn add_arg_with_token(&mut self, token: SepToken) {
        if let Some((redirect, span)) = self.last_redirect.take() {
            if self.last_arg_str.is_empty() {
                abort!(span, "wrong redirection format: missing target");
            }

            let mut stdouterr = false;
            let (fd, append) = match redirect {
                RedirectFd::Stdin => (0, false),
                RedirectFd::Stdout { append } => (1, append),
                RedirectFd::Stderr { append } => (2, append),
                RedirectFd::StdoutErr { append } => {
                    stdouterr = true;
                    (2, append)
                }
            };
            let last_arg_str = &self.last_arg_str;
            self.args.push(ParseArg::ParseRedirectFile(
                fd,
                quote!(#last_arg_str),
                append,
            ));
            if stdouterr {
                self.args
                    .push(ParseArg::ParseRedirectFile(1, quote!(#last_arg_str), true));
            }
        } else if !self.last_arg_str.is_empty() {
            let last_arg_str = &self.last_arg_str;
            let last_arg = ParseArg::ParseArgStr(quote!(#last_arg_str));
            self.args.push(last_arg);
        }
        match token {
            SepToken::Space => {}
            SepToken::SemiColon => self.args.push(ParseArg::ParseSemicolon),
            SepToken::Or => self.args.push(ParseArg::ParseOr),
            SepToken::Pipe => self.args.push(ParseArg::ParsePipe),
        }
        self.last_arg_str = TokenStream::new();
    }

    fn extend_last_arg(&mut self, stream: TokenStream) {
        if self.last_arg_str.is_empty() {
            self.last_arg_str = quote!(String::new());
        }
        self.last_arg_str.extend(quote!(+ #stream));
    }

    fn set_redirect(&mut self, span: Span, fd: RedirectFd) {
        if self.last_redirect.is_some() {
            abort!(span, "wrong double redirection format");
        }
        self.last_redirect = Some((fd, span));
    }

    fn scan_literal(
        &mut self,
        lit: Literal,
        iter: &mut TokenStreamPeekable<impl Iterator<Item = TokenTree>>,
    ) {
        let s = lit.to_string();
        if s.starts_with('\"') || s.starts_with('r') {
            // string literal
            self.extend_last_arg(parse_str_lit(&lit));
        } else {
            let mut is_redirect = false;
            if s == "1" || s == "2" {
                if let Some(TokenTree::Punct(ref p)) = iter.peek_no_gap() {
                    if p.as_char() == '>' {
                        iter.next();
                        self.scan_redirect_out(iter, s.parse().unwrap());
                        is_redirect = true;
                    }
                }
            }
            if !is_redirect {
                self.extend_last_arg(quote!(&#lit.to_string()));
            }
        }
    }

    fn scan_pipe_or(
        &mut self,
        allow_or_token: &mut bool,
        iter: &mut TokenStreamPeekable<impl Iterator<Item = TokenTree>>,
    ) {
        let mut is_pipe = true;
        if let Some(TokenTree::Punct(p)) = iter.peek_no_gap() {
            if p.as_char() == '|' {
                is_pipe = false;
                iter.next();
            }
        }

        // expect new command
        match iter.peek() {
            Some(TokenTree::Punct(np)) => {
                if np.as_char() == '|' {
                    abort!(np.span(), "expect new command after '|'");
                }
            }
            None => {
                abort!(iter.span(), "expect new command after '|'");
            }
            _ => {}
        }
        self.add_arg_with_token(if is_pipe {
            SepToken::Pipe
        } else {
            if !*allow_or_token {
                abort!(iter.span(), "only one || is allowed");
            }
            *allow_or_token = false;
            SepToken::Or
        });
    }

    fn scan_redirect_out(
        &mut self,
        iter: &mut TokenStreamPeekable<impl Iterator<Item = TokenTree>>,
        fd: i32,
    ) {
        let append = Self::check_append(iter);
        self.set_redirect(
            iter.span(),
            if fd == 1 {
                RedirectFd::Stdout { append }
            } else {
                RedirectFd::Stderr { append }
            },
        );
        if let Some(TokenTree::Punct(p)) = iter.peek_no_gap() {
            if p.as_char() == '&' {
                if append {
                    abort!(p.span(), "raw fd not allowed for append redirection");
                }
                iter.next();
                if let Some(TokenTree::Literal(lit)) = iter.peek_no_gap() {
                    let s = lit.to_string();
                    if s.starts_with('\"') || s.starts_with('r') {
                        abort!(lit.span(), "invalid literal string after &");
                    }
                    if &s == "1" {
                        self.args.push(ParseArg::ParseRedirectFd(fd, 1));
                    } else if &s == "2" {
                        self.args.push(ParseArg::ParseRedirectFd(fd, 2));
                    } else {
                        abort!(lit.span(), "Only &1 or &2 is supported");
                    }
                    self.last_redirect = None;
                    iter.next();
                } else {
                    abort!(iter.span(), "expect &1 or &2");
                }
            }
        }
    }

    fn scan_ampersand(&mut self, iter: &mut TokenStreamPeekable<impl Iterator<Item = TokenTree>>) {
        if let Some(TokenTree::Punct(p)) = iter.peek_no_gap() {
            let span = p.span();
            if p.as_char() == '>' {
                iter.next();
                self.set_redirect(
                    span,
                    RedirectFd::StdoutErr {
                        append: Self::check_append(iter),
                    },
                );
            } else {
                abort!(span, "invalid punctuation");
            }
        } else {
            if self.last_redirect.is_some() {
                abort!(
                    iter.span(),
                    "wrong redirection format: no spacing permitted before '&'"
                );
            } else {
                abort!(iter.span(), "invalid token after '&'");
            }
        }
    }

    fn scan_dollar(&mut self, iter: &mut TokenStreamPeekable<impl Iterator<Item = TokenTree>>) {
        if let Some(TokenTree::Ident(var)) = iter.peek_no_gap() {
            self.extend_last_arg(quote!(&#var.to_string()));
        } else if let Some(TokenTree::Group(g)) = iter.peek_no_gap() {
            if g.delimiter() != Delimiter::Brace && g.delimiter() != Delimiter::Bracket {
                abort!(
                    g.span(),
                    "invalid grouping: found {:?}, only \"brace/bracket\" is allowed",
                    format!("{:?}", g.delimiter()).to_lowercase()
                );
            }
            let mut found_var = false;
            for tt in g.stream() {
                let span = tt.span();
                if let TokenTree::Ident(ref var) = tt {
                    if found_var {
                        abort!(span, "more than one variable in grouping");
                    }
                    if g.delimiter() == Delimiter::Brace {
                        self.extend_last_arg(quote!(&#var.to_string()));
                    } else {
                        if !self.last_arg_str.is_empty() {
                            abort!(span, "vector variable can only be used alone");
                        }
                        self.args.push(ParseArg::ParseArgVec(quote!(#var)));
                    }
                    found_var = true;
                } else {
                    abort!(span, "invalid grouping: extra tokens");
                }
            }
        } else {
            abort!(iter.span(), "invalid token after $");
        }
        iter.next();
    }

    fn check_append(iter: &mut TokenStreamPeekable<impl Iterator<Item = TokenTree>>) -> bool {
        let mut append = false;
        if let Some(TokenTree::Punct(p)) = iter.peek_no_gap() {
            if p.as_char() == '>' {
                append = true;
                iter.next();
            }
        }
        append
    }
}

struct TokenStreamPeekable<I: Iterator<Item = TokenTree>> {
    peekable: Peekable<I>,
    span: Option<Span>,
}

impl<I: Iterator<Item = TokenTree>> Iterator for TokenStreamPeekable<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<TokenTree> {
        if let Some(tt) = self.peekable.next() {
            self.span = Some(tt.span());
            Some(tt)
        } else {
            self.span = None;
            None
        }
    }
}

impl<I: Iterator<Item = TokenTree>> TokenStreamPeekable<I> {
    fn peek(&mut self) -> Option<&TokenTree> {
        self.peekable.peek()
    }

    // peek next token which has no spaces between
    fn peek_no_gap(&mut self) -> Option<&TokenTree> {
        match self.peekable.peek() {
            None => None,
            Some(item) => {
                let (_, cur_end) = Self::span_location(&self.span.unwrap());
                let (new_start, _) = Self::span_location(&item.span());
                if new_start > cur_end {
                    None
                } else {
                    Some(item)
                }
            }
        }
    }

    fn span(&self) -> Span {
        self.span.unwrap()
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
}
