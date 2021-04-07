use proc_macro2::TokenStream;
use quote::quote;
use ParseArg::*;

#[derive(Debug)]
pub enum ParseArg {
    ParsePipe,
    ParseOr,
    ParseSemicolon,
    ParseRedirectFd(i32, i32),                 // fd1, fd2
    ParseRedirectFile(i32, TokenStream, bool), // fd1, file, append?
    ParseArgStr(TokenStream),
    ParseArgVec(TokenStream),
}

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
                if cmd1.is_none() {
                    ret.extend(quote!(.add(#cmd0, None)));
                } else {
                    ret.extend(quote!(.add(#cmd0, Some(#cmd1))));
                }
            }
        }
        ret
    }

    pub fn parse_for_spawn(&mut self) -> TokenStream {
        let mut ret = quote!(::cmd_lib::GroupCmds::default());
        let mut i = 0;
        while i < self.args.len() {
            let cmd = self.parse_cmd(&mut i);
            if !cmd.0.is_empty() {
                let (cmd0, cmd1) = cmd;
                if cmd1.is_some() || i < self.args.len() {
                    panic!("wrong spawning format");
                }
                ret.extend(quote!(.add(#cmd0, None)));
            }
        }
        ret
    }

    fn parse_cmd(&mut self, i: &mut usize) -> (TokenStream, Option<TokenStream>) {
        let mut ret = (quote!(Cmds::default()), None);
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
                } else {
                    break;
                }
            } else {
                ret.1 = Some(quote!(#cmds));
            }
            *i += 1;
        }
        ret
    }

    fn parse_pipe(&mut self, i: &mut usize) -> TokenStream {
        let mut ret = quote!(::cmd_lib::Cmd::default());
        while *i < self.args.len() {
            match &self.args[*i] {
                ParseRedirectFd(fd1, fd2) => {
                    if fd1 != fd2 {
                        let mut redirect = quote!(::cmd_lib::Redirect);
                        match (fd1, fd2) {
                            (1, 2) => redirect.extend(quote!(::StdoutToStderr)),
                            (2, 1) => redirect.extend(quote!(::StderrToStdout)),
                            _ => panic!("unsupported fd numbers: {} {}", fd1, fd2),
                        }
                        ret.extend(quote!(.add_redirect(#redirect)));
                    }
                }
                ParseRedirectFile(fd1, file, append) => {
                    let mut redirect = quote!(::cmd_lib::Redirect);
                    match fd1 {
                        0 => redirect.extend(quote!(::FileToStdin(#file))),
                        1 => redirect.extend(quote!(::StdoutToFile(#file, #append))),
                        2 => redirect.extend(quote!(::StderrToFile(#file, #append))),
                        _ => panic!("unsupported fd ({}) redirect to file {}", fd1, file),
                    }
                    ret.extend(quote!(.add_redirect(#redirect)));
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
