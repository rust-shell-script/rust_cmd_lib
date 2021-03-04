use proc_macro2::{Span, TokenStream};
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
pub fn use_custom_cmd(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
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
    )
    .into()
}

#[proc_macro]
pub fn use_builtin_cmd(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut ret = TokenStream::new();
    for t in item {
        if let proc_macro::TokenTree::Punct(ch) = t {
            if ch.as_char() != ',' {
                panic!("only comma is allowed");
            }
        } else if let proc_macro::TokenTree::Ident(cmd) = t {
            let cmd_name = cmd.to_string();
            let cmd_fn = syn::Ident::new(&format!("builtin_{}", cmd_name), Span::call_site());
            ret.extend(quote!(::cmd_lib::export_cmd(#cmd_name, ::cmd_lib::#cmd_fn);));
        } else {
            panic!("expect a list of comma separated commands");
        }
    }

    ret.into()
}

#[proc_macro]
pub fn run_cmd(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::from(input.into()).scan().parse();
    quote! ({
        #cmds.run_cmd()
    })
    .into()
}

#[proc_macro]
pub fn run_fun(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::from(input.into()).scan().parse();
    quote! ({
        #cmds.run_fun()
    })
    .into()
}

#[proc_macro]
pub fn spawn(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::from(input.into()).scan().parse_for_spawn();
    quote! ({
        #cmds.spawn()
    })
    .into()
}

#[proc_macro]
pub fn spawn_with_output(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::from(input.into()).scan().parse_for_spawn();
    quote! ({
        #cmds.spawn_with_output()
    })
    .into()
}

mod lexer;
mod parser;
