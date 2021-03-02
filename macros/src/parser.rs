use proc_macro2::TokenStream;
use quote::quote;
use ParseArg::*;

#[doc(hidden)]
#[derive(Clone, Debug)]
pub enum ParseArg {
    ParsePipe,
    ParseOr,
    ParseSemicolon,
    ParseRedirectFd(i32, TokenStream, bool), // fd1, fd2, append?
    ParseRedirectFile(i32, TokenStream, bool), // fd1, file, append?
    ParseArgStr(TokenStream),
    ParseArgVec(TokenStream),
}

#[doc(hidden)]
#[derive(Default)]
pub struct Parser {
    args: Vec<ParseArg>,
}

impl Parser {
    pub fn from_args(args: Vec<ParseArg>) -> Self {
        Self { args }
    }

    pub fn parse(&mut self) -> TokenStream {
        let mut ret = quote!(::cmd_lib::GroupCmds::default());
        let mut i = 0;
        while i < self.args.len() {
            let cmd = self.parse_cmd(&mut i);
            if !cmd.0.is_empty() {
                let (cmd0, cmd1) = cmd;
                ret.extend(quote!(.add(#cmd0, #cmd1)));
            }
        }
        ret
    }

    fn parse_cmd(&mut self, i: &mut usize) -> (TokenStream, TokenStream) {
        let mut ret = (quote!(Cmds::default()), quote!(None));
        for j in 0..2 {
            let mut cmds = quote!(::cmd_lib::Cmds::default());
            while *i < self.args.len() {
                let cmd = self.parse_pipe(i);
                cmds.extend(quote!(.pipe(#cmd)));
                if *i < self.args.len() {
                    match self.args[*i] {
                        ParsePipe => {}
                        _ => break,
                    }
                }
                *i += 1;
            }
            if j == 0 {
                ret.0 = cmds;
                if *i < self.args.len() {
                    match self.args[*i] {
                        ParseOr => {}
                        _ => {
                            *i += 1;
                            break;
                        }
                    }
                }
            } else {
                ret.1 = quote!(Some(#cmds));
            }
            *i += 1;
        }
        ret
    }

    fn parse_pipe(&mut self, i: &mut usize) -> TokenStream {
        let mut ret = quote!(::cmd_lib::Cmd::default());
        while *i < self.args.len() {
            match self.args[*i].clone() {
                ParseRedirectFd(fd1, fd2, append) => {
                    ret.extend(quote!(.set_redirect(#fd1, ::cmd_lib::FdOrFile::Fd(#fd2, #append))));
                }
                ParseRedirectFile(fd1, file, append) => {
                    ret.extend(
                        quote!(.set_redirect(#fd1, ::cmd_lib::FdOrFile::File(#file, #append))),
                    );
                }
                ParseArgStr(opt) => {
                    ret.extend(quote!(.add_arg(#opt)));
                }
                ParseArgVec(opts) => {
                    ret.extend(quote! (.add_args(#opts.iter().map(|s| s.to_string()).collect::<Vec<String>>())));
                }
                ParsePipe | ParseOr | ParseSemicolon => break,
            };
            *i += 1;
        }
        ret
    }
}
