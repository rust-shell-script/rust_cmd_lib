use proc_macro2::{Span, TokenStream, TokenTree};
use proc_macro_error::{abort, proc_macro_error};
use quote::{quote, ToTokens};

/// Mark main function to log error result by default
///
/// ```
/// # use cmd_lib::*;
///
/// #[cmd_lib::main]
/// fn main() -> MainResult {
///     run_cmd!(bad_cmd)?;
///     Ok(())
/// }
/// // output:
/// // [ERROR] FATAL: Running ["bad_cmd"] failed: No such file or directory (os error 2)
/// ```
#[proc_macro_attribute]
pub fn main(
    _args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let orig_function: syn::ItemFn = syn::parse2(item.into()).unwrap();
    let orig_main_return_type = orig_function.sig.output;
    let orig_main_block = orig_function.block;

    quote! (
        fn main() {
            fn cmd_lib_main() #orig_main_return_type {
                #orig_main_block
            }

            cmd_lib_main().unwrap_or_else(|err| {
                ::cmd_lib::error!("FATAL: {err}");
                std::process::exit(1);
            });
        }

    )
    .into()
}

/// Export the function as an command to be run by `run_cmd!` or `run_fun!`
///
/// ```
/// # use cmd_lib::*;
/// # use std::io::Write;
/// #[export_cmd(my_cmd)]
/// fn foo(env: &mut CmdEnv) -> CmdResult {
///     let msg = format!("msg from foo(), args: {:?}\n", env.args());
///     writeln!(env.stderr(), "{}", msg)?;
///     writeln!(env.stdout(), "bar")
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

/// Import user registered custom command
/// ```
/// # use cmd_lib::*;
/// #[export_cmd(my_cmd)]
/// fn foo(env: &mut CmdEnv) -> CmdResult {
///     let msg = format!("msg from foo(), args: {:?}\n", env.args());
///     writeln!(env.stderr(), "{}", msg)?;
///     writeln!(env.stdout(), "bar")
/// }
///
/// use_custom_cmd!(my_cmd);
/// run_cmd!(my_cmd)?;
/// # Ok::<(), std::io::Error>(())
/// ```
/// Here we import the previous defined `my_cmd` command, so we can run it like a normal command.
#[proc_macro]
#[proc_macro_error]
pub fn use_custom_cmd(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let item: proc_macro2::TokenStream = item.into();
    let mut cmd_fns = vec![];
    for t in item {
        if let TokenTree::Punct(ref ch) = t {
            if ch.as_char() != ',' {
                abort!(t, "only comma is allowed");
            }
        } else if let TokenTree::Ident(cmd) = t {
            let cmd_fn = syn::Ident::new(&format!("export_cmd_{}", cmd), Span::call_site());
            cmd_fns.push(cmd_fn);
        } else {
            abort!(t, "expect a list of comma separated commands");
        }
    }

    quote! (
        #(#cmd_fns();)*
    )
    .into()
}

/// Run commands, returning result handle to check status
/// ```
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
///     ignore ls /nofile;
///     date;
///     ls oops;
///     cat oops;
/// }.is_err() {
///     // your error handling code
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
#[proc_macro]
#[proc_macro_error]
pub fn run_cmd(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::new(input.into()).scan().parse(false);
    quote! ({
        use ::cmd_lib::AsOsStr;
        #cmds.run_cmd()
    })
    .into()
}

/// Run commands, returning result handle to capture output and to check status
/// ```
/// # use cmd_lib::run_fun;
/// let version = run_fun!(rustc --version)?;
/// println!("Your rust version is {}", version);
///
/// // with pipes
/// let n = run_fun!(echo "the quick brown fox jumped over the lazy dog" | wc -w)?;
/// println!("There are {} words in above sentence", n);
/// # Ok::<(), std::io::Error>(())
/// ```
#[proc_macro]
#[proc_macro_error]
pub fn run_fun(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::new(input.into()).scan().parse(false);
    quote! ({
        use ::cmd_lib::AsOsStr;
        #cmds.run_fun()
    })
    .into()
}

/// Run commands with/without pipes as a child process, returning a handle to check the final
/// result
/// ```
/// # use cmd_lib::*;
///
/// let handle = spawn!(ping -c 10 192.168.0.1)?;
/// // ...
/// if handle.wait().is_err() {
///     // ...
/// }
/// # Ok::<(), std::io::Error>(())
#[proc_macro]
#[proc_macro_error]
pub fn spawn(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::new(input.into()).scan().parse(true);
    quote! ({
        use ::cmd_lib::AsOsStr;
        #cmds.spawn(false)
    })
    .into()
}

/// Run commands with/without pipes as a child process, returning a handle to capture the
/// final output
/// ```
/// # use cmd_lib::*;
/// let mut procs = vec![];
/// for _ in 0..4 {
///     let proc = spawn_with_output!(
///         sudo bash -c "dd if=$file of=/dev/null bs=$block_size skip=$off count=$cnt 2>&1"
///         | awk r#"/copied/{print $(NF-1) " " $NF}"#
///     )?;
///     procs.push(proc);
/// }
///
/// for (i, mut proc) in procs.into_iter().enumerate() {
///     let bandwidth = proc.wait_with_output()?;
///     info!("thread {i} bandwidth: {bandwidth} MB/s")?;
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
#[proc_macro]
#[proc_macro_error]
pub fn spawn_with_output(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let cmds = lexer::Lexer::new(input.into()).scan().parse(true);
    quote! ({
        use ::cmd_lib::AsOsStr;
        #cmds.spawn_with_output()
    })
    .into()
}

#[proc_macro]
#[proc_macro_error]
/// Log a fatal message at the error level, and exit process
///
/// e.g:
/// ```
/// # use cmd_lib::cmd_die;
/// let file = "bad_file";
/// cmd_die!("could not open file: $file");
/// // output:
/// // [ERROR] FATAL: could not open file: bad_file
/// ```
/// format should be string literals, and variable interpolation is supported.
/// Note that this macro is just for convenience. The process will exit with 1 and print
/// "FATAL: ..." messages to error console. If you want to exit with other code, you
/// should probably define your own macro or functions.
pub fn cmd_die(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let msg = parse_msg(input.into());
    quote!({
        ::cmd_lib::error!("FATAL: {}", #msg);
        std::process::exit(1)
    })
    .into()
}

fn parse_msg(input: TokenStream) -> TokenStream {
    let mut iter = input.into_iter();
    let mut output = TokenStream::new();
    let mut valid = false;
    if let Some(ref tt) = iter.next() {
        if let TokenTree::Literal(lit) = tt {
            let s = lit.to_string();
            if s.starts_with('\"') || s.starts_with('r') {
                let str_lit = lexer::scan_str_lit(lit);
                output.extend(quote!(#str_lit));
                valid = true;
            }
        }
        if !valid {
            abort!(tt, "invalid format: expect string literal");
        }
        if let Some(tt) = iter.next() {
            abort!(
                tt,
                "expect string literal only, found extra {}",
                tt.to_string()
            );
        }
    }
    output
}

mod lexer;
mod parser;
