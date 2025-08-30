use crate::parser::{ParseArg, Parser};
use proc_macro2::{token_stream, Delimiter, Ident, Literal, Span, TokenStream, TokenTree};
use proc_macro_error2::abort;
use quote::quote;
use std::iter::Peekable;
use std::str::Chars;

/// Scan string literal to tokenstream, used by most of the macros
///
/// - support $var, ${var} or ${var:fmt} for interpolation, where `fmt` can be any
///   of the standard Rust formatting specifiers (e.g., `?`, `x`, `X`, `o`, `b`, `p`, `e`, `E`).
///   - to escape '$' itself, use "$$"
/// - support normal rust character escapes:
///   https://doc.rust-lang.org/reference/tokens.html#ascii-escapes
pub fn scan_str_lit(lit: &Literal) -> TokenStream {
    let s = lit.to_string();

    // If the literal is not a string (e.g., a number literal), treat it as a direct CmdString.
    if !s.starts_with('\"') {
        return quote!(::cmd_lib::CmdString::from(#lit));
    }

    // Extract the inner string by trimming the surrounding quotes.
    let inner_str = &s[1..s.len() - 1];
    let mut chars = inner_str.chars().peekable();
    let mut output = quote!(::cmd_lib::CmdString::default());
    let mut current_literal_part = String::new();

    // Helper function to append the accumulated literal part to the output TokenStream
    // and clear the current_literal_part.
    let seal_current_literal_part = |output: &mut TokenStream, last_part: &mut String| {
        if !last_part.is_empty() {
            let lit_str = format!("\"{}\"", last_part);
            // It's safe to unwrap parse_str because we are constructing a valid string literal.
            let literal_token = syn::parse_str::<Literal>(&lit_str).unwrap();
            output.extend(quote!(.append(#literal_token)));
            last_part.clear();
        }
    };

    while let Some(ch) = chars.next() {
        if ch == '$' {
            // Handle "$$" for escaping '$'
            if chars.peek() == Some(&'$') {
                chars.next(); // Consume the second '$'
                current_literal_part.push('$');
                continue;
            }

            // Before handling a variable, append any accumulated literal part.
            seal_current_literal_part(&mut output, &mut current_literal_part);

            let mut format_specifier = String::new(); // To store the fmt specifier (e.g., "?", "x", "#x")
            let mut is_braced_interpolation = false;

            // Check for '{' to start a braced interpolation
            if chars.peek() == Some(&'{') {
                is_braced_interpolation = true;
                chars.next(); // Consume '{'
            }

            let var_name = parse_variable_name(&mut chars);

            if is_braced_interpolation {
                // If it's braced, we might have a format specifier or it might just be empty braces.
                if chars.peek() == Some(&':') {
                    chars.next(); // Consume ':'
                                  // Read the format specifier until '}'
                    while let Some(&c) = chars.peek() {
                        if c == '}' {
                            break;
                        }
                        format_specifier.push(c);
                        chars.next(); // Consume the character of the specifier
                    }
                }

                // Expect '}' to close the braced interpolation
                if chars.next() != Some('}') {
                    abort!(lit.span(), "bad substitution: expected '}'");
                }
            }

            if !var_name.is_empty() {
                let var_ident = syn::parse_str::<Ident>(&var_name).unwrap();

                // To correctly handle all format specifiers (like {:02X}), we need to insert the
                // entire format string *as a literal* into the format! macro.
                // The `format_specifier` string itself needs to be embedded.
                let format_macro_call = if format_specifier.is_empty() {
                    quote! {
                        .append(format!("{}", #var_ident))
                    }
                } else {
                    let format_literal_str = format!("{{:{}}}", format_specifier);
                    let format_literal_token = Literal::string(&format_literal_str);
                    quote! {
                        .append(format!(#format_literal_token, #var_ident))
                    }
                };
                output.extend(format_macro_call);
            } else {
                // This covers cases like "${}" or "${:?}" with empty variable name
                output.extend(quote!(.append("$")));
            }
        } else {
            current_literal_part.push(ch);
        }
    }

    // Append any remaining literal part after the loop finishes.
    seal_current_literal_part(&mut output, &mut current_literal_part);
    output
}

/// Parses a variable name from the character iterator.
/// A variable name consists of alphanumeric characters and underscores,
/// and cannot start with a digit.
fn parse_variable_name(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut var = String::new();
    while let Some(&c) = chars.peek() {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            break;
        }
        if var.is_empty() && c.is_ascii_digit() {
            // Variable names cannot start with a digit
            break;
        }
        var.push(c);
        chars.next(); // Consume the character
    }
    var
}

enum SepToken {
    Space,
    SemiColon,
    Pipe,
}

enum RedirectFd {
    Stdin,
    Stdout { append: bool },
    Stderr { append: bool },
    StdoutErr { append: bool },
}

pub struct Lexer {
    iter: TokenStreamPeekable<token_stream::IntoIter>,
    args: Vec<ParseArg>,
    last_arg_str: TokenStream,
    last_redirect: Option<(RedirectFd, Span)>,
    seen_redirect: (bool, bool, bool),
}

impl Lexer {
    pub fn new(input: TokenStream) -> Self {
        Self {
            args: vec![],
            last_arg_str: TokenStream::new(),
            last_redirect: None,
            seen_redirect: (false, false, false),
            iter: TokenStreamPeekable {
                peekable: input.into_iter().peekable(),
                span: Span::call_site(),
            },
        }
    }

    pub fn scan(mut self) -> Parser<impl Iterator<Item = ParseArg>> {
        while let Some(item) = self.iter.next() {
            match item {
                TokenTree::Group(_) => {
                    abort!(self.iter.span(), "grouping is only allowed for variables");
                }
                TokenTree::Literal(lit) => {
                    self.scan_literal(lit);
                }
                TokenTree::Ident(ident) => {
                    let s = ident.to_string();
                    self.extend_last_arg(quote!(#s));
                }
                TokenTree::Punct(punct) => {
                    let ch = punct.as_char();
                    if ch == ';' {
                        self.add_arg_with_token(SepToken::SemiColon, self.iter.span());
                    } else if ch == '|' {
                        self.scan_pipe();
                    } else if ch == '<' {
                        self.set_redirect(self.iter.span(), RedirectFd::Stdin);
                    } else if ch == '>' {
                        self.scan_redirect_out(1);
                    } else if ch == '&' {
                        self.scan_ampersand();
                    } else if ch == '$' {
                        self.scan_dollar();
                    } else {
                        let s = ch.to_string();
                        self.extend_last_arg(quote!(#s));
                    }
                }
            }

            if self.iter.peek_no_gap().is_none() && !self.last_arg_str.is_empty() {
                self.add_arg_with_token(SepToken::Space, self.iter.span());
            }
        }
        self.add_arg_with_token(SepToken::Space, self.iter.span());
        Parser::from(self.args.into_iter().peekable())
    }

    fn add_arg_with_token(&mut self, token: SepToken, token_span: Span) {
        let last_arg_str = &self.last_arg_str;
        if let Some((redirect, span)) = self.last_redirect.take() {
            if last_arg_str.is_empty() {
                abort!(span, "wrong redirection format: missing target");
            }

            let mut stdouterr = false;
            let (fd, append) = match redirect {
                RedirectFd::Stdin => (0, false),
                RedirectFd::Stdout { append } => (1, append),
                RedirectFd::Stderr { append } => (2, append),
                RedirectFd::StdoutErr { append } => {
                    stdouterr = true;
                    (1, append)
                }
            };
            self.args
                .push(ParseArg::RedirectFile(fd, quote!(#last_arg_str), append));
            if stdouterr {
                self.args.push(ParseArg::RedirectFd(2, 1));
            }
        } else if !last_arg_str.is_empty() {
            self.args.push(ParseArg::ArgStr(quote!(#last_arg_str)));
        }
        let mut new_redirect = (false, false, false);
        match token {
            SepToken::Space => new_redirect = self.seen_redirect,
            SepToken::SemiColon => self.args.push(ParseArg::Semicolon),
            SepToken::Pipe => {
                Self::check_set_redirect(&mut self.seen_redirect.1, "stdout", token_span);
                self.args.push(ParseArg::Pipe);
                new_redirect.0 = true;
            }
        }
        self.seen_redirect = new_redirect;
        self.last_arg_str = TokenStream::new();
    }

    fn extend_last_arg(&mut self, stream: TokenStream) {
        if self.last_arg_str.is_empty() {
            self.last_arg_str = quote!(::cmd_lib::CmdString::default());
        }
        self.last_arg_str.extend(quote!(.append(#stream)));
    }

    fn check_set_redirect(redirect: &mut bool, name: &str, span: Span) {
        if *redirect {
            abort!(span, "already set {} redirection", name);
        }
        *redirect = true;
    }

    fn set_redirect(&mut self, span: Span, fd: RedirectFd) {
        if self.last_redirect.is_some() {
            abort!(span, "wrong double redirection format");
        }
        match fd {
            RedirectFd::Stdin => Self::check_set_redirect(&mut self.seen_redirect.0, "stdin", span),
            RedirectFd::Stdout { append: _ } => {
                Self::check_set_redirect(&mut self.seen_redirect.1, "stdout", span)
            }
            RedirectFd::Stderr { append: _ } => {
                Self::check_set_redirect(&mut self.seen_redirect.2, "stderr", span)
            }
            RedirectFd::StdoutErr { append: _ } => {
                Self::check_set_redirect(&mut self.seen_redirect.1, "stdout", span);
                Self::check_set_redirect(&mut self.seen_redirect.2, "stderr", span);
            }
        }
        self.last_redirect = Some((fd, span));
    }

    fn scan_literal(&mut self, lit: Literal) {
        let s = lit.to_string();
        if s.starts_with('\"') || s.starts_with('r') {
            // string literal
            let ss = scan_str_lit(&lit);
            self.extend_last_arg(quote!(#ss.into_os_string()));
        } else {
            let mut is_redirect = false;
            if s == "1" || s == "2" {
                if let Some(TokenTree::Punct(ref p)) = self.iter.peek_no_gap() {
                    if p.as_char() == '>' {
                        self.iter.next();
                        self.scan_redirect_out(if s == "1" { 1 } else { 2 });
                        is_redirect = true;
                    }
                }
            }
            if !is_redirect {
                self.extend_last_arg(quote!(#s));
            }
        }
    }

    fn scan_pipe(&mut self) {
        if let Some(TokenTree::Punct(p)) = self.iter.peek_no_gap() {
            if p.as_char() == '&' {
                if let Some(ref redirect) = self.last_redirect {
                    abort!(redirect.1, "invalid '&': found previous redirect");
                }
                Self::check_set_redirect(&mut self.seen_redirect.2, "stderr", p.span());
                self.args.push(ParseArg::RedirectFd(2, 1));
                self.iter.next();
            }
        }

        // expect new command
        match self.iter.peek() {
            Some(TokenTree::Punct(np)) => {
                if np.as_char() == '|' || np.as_char() == ';' {
                    abort!(np.span(), "expect new command after '|'");
                }
            }
            None => {
                abort!(self.iter.span(), "expect new command after '|'");
            }
            _ => {}
        }
        self.add_arg_with_token(SepToken::Pipe, self.iter.span());
    }

    fn scan_redirect_out(&mut self, fd: i32) {
        let append = self.check_append();
        self.set_redirect(
            self.iter.span(),
            if fd == 1 {
                RedirectFd::Stdout { append }
            } else {
                RedirectFd::Stderr { append }
            },
        );
        if let Some(TokenTree::Punct(p)) = self.iter.peek_no_gap() {
            if p.as_char() == '&' {
                if append {
                    abort!(p.span(), "raw fd not allowed for append redirection");
                }
                self.iter.next();
                if let Some(TokenTree::Literal(lit)) = self.iter.peek_no_gap() {
                    let s = lit.to_string();
                    if s.starts_with('\"') || s.starts_with('r') {
                        abort!(lit.span(), "invalid literal string after &");
                    }
                    if &s == "1" {
                        self.args.push(ParseArg::RedirectFd(fd, 1));
                    } else if &s == "2" {
                        self.args.push(ParseArg::RedirectFd(fd, 2));
                    } else {
                        abort!(lit.span(), "Only &1 or &2 is supported");
                    }
                    self.last_redirect = None;
                    self.iter.next();
                } else {
                    abort!(self.iter.span(), "expect &1 or &2");
                }
            }
        }
    }

    fn scan_ampersand(&mut self) {
        if let Some(tt) = self.iter.peek_no_gap() {
            if let TokenTree::Punct(p) = tt {
                let span = p.span();
                if p.as_char() == '>' {
                    self.iter.next();
                    let append = self.check_append();
                    self.set_redirect(span, RedirectFd::StdoutErr { append });
                } else {
                    abort!(span, "invalid punctuation");
                }
            } else {
                abort!(tt.span(), "invalid format after '&'");
            }
        } else if self.last_redirect.is_some() {
            abort!(
                self.iter.span(),
                "wrong redirection format: no spacing permitted before '&'"
            );
        } else if self.iter.peek().is_some() {
            abort!(self.iter.span(), "invalid spacing after '&'");
        } else {
            abort!(self.iter.span(), "invalid '&' at the end");
        }
    }

    fn scan_dollar(&mut self) {
        let peek_no_gap = self.iter.peek_no_gap().map(|tt| tt.to_owned());
        // let peek_no_gap = None;
        if let Some(TokenTree::Ident(var)) = peek_no_gap {
            self.extend_last_arg(quote!(#var.as_os_str()));
        } else if let Some(TokenTree::Group(g)) = peek_no_gap {
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
                        self.extend_last_arg(quote!(#var.as_os_str()));
                    } else {
                        if !self.last_arg_str.is_empty() {
                            abort!(span, "vector variable can only be used alone");
                        }
                        self.args.push(ParseArg::ArgVec(quote!(#var)));
                    }
                    found_var = true;
                } else {
                    abort!(span, "invalid grouping: extra tokens");
                }
            }
        } else {
            abort!(self.iter.span(), "invalid token after $");
        }
        self.iter.next();
    }

    fn check_append(&mut self) -> bool {
        let mut append = false;
        if let Some(TokenTree::Punct(p)) = self.iter.peek_no_gap() {
            if p.as_char() == '>' {
                append = true;
                self.iter.next();
            }
        }
        append
    }
}

struct TokenStreamPeekable<I: Iterator<Item = TokenTree>> {
    peekable: Peekable<I>,
    span: Span,
}

impl<I: Iterator<Item = TokenTree>> Iterator for TokenStreamPeekable<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<TokenTree> {
        if let Some(tt) = self.peekable.next() {
            self.span = tt.span();
            Some(tt)
        } else {
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
                if self.span.end() == item.span().start() {
                    Some(item)
                } else {
                    None
                }
            }
        }
    }

    fn span(&self) -> Span {
        self.span
    }
}
