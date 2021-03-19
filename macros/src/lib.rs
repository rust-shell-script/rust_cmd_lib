use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{quote, quote_spanned, ToTokens};

/// export the function as an command to be run by `run_cmd!` or `run_fun!`
///
/// ```
/// # use cmd_lib::*;
/// #[export_cmd(my_cmd)]
/// fn foo(args: CmdArgs, _envs: CmdEnvs) -> FunResult {
///     println!("msg from foo(), args: {:?}", args);
///     Ok("bar".into())
/// }
///
/// use_custom_cmd!(my_cmd);
/// run_cmd!(my_cmd)?;
/// println!("get result: {}", run_fun!(my_cmd)?);
/// # Ok::<(), std::io::Error>(())
/// ```
/// Here we export function `foo` as `my_cmd` command.

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

/// import user registered custom command
/// ```
/// # use cmd_lib::*;
/// #[export_cmd(my_cmd)]
/// fn foo(args: CmdArgs, _envs: CmdEnvs) -> FunResult {
///     println!("msg from foo(), args: {:?}", args);
///     Ok("bar".into())
/// }
///
/// use_custom_cmd!(my_cmd);
/// run_cmd!(my_cmd)?;
/// # Ok::<(), std::io::Error>(())
/// ```
/// Here we import the previous defined `my_cmd` command, so we can run it like a normal command.
#[proc_macro]
pub fn use_custom_cmd(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let item: proc_macro2::TokenStream = item.into();
    let mut cmd_fns = vec![];
    for t in item {
        if let TokenTree::Punct(ref ch) = t {
            if ch.as_char() != ',' {
                return quote_spanned!(t.span() => compile_error!("only comma is allowed")).into();
            }
        } else if let TokenTree::Ident(cmd) = t {
            let cmd_fn = syn::Ident::new(&format!("export_cmd_{}", cmd), Span::call_site());
            cmd_fns.push(cmd_fn);
        } else {
            return quote_spanned!(
                t.span() => compile_error!("expect a list of comma separated commands")
            )
            .into();
        }
    }

    quote! (
        #(#cmd_fns();)*
    )
    .into()
}

/// import library predefined builtin command
#[proc_macro]
/// ```
/// # use cmd_lib::*;
/// use_builtin_cmd!(info); // import only one builtin command
/// use_builtin_cmd!(true, echo, info, warn, err, die); // import all the builtins
/// ```
/// `cd` builtin command is always enabled without importing it.
pub fn use_builtin_cmd(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let item: proc_macro2::TokenStream = item.into();
    let mut ret = TokenStream::new();
    for t in item {
        if let TokenTree::Punct(ref ch) = t {
            if ch.as_char() != ',' {
                return quote_spanned!(t.span() => compile_error!("only comma is allowed")).into();
            }
        } else if let TokenTree::Ident(cmd) = t {
            let cmd_name = cmd.to_string();
            let cmd_fn = syn::Ident::new(&format!("builtin_{}", cmd_name), Span::call_site());
            ret.extend(quote!(::cmd_lib::export_cmd(#cmd_name, ::cmd_lib::#cmd_fn);));
        } else {
            return quote_spanned!(
                t.span() => compile_error!("expect a list of comma separated commands")
            )
            .into();
        }
    }

    ret.into()
}

/// Run commands, returning result handle to check status
/// ```no_run
/// # use cmd_lib::run_cmd;
/// let msg = "I love rust";
/// run_cmd!(echo $msg)?;
/// run_cmd!(echo "This is the message: $msg")?;
///
/// // pipe commands are also supported
/// run_cmd!(du -ah . | sort -hr | head -n 10)?;
///
/// // or a group of commands
/// // if any command fails, just return Err(...)
/// let file = "/tmp/f";
/// let keyword = "rust";
/// if run_cmd! {
///     cat ${file} | grep ${keyword};
///     echo "bad cmd" >&2;
///     ls /nofile || true;
///     date;
///     ls oops;
///     cat oops;
/// }.is_err() {
///     // your error handling code
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
#[proc_macro]
pub fn run_cmd(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::from(input.into()).scan().parse();
    quote! ({
        #cmds.run_cmd()
    })
    .into()
}

/// Run commands, returning result handle to capture output and to check status
/// ```
/// # use cmd_lib::run_fun;
/// let version = run_fun!(rustc --version)?;
/// eprintln!("Your rust version is {}", version);
///
/// // with pipes
/// let n = run_fun!(echo "the quick brown fox jumped over the lazy dog" | wc -w)?;
/// eprintln!("There are {} words in above sentence", n);
/// # Ok::<(), std::io::Error>(())
/// ```
#[proc_macro]
pub fn run_fun(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::from(input.into()).scan().parse();
    quote! ({
        #cmds.run_fun()
    })
    .into()
}

/// Run commands with/without pipes as a child process, returning a handle to check the final
/// status
/// ```no_run
/// # use cmd_lib::*;
///
/// let handle = spawn!(ping -c 10 192.168.0.1)?;
/// // ...
/// if handle.wait_result().is_err() {
///     // ...
/// }
/// # Ok::<(), std::io::Error>(())
#[proc_macro]
pub fn spawn(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::from(input.into()).scan().parse_for_spawn();
    quote! ({
        #cmds.spawn()
    })
    .into()
}

/// Run commands with/without pipes as a child process, returning a handle to capture the
/// final output
/// ```no_run
/// # use cmd_lib::*;
/// // from examples/dd_test.rs:
/// let mut procs = vec![];
/// for _ in 0..4 {
///     let proc = spawn_with_output!(
///         sudo bash -c "dd if=$file of=/dev/null bs=$block_size skip=$off count=$cnt 2>&1"
///     )?;
/// }
///
/// for (i, mut proc) in procs.into_iter().enumerate() {
///     let output = proc.wait_result()?;
///     run_cmd!(info "thread $i bandwidth: $bandwidth MB/s")?;
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
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
