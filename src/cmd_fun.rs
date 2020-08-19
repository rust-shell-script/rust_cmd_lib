use crate::{CmdResult, FunResult, Parser};

/// ## run_fun! --> FunResult
/// ```no_run
/// #[macro_use]
/// use cmd_lib::run_fun;
/// let version = run_fun!(rustc --version).unwrap();
/// eprintln!("Your rust version is {}", version);
///
/// // with pipes
/// let files = run_fun!(du -ah . | sort -hr | head -n 10).unwrap();
/// eprintln!("files: {}", files);
/// ```
#[macro_export]
macro_rules! run_fun {
   ($($cur:tt)*) => {
       $crate::Parser::new($crate::source_text!(run_fun).clone())
           .with_lits($crate::parse_string_literal!($($cur)*))
           .with_sym_table($crate::parse_sym_table!($($cur)*))
           .with_location(file!(), line!())
           .parse()
           .run_fun()
   };
}

///
/// ## run_cmd! --> CmdResult
/// ```rust
/// #[macro_use]
/// use cmd_lib::run_cmd;
///
/// let name = "rust";
/// run_cmd!(echo $name);
/// run_cmd!(|name| echo "hello, $name");
///
/// // pipe commands are also supported
/// run_cmd!(du -ah . | sort -hr | head -n 10);
///
/// // or a group of commands
/// // if any command fails, just return Err(...)
/// let file = "/tmp/f";
/// run_cmd!{
///     date;
///     ls -l $file;
/// };
/// ```
#[macro_export]
macro_rules! run_cmd {
   ($($cur:tt)*) => {
       $crate::Parser::new($crate::source_text!(run_cmd).clone())
           .with_lits($crate::parse_string_literal!($($cur)*))
           .with_sym_table($crate::parse_sym_table!($($cur)*))
           .with_location(file!(), line!())
           .parse()
           .run_cmd()
   };
}

pub fn run_cmd<S: Into<String>>(cmds: S) -> CmdResult {
    Parser::new(cmds.into()).parse().run_cmd()
}

pub fn run_fun<S: Into<String>>(cmds: S) -> FunResult {
    Parser::new(cmds.into()).parse().run_fun()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optional_args() {
        let show_all = true;
        let show_details = true;

        let mut ls_opts = String::new();
        if show_all {
            ls_opts += " -a";
        }
        if show_details {
            ls_opts += " -l";
        }
        let ls_cmd = format!(r#"ls {} | grep "\.\.$" | awk "{{print $9}}""#, ls_opts);
        assert_eq!(run_fun(ls_cmd).unwrap(), "..");
    }
}
