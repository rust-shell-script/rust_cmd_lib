use proc_macro2::{
    TokenStream,
    TokenTree,
    Ident,
    Literal,
    Span,
};
use quote::quote;

#[proc_macro]
pub fn run_cmd(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let (vars, lits, src) = source_text(input);
    quote! (
        cmd_lib::run_cmd_with_ctx(
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
    let (vars, lits, src) = source_text(input);
    quote! (
        cmd_lib::run_fun_with_ctx(
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

fn source_text(input: proc_macro::TokenStream) -> (Vec<Ident>, Vec<Literal>, String) {
    let input = TokenStream::from(input);
    let mut source_text = String::new();
    let mut sym_table_vars: Vec<Ident> = vec![];
    let mut str_lits: Vec<Literal> = vec![];
    let mut end = 0;
    for t in input {
        let (_start, _end) = span_location(&t.span());
        let src = t.to_string();
        if source_text.ends_with("$") {
            if let TokenTree::Group(g) = t {
                for tt in g.stream() {
                    if let TokenTree::Ident(var) = tt {
                        source_text += "{";
                        source_text += &var.to_string();
                        source_text += "}";
                        sym_table_vars.push(var);
                        break;
                    }
                }
            } else if let TokenTree::Ident(var) = t {
                source_text += &var.to_string();
                sym_table_vars.push(var);
            }
        } else {
            if let TokenTree::Literal(lit) = t {
                let s = lit.to_string();
                if s.starts_with("\"") || s.starts_with("r") {
                    if s.starts_with("\"") {
                        parse_vars(&s, &mut sym_table_vars);
                    }
                    str_lits.push(lit);
                }
            }

            if end != 0 && end < _start {
                source_text += " ";
            }
            source_text += &src;
        }
        end = _end;
    }
    (sym_table_vars, str_lits, source_text)
}

fn parse_vars(src: &str, sym_table_vars: &mut Vec<Ident>) {
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
                    || (input[i] == '_')) {
                var.push(input[i]);
                i += 1;
            }
            if with_bracket {
                assert_eq!(input[i], '}');
            } else {
                i -= 1; // back off 1 char
            }
            sym_table_vars.push(syn::parse_str::<Ident>(&var).unwrap());
        }
        i += 1;
    }
}
