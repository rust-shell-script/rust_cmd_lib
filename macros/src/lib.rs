use proc_macro2::{
    TokenStream,
    TokenTree,
    Ident,
    Literal,
    Span,
    Delimiter,
};
use quote::{
    quote,
    ToTokens,
};

#[proc_macro_attribute]
pub fn cmd(attr: proc_macro::TokenStream, item: proc_macro::TokenStream)
    -> proc_macro::TokenStream
{
    let cmd_name = attr.to_string();
    let config_cmd_fn = syn::Ident::new(
        &format!("config_cmd_{}", cmd_name),
        Span::call_site()
    );

    let input: syn::ItemFn = syn::parse2(item.into()).unwrap();
    let mut output = input.to_token_stream();
    let fn_ident = input.sig.ident;
    let fn_name = fn_ident.to_string();
    quote! (
        fn #config_cmd_fn() {
            println!("this is from bar, calling {}():", #fn_name);
            config_cmd(#cmd_name, #fn_ident);
        }
    ).to_tokens(&mut output);
    output.into()
}

#[proc_macro]
pub fn config_cmd(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmd_name = item.to_string();
    let config_cmd_fn = syn::Ident::new(
        &format!("config_cmd_{}", cmd_name),
        Span::call_site()
    );

    quote! (
        println!("calling config_cmd!");
        #config_cmd_fn();
    ).into()
}

#[proc_macro]
pub fn run_cmd(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let (vars, lits, src) = source_text(input.into());
    quote! (
        cmd_lib::parse_cmds_with_ctx(
            #src,
            |sym_table| {
                #(sym_table.insert(stringify!(#vars), #vars.to_string());)*
            },
            |str_lits| {
                #(str_lits.push_back(#lits.to_string());)*
            }
        ).run_cmd()
    ).into()
}

#[proc_macro]
pub fn run_fun(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let (vars, lits, src) = source_text(input.into());
    quote! (
        cmd_lib::parse_cmds_with_ctx(
            #src,
            |sym_table| {
                #(sym_table.insert(stringify!(#vars), #vars.to_string());)*
            },
            |str_lits| {
                #(str_lits.push_back(#lits.to_string());)*
            }
        ).run_fun()
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

fn source_text(input: TokenStream) -> (Vec<Ident>, Vec<Literal>, String) {
    let mut source_text = String::new();
    let mut sym_table_vars: Vec<Ident> = vec![];
    let mut str_lits: Vec<Literal> = vec![];
    let mut end = 0;
    for t in input {
        let (_start, _end) = span_location(&t.span());
        let src = t.to_string();
        if source_text.ends_with("$") {
            if let TokenTree::Group(g) = t.clone() {
                if g.delimiter() != Delimiter::Brace {
                    panic!("invalid grouping: found {:?}, only Brace is allowed", g.delimiter());
                }
                let mut found_var = false;
                for tt in g.stream() {
                    if let TokenTree::Ident(var) = tt {
                        if found_var {
                            panic!("more than one variable in grouping");
                        }
                        source_text += "{";
                        source_text += &var.to_string();
                        source_text += "}";
                        sym_table_vars.push(var);
                        found_var = true;
                    } else {
                        panic!("invalid grouping: extra tokens");
                    }
                }
                end = _end; continue;
            } else if let TokenTree::Ident(var) = t {
                if _start == end {
                    source_text += &var.to_string();
                    sym_table_vars.push(var);
                } else {
                    source_text += " ";
                    source_text += &src;
                }
                end = _end; continue;
            }
        }

        if let TokenTree::Group(_) = t {
            panic!("grouping is only allowed for variable");
        } else if let TokenTree::Literal(lit) = t {
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
            let with_brace = i < len && input[i] == '{';
            let mut var = String::new();
            if with_brace { i += 1; }
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
                sym_table_vars.push(syn::parse_str::<Ident>(&var).unwrap());
            }
        }
        i += 1;
    }
}
