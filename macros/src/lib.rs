use proc_macro2::{Delimiter, Ident, Literal, Span, TokenStream, TokenTree, Group};
use quote::{quote, ToTokens};

#[proc_macro_attribute]
pub fn export_cmd(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let cmd_name = attr.to_string();
    let export_cmd_fn = syn::Ident::new(&format!("export_cmd_{}", cmd_name), Span::call_site());

    let input: syn::ItemFn = syn::parse2(item.into()).unwrap();
    let mut output = input.to_token_stream();
    let fn_ident = input.sig.ident;

    quote! (
        fn #export_cmd_fn() {
            export_cmd(#cmd_name, #fn_ident);
        }
    ).to_tokens(&mut output);
    output.into()
}

#[proc_macro]
pub fn use_cmd(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut cmd_fns = vec![];
    for t in item {
        if let proc_macro::TokenTree::Punct(ch) = t {
            if ch.as_char() != ',' {
                panic!("only comma is allowed");
            }
        } else if let proc_macro::TokenTree::Ident(cmd) = t {
            let cmd_fn = syn::Ident::new(&format!("export_cmd_{}", cmd), Span::call_site());
            cmd_fns.push(cmd_fn);
        } else {
            panic!("expect a list of comma separated commands");
        }
    }

    quote! (
        #(#cmd_fns();)*
    ).into()
}

#[proc_macro]
pub fn run_cmd(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let args = get_args_from_stream(input.into());
    let mut ret = quote! ( ::cmd_lib::Parser::default() );
    for arg in args {
        ret.extend(quote!(.arg));
        ret.extend(Group::new(Delimiter::Parenthesis, arg).to_token_stream());
    }
    ret.extend(quote!(.parse().run_cmd()));
    ret.into()
}

#[proc_macro]
pub fn run_fun(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let args = get_args_from_stream(input.into());
    let mut ret = quote! ( ::cmd_lib::Parser::default() );
    for arg in args {
        ret.extend(arg);
    }
    ret.extend(quote!(.parse().run_fun()));
    ret.into()
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

fn get_args_from_stream(input: TokenStream) -> Vec<TokenStream> {
    let mut args = vec![];
    let mut last_arg_stream = quote!(String::new());
    let mut last_is_dollar_sign = false;
    let mut source_text = String::new();
    let mut end = 0;
    for t in input {
        let (_start, _end) = span_location(&t.span());
        if end != 0 && end < _start { // new argument with spacing
            source_text += " ";
            args.push(quote!(::cmd_lib::ParseArg::ParseArgStr(#last_arg_stream)));
            last_arg_stream = quote!(String::new());
        }
        end = _end;

        let src = t.to_string();
        if last_is_dollar_sign {
            last_is_dollar_sign = false;
            if let TokenTree::Group(g) = t.clone() {
                if g.delimiter() != Delimiter::Brace {
                    panic!(
                        "invalid grouping: found {:?}, only Brace is allowed",
                        g.delimiter()
                    );
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
                        last_arg_stream.extend(quote!(+ &#var.to_string()));
                        found_var = true;
                    } else {
                        panic!("invalid grouping: extra tokens");
                    }
                }
                continue;
            } else if let TokenTree::Ident(var) = t {
                source_text += &var.to_string();
                last_arg_stream.extend(quote!(+ &#var.to_string()));
                continue;
            }
        }

        if let TokenTree::Group(_) = t {
            panic!("grouping is only allowed for variable");
        } else if let TokenTree::Literal(lit) = t {
            let s = lit.to_string();
            if s.starts_with("\"") || s.starts_with("r") {
                if s.starts_with("\"") {
                    parse_vars(&s[1..s.len()-1], &mut last_arg_stream);
                } else {
                    last_arg_stream.extend(quote!(+ #lit));
                }
            } else {
                last_arg_stream.extend(quote!(+ #lit));
            }
        } else {
            last_is_dollar_sign = if let TokenTree::Punct(ch) = t {
                ch.as_char() == '$'
            } else {
                false
            };
            if !last_is_dollar_sign {
                last_arg_stream.extend(quote!(+ #src));
            }
        }
        source_text += &src;
    }
    if !last_arg_stream.is_empty() {
        args.push(quote!(::cmd_lib::ParseArg::ParseArgStr(#last_arg_stream)));
    }
    dbg!(source_text);
    args
}

fn parse_vars(src: &str, last_arg_stream: &mut TokenStream) {
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
                last_arg_stream.extend(quote!(+ &#var.to_string()));
            } else {
                last_arg_stream.extend(quote!(+ &'$'.to_string()));
            }
        } else {
            let ch = input[i];
            last_arg_stream.extend(quote!(+ &#ch.to_string()));
        }
        i += 1;
    }
}
