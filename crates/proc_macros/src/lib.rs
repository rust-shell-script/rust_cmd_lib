#![feature(proc_macro_span)]

extern crate proc_macro;
use std::iter::Peekable;
use proc_macro2::{
    token_stream::IntoIter,
    TokenStream,
    TokenTree,
    LineColumn,
    Ident,
    Literal,
};
use quote::quote;

#[proc_macro]
pub fn run_cmd(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut s = Source::new();
    let (vars, lits, src) = s.reconstruct_from(TokenStream::from(input));
    quote! (
        cmd_lib_core::run_cmd_with_ctx(
            #src,
            |sym_table| {
                #(sym_table.insert(stringify!(#vars), #vars.to_string());)*
            },
            |str_lits| {
                #(str_lits.push_back(#lits.to_string());)*
            }
        )
    ).into()
}

#[proc_macro]
pub fn run_fun(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut s = Source::new();
    let (vars, lits, src) = s.reconstruct_from(TokenStream::from(input));
    quote! (
        cmd_lib_core::run_fun_with_ctx(
            #src,
            |sym_table| {
                #(sym_table.insert(stringify!(#vars), #vars.to_string());)*
            },
            |str_lits| {
                #(str_lits.push_back(#lits.to_string());)*
            }
        )
    ).into()
}

// from inline-python: https://blog.m-ou.se/writing-python-inside-rust-1/
struct Source {
    source: String,
    line: usize,
    col: usize,
    sym_table_vars: Vec<Ident>,
    str_lits: Vec<Literal>,
}

impl Source {
    fn new() -> Self {
        Self {
            source: String::new(),
            sym_table_vars: vec![],
            str_lits: vec![],
            line: 1,
            col: 0,
        }
    }

    fn reconstruct_from(&mut self, input: TokenStream) -> (&Vec<Ident>, &Vec<Literal>, &str) {
        let mut input = input.into_iter().peekable();
        let mut with_captures = false;

        if let Some(t) = input.peek() {
            if let TokenTree::Punct(ch) = &t {
                if ch.as_char() == '|' {
                    with_captures = true;
                }
            }
        }
        if with_captures {
            self.parse_captures(&mut input);
        }

        while let Some(t) = input.next() {
            if let TokenTree::Group(g) = t {
                let s = g.to_string();
                self.add_whitespace(g.span_open().start());
                self.add_str(&s[..1]); // the '[', '{', or '('.
                self.reconstruct_from(g.stream());
                self.add_whitespace(g.span_close().start());
                self.add_str(&s[s.len() - 1..]); // the ']', '}', or ')'.
            } else {
                self.add_whitespace(t.span().start());
                if let TokenTree::Literal(lit) = t {
                    let s = lit.to_string();
                    if s.starts_with("\"") || s.starts_with("r") {
                        self.str_lits.push(lit);
                    }
                    self.add_str(&s);
                } else if let TokenTree::Ident(var) = t {
                    if self.source.ends_with("$") || self.source.ends_with("${") {
                        self.add_str(&var.to_string());
                        self.sym_table_vars.push(var);
                    } else {
                        self.add_str(&var.to_string());
                    }
                } else {
                    self.add_str(&t.to_string());
                }
            }
        }
        (&self.sym_table_vars, &self.str_lits, &self.source)
    }

    fn parse_captures(&mut self, input: &mut Peekable<IntoIter>) {
        input.next();
        while let Some(TokenTree::Ident(var)) = input.next() {
            self.sym_table_vars.push(var);
            if let Some(TokenTree::Punct(ch)) = input.next() {
                if ch.as_char() == ',' {
                    continue;
                } else if ch.as_char() == '|' {
                    break;
                } else {
                    unreachable!();
                }
            } else {
                unreachable!();
            }
        }
    }

    fn add_str(&mut self, s:&str) {
        // let's assume for now s contains no newlines.
        self.source += s;
        self.col += s.len();
    }

    fn add_whitespace(&mut self, loc: LineColumn) {
        while self.line < loc.line {
            self.source.push('\n');
            self.line += 1;
            self.col = 0;
        }
        while self.col < loc.column {
            self.source.push(' ');
            self.col += 1;
        }
    }
}
