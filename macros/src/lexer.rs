use crate::parser::{ParseArg, Parser};
use proc_macro2::{Delimiter, Ident, Literal, Span, TokenStream, TokenTree};
use proc_macro_error::abort;
use quote::quote;
use std::iter::Peekable;

enum SepToken {
    Space,
    SemiColon,
    Or,
    Pipe,
}

#[derive(PartialEq, Clone, Debug)]
enum RedirectFd {
    Stdin,
    Stdout(bool),    // append?
    Stderr(bool),    // append?
    StdoutErr(bool), // append?
}
impl RedirectFd {
    fn get_id(&self) -> i32 {
        match self {
            Self::Stdin => 0,
            Self::Stdout(_) => 1,
            Self::Stderr(_) | Self::StdoutErr(_) => 2,
        }
    }

    fn get_append(&self) -> bool {
        self == &Self::Stdout(true) || self == &Self::Stderr(true) || self == &Self::StdoutErr(true)
    }
}

#[derive(Default)]
pub struct Lexer {
    args: Vec<ParseArg>,
    last_arg_str: TokenStream,
    last_redirect: Option<RedirectFd>,
}

impl Lexer {
    pub fn scan(mut self, input: TokenStream) -> Parser {
        let mut iter = input.into_iter().peekable();
        let mut allow_or_token = true;
        while let Some(item) = iter.next() {
            let span = item.span();
            match item {
                TokenTree::Group(_) => {
                    abort!(item.span(), "grouping is only allowed for variables");
                }
                TokenTree::Literal(lit) => {
                    self.scan_literal(lit, span, &mut iter);
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
                        self.scan_pipe_or(&mut allow_or_token, span, &mut iter);
                    } else if ch == '<' {
                        self.set_redirect(span, RedirectFd::Stdin);
                    } else if ch == '>' {
                        self.scan_redirect_out(span, &mut iter, 1);
                    } else if ch == '&' {
                        self.scan_ampersand(span, &mut iter);
                    } else if ch == '$' {
                        self.scan_dollar(span, &mut iter);
                    } else {
                        self.extend_last_arg(quote!(&#ch.to_string()));
                    }
                }
            }

            if Self::peek(span, &mut iter).is_none() && !self.last_arg_str.is_empty() {
                self.add_arg_with_token(SepToken::Space);
            }
        }
        self.add_arg_with_token(SepToken::Space);
        Parser::from_args(self.args)
    }

    fn add_arg_with_token(&mut self, token: SepToken) {
        if let Some(fd) = self.last_redirect.clone() {
            let last_arg_str = self.last_arg_str.clone();
            let fd_id = fd.get_id();
            let fd_append = fd.get_append();
            self.args.push(ParseArg::ParseRedirectFile(
                fd_id,
                quote!(#last_arg_str),
                fd_append,
            ));
            if let RedirectFd::StdoutErr(_) = fd {
                self.args
                    .push(ParseArg::ParseRedirectFile(1, quote!(#last_arg_str), true));
            }
            self.last_redirect = None;
        } else if !self.last_arg_str.is_empty() {
            let last_arg_str = self.last_arg_str.clone();
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
            abort!(span, "wrong redirection format");
        }
        self.last_redirect = Some(fd);
    }

    fn add_fd_redirect_arg(&mut self, span: Span, new_fd: i32) {
        if let Some(fd) = self.last_redirect.clone() {
            if !fd.get_append() {
                self.args
                    .push(ParseArg::ParseRedirectFd(fd.get_id(), new_fd));
                self.last_redirect = None;
                return;
            }
        }
        abort!(span, "invalid token");
    }

    fn scan_literal(
        &mut self,
        lit: Literal,
        span: Span,
        iter: &mut Peekable<impl Iterator<Item = TokenTree>>,
    ) {
        let s = lit.to_string();
        if s.starts_with('\"') || s.starts_with('r') {
            // string literal
            self.extend_last_arg(Self::parse_str_lit(&lit));
        } else {
            let mut is_redirect = false;
            if s == "1" || s == "2" {
                if let Some(TokenTree::Punct(ref p)) = Self::peek(span, iter) {
                    let span = p.span();
                    if p.as_char() == '>' {
                        iter.next();
                        self.scan_redirect_out(span, iter, s.parse().unwrap());
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
        span: Span,
        iter: &mut Peekable<impl Iterator<Item = TokenTree>>,
    ) {
        let mut is_pipe = true;
        if let Some(TokenTree::Punct(p)) = Self::peek(span, iter) {
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
                abort!(span, "expect new command after '|'");
            }
            _ => {}
        }
        self.add_arg_with_token(if is_pipe {
            SepToken::Pipe
        } else {
            if !*allow_or_token {
                abort!(span, "only one || is allowed");
            }
            *allow_or_token = false;
            SepToken::Or
        });
    }

    fn scan_redirect_out(&mut self, span: Span, iter: &mut Peekable<impl Iterator<Item = TokenTree>>, fd: i32) {
        self.set_redirect(span, if fd == 1 {
            RedirectFd::Stdout(Self::check_append(span, iter))
        } else {
            RedirectFd::Stderr(Self::check_append(span, iter))
        });
        if let Some(TokenTree::Punct(p)) = Self::peek(span, iter) {
            let span = p.span();
            if p.as_char() == '&' {
                iter.next();
                if let Some(TokenTree::Literal(lit)) = Self::peek(span, iter) {
                    let s = lit.to_string();
                    if s.starts_with('\"') || s.starts_with('r') {
                        abort!(lit.span(), "invalid literal string after &");
                    }
                    if &s == "1" {
                        self.add_fd_redirect_arg(span, 1);
                    } else if &s == "2" {
                        self.add_fd_redirect_arg(span, 2);
                    } else {
                        abort!(lit.span(), "Only &1 or &2 is supported");
                    }
                    iter.next();
                } else {
                    abort!(span, "expect &1 or &2");
                }
            }
        }
    }

    fn scan_ampersand(&mut self, span: Span, iter: &mut Peekable<impl Iterator<Item = TokenTree>>) {
        if let Some(TokenTree::Punct(p)) = Self::peek(span, iter) {
            if p.as_char() == '>' {
                iter.next();
                self.set_redirect(span, RedirectFd::StdoutErr(Self::check_append(span, iter)));
            } else {
                abort!(p.span(), "invalid punctuation");
            }
        } else {
            abort!(span, "invalid token after '&'");
        }
    }

    fn scan_dollar(&mut self, span: Span, iter: &mut Peekable<impl Iterator<Item = TokenTree>>) {
        if let Some(TokenTree::Ident(var)) = Self::peek(span, iter) {
            self.extend_last_arg(quote!(&#var.to_string()));
        } else if let Some(TokenTree::Group(g)) = Self::peek(span, iter) {
            if g.delimiter() != Delimiter::Brace && g.delimiter() != Delimiter::Bracket {
                abort!(
                    g,
                    "invalid grouping: found {:?}, only \"brace/bracket\" is allowed",
                    format!("{:?}", g.delimiter()).to_lowercase()
                );
            }
            let mut found_var = false;
            for tt in g.stream() {
                if let TokenTree::Ident(ref var) = tt {
                    if found_var {
                        abort!(tt, "more than one variable in grouping");
                    }
                    if g.delimiter() == Delimiter::Brace {
                        self.extend_last_arg(quote!(&#var.to_string()));
                    } else {
                        if !self.last_arg_str.is_empty() {
                            abort!(tt, "vector variable can only be used alone");
                        }
                        self.args.push(ParseArg::ParseArgVec(quote!(#var)));
                    }
                    found_var = true;
                } else {
                    abort!(tt, "invalid grouping: extra tokens");
                }
            }
        } else {
            abort!(span, "invalid token after $");
        }
        iter.next();
    }

    fn check_append(cur: Span, iter: &mut Peekable<impl Iterator<Item = TokenTree>>) -> bool {
        let mut append = false;
        if let Some(TokenTree::Punct(p)) = Self::peek(cur, iter) {
            if p.as_char() == '>' {
                append = true;
                iter.next();
            }
        }
        append
    }

    // peek next token which has no spaces between
    fn peek(cur: Span, iter: &mut Peekable<impl Iterator<Item = TokenTree>>) -> Option<&TokenTree> {
        match iter.peek() {
            None => None,
            Some(item) => {
                let (_, cur_end) = Self::span_location(&cur);
                let (new_start, _) = Self::span_location(&item.span());
                if new_start > cur_end {
                    None
                } else {
                    Some(item)
                }
            }
        }
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
                if let Some(&pc) = iter.peek() {
                    if pc == '$' {
                        extend_last_part(&mut last_part, pc);
                        iter.next();
                        continue;
                    }
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
}
