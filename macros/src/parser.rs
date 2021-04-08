use proc_macro2::TokenStream;
use quote::quote;
use std::iter::Peekable;
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

pub struct Parser<I: Iterator<Item = ParseArg>> {
    iter: Peekable<I>,
}

impl<I: Iterator<Item = ParseArg>> Parser<I> {
    pub fn from(iter: Peekable<I>) -> Self {
        Self { iter }
    }

    pub fn parse(mut self, for_spawn: bool) -> TokenStream {
        let mut ret = quote!(::cmd_lib::GroupCmds::default());
        while self.iter.peek().is_some() {
            let cmd = self.parse_cmd();
            if !cmd.0.is_empty() {
                let (cmd0, cmd1) = cmd;
                if cmd1.is_none() {
                    ret.extend(quote!(.add(#cmd0, None)));
                } else if for_spawn {
                    panic!("wrong spawning format: or command not allowed");
                } else {
                    ret.extend(quote!(.add(#cmd0, Some(#cmd1))));
                }
                if for_spawn && self.iter.peek().is_some() {
                    panic!("wrong spawning format: group command not allowed");
                }
            }
        }
        ret
    }

    fn parse_cmd(&mut self) -> (TokenStream, Option<TokenStream>) {
        let mut ret = (quote!(Cmds::default()), None);
        for j in 0..2 {
            let mut cmds = quote!(::cmd_lib::Cmds::default());
            while self.iter.peek().is_some() {
                let cmd = self.parse_pipe();
                cmds.extend(quote!(.pipe(#cmd)));
                if !matches!(self.iter.peek(), Some(ParsePipe)) {
                    break;
                }
                self.iter.next();
            }
            if j == 0 {
                ret.0 = cmds;
                if !matches!(self.iter.peek(), Some(ParseOr)) {
                    self.iter.next();
                    break;
                }
            } else {
                ret.1 = Some(quote!(#cmds));
            }
            self.iter.next();
        }
        ret
    }

    fn parse_pipe(&mut self) -> TokenStream {
        let mut ret = quote!(::cmd_lib::Cmd::default());
        while let Some(arg) = self.iter.peek() {
            match arg {
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
            }
            self.iter.next();
        }
        ret
    }
}
