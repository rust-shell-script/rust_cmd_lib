use crate::process::{GroupCmds, Cmds, Cmd, FdOrFile};
use ParseArg::*;

#[derive(PartialEq, Clone, Debug)]
pub enum ParseArg {
    ParsePipe,
    ParseOr,
    ParseSemicolon,
    ParseFd(i32, i32, bool),        // fd1, fd2, append?
    ParseFile(i32, String, bool),   // fd1, file, append?
    ParseArgStr(String),
    // ParseArgVec(Vec<String>),
}

#[doc(hidden)]
#[derive(Default)]
pub struct Parser {
    args: Vec<ParseArg>,
}

impl Parser {
    pub fn arg(&mut self, arg: ParseArg) -> &mut Self {
        self.args.push(arg);
        self
    }

    pub fn parse(&mut self) -> GroupCmds {
        let mut ret = GroupCmds::default();
        let mut i = 0;
        while i < self.args.len() {
            let cmd = self.parse_cmd(&mut i);
            if !cmd.0.is_empty() {
                ret.add(cmd.0, cmd.1);
            }
        }
        ret
    }

    fn parse_cmd(&mut self, i: &mut usize) -> (Cmds, Option<Cmds>) {
        let mut ret = (Cmds::default(), None);
        for j in 0..2 {
            let mut cmds = Cmds::default();
            while *i < self.args.len() {
                if self.args[*i] == ParseSemicolon {
                    *i += 1;
                    break;
                }
                let cmd = self.parse_pipe(i);
                if !cmd.is_empty() {
                    cmds.pipe(cmd);
                }
            }
            if j == 0 {
                ret.0 = cmds;
                if *i < self.args.len() && self.args[*i] != ParseOr {
                    break;
                }
            } else {
                ret.1 = Some(cmds);
            }
            *i += 1;
        }
        ret
    }

    fn parse_pipe(&mut self, i: &mut usize) -> Cmd {
        let mut ret = Cmd::default();
        while *i < self.args.len() {
            match self.args[*i].clone() {
                ParseFd(fd1, fd2, append) => ret.set_redirect(fd1, FdOrFile::Fd(fd2, append)),
                ParseFile(fd1, file, append) => ret.set_redirect(fd1, FdOrFile::File(file, append)),
                ParseArgStr(s) => ret.add_arg(s),
                ParsePipe => {
                    *i += 1;
                    break;
                },
                ParseSemicolon | ParseOr => break,
            };
            *i += 1;
        }
        ret
    }
}
