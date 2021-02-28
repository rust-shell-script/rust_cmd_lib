use proc_macro2::{Delimiter, Ident, Span, TokenStream, TokenTree, Group};
use quote::{quote, ToTokens};

#[proc_macro_attribute]
pub fn export_cmd(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let cmd_name = attr.to_string();
    let export_cmd_fn = syn::Ident::new(&format!("export_cmd_{}", cmd_name), Span::call_site());

    let orig_function: syn::ItemFn = syn::parse2(item.into()).unwrap();
    let fn_ident = &orig_function.sig.ident;

    let mut new_functions = orig_function.to_token_stream();
    new_functions.extend(quote! (
        fn #export_cmd_fn() {
            export_cmd(#cmd_name, #fn_ident);
        }
    ));
    new_functions.into()
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
    let mut cmds = parse_cmds_from_stream(input.into());
    cmds.extend(quote!(.run_cmd()));
    cmds.into()
}

#[proc_macro]
pub fn run_fun(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut cmds = parse_cmds_from_stream(input.into());
    cmds.extend(quote!(.run_fun()));
    cmds.into()
}

fn parse_cmds_from_stream(input: TokenStream) -> TokenStream {
    let args = get_args_from_stream(input);
    let mut ret = quote! ( ::cmd_lib::Parser::default() );
    for arg in args {
        ret.extend(quote!(.arg));
        ret.extend(Group::new(Delimiter::Parenthesis, arg).to_token_stream());
    }
    ret.extend(quote!(.parse()));
    ret
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
    let mut last_arg_empty = true;
    let mut last_is_dollar_sign = false;
    let mut last_is_pipe = false;
    let mut end = 0;
    for t in input {
        let (_start, _end) = span_location(&t.span());
        if end != 0 && end < _start { // new argument with spacing
            if !last_arg_empty {
                args.push(quote!(::cmd_lib::ParseArg::ParseArgStr(#last_arg_stream)));
            }
            last_arg_stream = quote!(String::new());
            last_arg_empty = true;
        }
        end = _end;

        let src = t.to_string();
        if last_is_dollar_sign {
            last_is_dollar_sign = false;
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
                            last_arg_stream.extend(quote!(+ &#var.to_string()));
                            last_arg_empty = false;
                        } else {
                            assert!(last_arg_empty);
                            args.push(quote! (
                                ::cmd_lib::ParseArg::ParseArgVec(
                                    #var.iter().map(|s| s.to_string()).collect::<Vec<String>>()))
                            );
                        }
                        found_var = true;
                    } else {
                        panic!("invalid grouping: extra tokens");
                    }
                }
                continue;
            } else if let TokenTree::Ident(var) = t {
                last_arg_stream.extend(quote!(+ &#var.to_string()));
                last_arg_empty = false;
                continue;
            }
        }

        if let TokenTree::Group(_) = t {
            panic!("grouping is only allowed for variable");
        } else if let TokenTree::Literal(lit) = t {
            last_arg_empty = false;
            let s = lit.to_string();
            if s.starts_with("\"") || s.starts_with("r") {
                if s.starts_with("\"") {
                    parse_vars(&s[1..s.len()-1], &mut last_arg_stream);
                } else {
                    last_arg_stream.extend(quote!(+ #lit));
                }
            } else {
                last_arg_stream.extend(quote!(+ &#lit.to_string()));
            }
        } else {
            if let TokenTree::Punct(p) = t {
                let ch = p.as_char();
                if ch == '$' {
                    last_is_dollar_sign = true;
                    last_is_pipe = false;
                    continue;
                } else if ch == ';' {
                    if !last_arg_empty {
                        args.push(quote!(::cmd_lib::ParseArg::ParseArgStr(#last_arg_stream)));
                    }
                    args.push(quote!(::cmd_lib::ParseArg::ParseSemicolon));
                    last_arg_empty = true;
                    last_is_pipe = false;
                    continue;
                } else if ch == '|' {
                    if !last_arg_empty {
                        args.push(quote!(::cmd_lib::ParseArg::ParseArgStr(#last_arg_stream)));
                    }
                    if last_is_pipe {
                        args.pop();
                        args.push(quote!(::cmd_lib::ParseArg::ParseOr));
                        last_is_pipe = false;
                    } else {
                        args.push(quote!(::cmd_lib::ParseArg::ParsePipe));
                        last_is_pipe = true;
                    }
                    last_arg_empty = true;
                    continue;
                }
            }

            last_arg_stream.extend(quote!(+ &#src.to_string()));
            last_arg_empty = false;
            last_is_pipe = false;
        }
    }
    if !last_arg_empty {
        args.push(quote!(::cmd_lib::ParseArg::ParseArgStr(#last_arg_stream)));
    }
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
