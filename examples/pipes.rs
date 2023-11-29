#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
use cmd_lib::*;
use std::io::Read;
use std::{thread, time};

// Converted from bash script, original comments:
//
// pipes.sh: Animated pipes terminal screensaver.
// https://github.com/pipeseroni/pipes.sh
//
// Copyright (c) 2015-2018 Pipeseroni/pipes.sh contributors
// Copyright (c) 2013-2015 Yu-Jie Lin
// Copyright (c) 2010 Matthew Simpson
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

const VERSION: &str = "1.3.0";

const M: i32 = 32768; // Bash RANDOM maximum + 1
tls_init!(p, i32, 1); // number of pipes
tls_init!(f, i32, 75); // frame rate
tls_init!(s, i32, 13); // probability of straight fitting
tls_init!(r, i32, 2000); // characters limit
tls_init!(t, i32, 0); // iteration counter for -r character limit
tls_init!(w, i32, 80); // terminal size
tls_init!(h, i32, 24);

// ab -> sets[][idx] = a*4 + b
// 0: up, 1: right, 2: down, 3: left
// 00 means going up   , then going up   -> ┃
// 12 means going right, then going down -> ┓
#[rustfmt::skip]
tls_init!(sets, Vec<String>, [
    r"┃┏ ┓┛━┓  ┗┃┛┗ ┏━",
    r"│╭ ╮╯─╮  ╰│╯╰ ╭─",
    r"│┌ ┐┘─┐  └│┘└ ┌─",
    r"║╔ ╗╝═╗  ╚║╝╚ ╔═",
    r"|+ ++-+  +|++ +-",
    r"|/ \/-\  \|/\ /-",
    r".. ....  .... ..",
    r".o oo.o  o.oo o.",
    r"-\ /\|/  /-\/ \|",  // railway
    r"╿┍ ┑┚╼┒  ┕╽┙┖ ┎╾",  // knobby pipe
].iter().map(|ns| ns.to_string()).collect());
// rearranged all pipe chars into individual elements for easier access
tls_init!(SETS, Vec<char>, vec![]);

// pipes'
tls_init!(x, Vec<i32>, vec![]); // current position
tls_init!(y, Vec<i32>, vec![]);
tls_init!(l, Vec<i32>, vec![]); // current directions
                                // 0: up, 1: right, 2: down, 3: left
tls_init!(n, Vec<i32>, vec![]); // new directions
tls_init!(v, Vec<i32>, vec![]); // current types
tls_init!(c, Vec<String>, vec![]); // current escape codes

// selected pipes'
tls_init!(V, Vec<i32>, vec![0]); // types (indexes to sets[])
tls_init!(C, Vec<i32>, vec![1, 2, 3, 4, 5, 6, 7, 0]); // color indices for tput setaf
tls_init!(VN, i32, 1); // number of selected types
tls_init!(CN, i32, 8); // number of selected colors
tls_init!(E, Vec<String>, vec![]); // pre-generated escape codes from BOLD, NOCOLOR, and C

// switches
tls_init!(RNDSTART, bool, false); // randomize starting position and direction
tls_init!(BOLD, bool, true);
tls_init!(NOCOLOR, bool, false);
tls_init!(KEEPCT, bool, false); // keep pipe color and type

fn prog_name() -> String {
    let arg0 = std::env::args().next().unwrap();
    run_fun!(basename $arg0).unwrap()
}

// print help message in 72-char width
fn print_help() {
    let prog = prog_name();
    let max_type = tls_get!(sets).len() - 1;
    let cgap = " ".repeat(15 - format!("{}", tls_get!(COLORS)).chars().count());
    let colors = run_fun!(tput colors).unwrap();
    let term = std::env::var("TERM").unwrap();
    #[rustfmt::skip]
    eprintln!("
Usage: {prog} [OPTION]...
Animated pipes terminal screensaver.

  -p [1-]               number of pipes (D=1)
  -t [0-{max_type}]              pipe type (D=0)
  -t c[16 chars]        custom pipe type
  -c [0-{colors}]{cgap}pipe color INDEX (TERM={term}), can be
                        hexadecimal with '#' prefix
                        (D=-c 1 -c 2 ... -c 7 -c 0)
  -f [20-100]           framerate (D=75)
  -s [5-15]             going straight probability, 1 in (D=13)
  -r [0-]               reset after (D=2000) characters, 0 if no reset
  -R                    randomize starting position and direction
  -B                    no bold effect
  -C                    no color
  -K                    keep pipe color and type when crossing edges
  -h                    print this help message
  -v                    print version number

Note: -t and -c can be used more than once.");
}

// parse command-line options
// It depends on a valid COLORS which is set by _CP_init_termcap_vars
fn parse() -> CmdResult {
    // test if $1 is a natural number in decimal, an integer >= 0
    fn is_N(arg_opt: Option<String>) -> (bool, i32) {
        if let Some(arg) = arg_opt {
            if let Ok(vv) = arg.parse::<i32>() {
                return (vv >= 0, vv);
            }
        }
        (false, 0)
    }

    // test if $1 is a hexadecimal string
    fn is_hex(arg: &str) -> (bool, i32) {
        if let Ok(vv) = i32::from_str_radix(&arg, 16) {
            return (true, vv);
        }
        (false, 0)
    }

    // print error message for invalid argument to standard error, this
    // - mimics getopts error message
    // - use all positional parameters as error message
    // - has a newline appended
    // $arg and $OPTARG are the option name and argument set by getopts.
    fn pearg(arg: &str, msg: &str) -> ! {
        let arg0 = prog_name();
        info!("{arg0}: -{arg} invalid argument; {msg}");
        print_help();
        std::process::exit(1)
    }

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-p" => {
                let (is_valid, vv) = is_N(args.next());
                if is_valid && vv > 0 {
                    tls_set!(p, |np| *np = vv);
                } else {
                    pearg(&arg, "must be an integer and greater than 0");
                }
            }
            "-t" => {
                let arg_opt = args.next();
                let (is_valid, vv) = is_N(arg_opt.clone());
                let arg_str = arg_opt.unwrap_or_default();
                let len = tls_get!(sets).len() as i32;
                if arg_str.chars().count() == 16 {
                    tls_set!(V, |nv| nv.push(len));
                    tls_set!(sets, |ns| ns.push(arg_str));
                } else if is_valid && vv < len {
                    tls_set!(V, |nv| nv.push(vv));
                } else {
                    pearg(
                        &arg,
                        &format!("must be an integer and from 0 to {}; or a custom type", len),
                    );
                }
            }
            "-c" => {
                let arg_opt = args.next();
                let (is_valid, vv) = is_N(arg_opt.clone());
                let arg_str = arg_opt.unwrap_or_default();
                if arg_str.starts_with("#") {
                    let (is_valid_hex, hv) = is_hex(&arg_str[1..]);
                    if !is_valid_hex {
                        pearg(&arg, "unrecognized hexadecimal string");
                    }
                    if hv >= tls_get!(COLORS) {
                        pearg(
                            &arg,
                            &format!("hexadecimal must be from #0 to {:X}", tls_get!(COLORS) - 1),
                        );
                    }
                    tls_set!(C, |nc| nc.push(hv));
                } else if is_valid && vv < tls_get!(COLORS) {
                    tls_set!(C, |nc| nc.push(vv));
                } else {
                    pearg(
                        &arg,
                        &format!(
                            "must be an integer and from 0 to {};
                             or a hexadecimal string with # prefix",
                            tls_get!(COLORS) - 1
                        ),
                    );
                }
            }
            "-f" => {
                let (is_valid, vv) = is_N(args.next());
                if is_valid && vv >= 20 && vv <= 100 {
                    tls_set!(f, |nf| *nf = vv);
                } else {
                    pearg(&arg, "must be an integer and from 20 to 100");
                }
            }
            "-s" => {
                let (is_valid, vv) = is_N(args.next());
                if is_valid && vv >= 5 && vv <= 15 {
                    tls_set!(r, |nr| *nr = vv);
                } else {
                    pearg(&arg, "must be a non-negative integer");
                }
            }
            "-r" => {
                let (is_valid, vv) = is_N(args.next());
                if is_valid && vv > 0 {
                    tls_set!(r, |nr| *nr = vv);
                } else {
                    pearg(&arg, "must be a non-negative integer");
                }
            }
            "-R" => tls_set!(RNDSTART, |nr| *nr = true),
            "-B" => tls_set!(BOLD, |nb| *nb = false),
            "-C" => tls_set!(NOCOLOR, |nc| *nc = true),
            "-K" => tls_set!(KEEPCT, |nk| *nk = true),
            "-h" => {
                print_help();
                std::process::exit(0);
            }
            "-v" => {
                let arg0 = std::env::args().next().unwrap();
                let prog = run_fun!(basename $arg0)?;
                run_cmd!(echo $prog $VERSION)?;
                std::process::exit(0);
            }
            _ => {
                pearg(
                    &arg,
                    &format!("illegal arguments -- {}; no arguments allowed", arg),
                );
            }
        }
    }
    Ok(())
}

fn cleanup() -> CmdResult {
    let sgr0 = tls_get!(SGR0);
    run_cmd!(
        tput reset;  // fix for konsole, see pipeseroni/pipes.sh#43
        tput rmcup;
        tput cnorm;
        stty echo;
        echo $sgr0;
    )?;

    Ok(())
}

fn resize() -> CmdResult {
    let cols = run_fun!(tput cols)?.parse().unwrap();
    let lines = run_fun!(tput lines)?.parse().unwrap();
    tls_set!(w, |nw| *nw = cols);
    tls_set!(h, |nh| *nh = lines);
    Ok(())
}

fn init_pipes() {
    // +_CP_init_pipes
    let mut ci = if tls_get!(KEEPCT) {
        0
    } else {
        tls_get!(CN) * rand() / M
    };

    let mut vi = if tls_get!(RNDSTART) {
        0
    } else {
        tls_get!(VN) * rand() / M
    };

    for _ in 0..tls_get!(p) as usize {
        tls_set!(n, |nn| nn.push(0));
        tls_set!(l, |nl| nl.push(if tls_get!(RNDSTART) {
            rand() % 4
        } else {
            0
        }));
        tls_set!(x, |nx| nx.push(if tls_get!(RNDSTART) {
            tls_get!(w) * rand() / M
        } else {
            tls_get!(w) / 2
        }));
        tls_set!(y, |ny| ny.push(if tls_get!(RNDSTART) {
            tls_get!(h) * rand() / M
        } else {
            tls_get!(h) / 2
        }));
        tls_set!(v, |nv| nv.push(tls_get!(V)[vi as usize]));
        tls_set!(c, |nc| nc.push(tls_get!(E)[ci as usize].clone()));
        ci = (ci + 1) % tls_get!(CN);
        vi = (vi + 1) % tls_get!(VN);
    }
    // -_CP_init_pipes
}

fn init_screen() -> CmdResult {
    run_cmd!(
        stty -echo -isig -icanon min 0 time 0;
        tput smcup;
        tput civis;
        tput clear;
    )?;
    resize()?;
    Ok(())
}

tls_init!(SGR0, String, String::new());
tls_init!(SGR_BOLD, String, String::new());
tls_init!(COLORS, i32, 0);

fn rand() -> i32 {
    run_fun!(bash -c r"echo $RANDOM").unwrap().parse().unwrap()
}

#[cmd_lib::main]
fn main() -> CmdResult {
    // simple pre-check of TERM, tput's error message should be enough
    let term = std::env::var("TERM").unwrap();
    run_cmd!(tput -T $term sgr0 >/dev/null)?;

    // +_CP_init_termcap_vars
    let colors = run_fun!(tput colors)?.parse().unwrap();
    tls_set!(COLORS, |nc| *nc = colors); // COLORS - 1 == maximum color index for -c argument
    tls_set!(SGR0, |ns| *ns = run_fun!(tput sgr0).unwrap());
    tls_set!(SGR_BOLD, |nb| *nb = run_fun!(tput bold).unwrap());
    // -_CP_init_termcap_vars

    parse()?;

    // +_CP_init_VC
    // set default values if not by options
    tls_set!(VN, |vn| *vn = tls_get!(V).len() as i32);
    tls_set!(CN, |cn| *cn = tls_get!(C).len() as i32);
    // -_CP_init_VC

    // +_CP_init_E
    // generate E[] based on BOLD (SGR_BOLD), NOCOLOR, and C for each element in
    // C, a corresponding element in E[] =
    //   SGR0
    //   + SGR_BOLD, if BOLD
    //   + tput setaf C, if !NOCOLOR
    for i in 0..(tls_get!(CN) as usize) {
        tls_set!(E, |ne| ne.push(tls_get!(SGR0)));
        if tls_get!(BOLD) {
            tls_set!(E, |ne| ne[i] += &tls_get!(SGR_BOLD));
        }
        if !tls_get!(NOCOLOR) {
            let cc = tls_get!(C)[i];
            let setaf = run_fun!(tput setaf $cc)?;
            tls_set!(E, |ne| ne[i] += &setaf);
        }
    }
    // -_CP_init_E

    // +_CP_init_SETS
    for i in 0..tls_get!(sets).len() {
        for j in 0..16 {
            let cc = tls_get!(sets)[i].chars().nth(j).unwrap();
            tls_set!(SETS, |ns| ns.push(cc));
        }
    }
    // -_CP_init_SETS

    init_screen()?;
    init_pipes();

    loop {
        thread::sleep(time::Duration::from_millis(1000 / tls_get!(f) as u64));
        let mut buffer = String::new();
        if std::io::stdin().read_to_string(&mut buffer).is_ok() {
            match buffer.as_str() {
                "q" | "\u{1b}" | "\u{3}" => {
                    cleanup()?; // q, ESC or Ctrl-C to exit
                    break;
                }
                "P" => tls_set!(s, |ns| *ns = if *ns < 15 { *ns + 1 } else { *ns }),
                "O" => tls_set!(s, |ns| *ns = if *ns > 3 { *ns - 1 } else { *ns }),
                "F" => tls_set!(f, |nf| *nf = if *nf < 100 { *nf + 1 } else { *nf }),
                "D" => tls_set!(f, |nf| *nf = if *nf > 20 { *nf - 1 } else { *nf }),
                "B" => tls_set!(BOLD, |nb| *nb = !*nb),
                "C" => tls_set!(NOCOLOR, |nc| *nc = !*nc),
                "K" => tls_set!(KEEPCT, |nk| *nk = !*nk),
                _ => (),
            }
        }
        for i in 0..(tls_get!(p) as usize) {
            // New position:
            // l[] direction = 0: up, 1: right, 2: down, 3: left
            // +_CP_newpos
            if tls_get!(l)[i] % 2 == 1 {
                tls_set!(x, |nx| nx[i] += -tls_get!(l)[i] + 2);
            } else {
                tls_set!(y, |ny| ny[i] += tls_get!(l)[i] - 1);
            }
            // -_CP_newpos

            // Loop on edges (change color on loop):
            // +_CP_warp
            if !tls_get!(KEEPCT) {
                if tls_get!(x)[i] >= tls_get!(w)
                    || tls_get!(x)[i] < 0
                    || tls_get!(y)[i] >= tls_get!(h)
                    || tls_get!(y)[i] < 0
                {
                    tls_set!(c, |nc| nc[i] =
                        tls_get!(E)[(tls_get!(CN) * rand() / M) as usize].clone());
                    tls_set!(v, |nv| nv[i] =
                        tls_get!(V)[(tls_get!(VN) * rand() / M) as usize].clone());
                }
            }
            tls_set!(x, |nx| nx[i] = (nx[i] + tls_get!(w)) % tls_get!(w));
            tls_set!(y, |ny| ny[i] = (ny[i] + tls_get!(h)) % tls_get!(h));
            // -_CP_warp

            // new turning direction:
            // $((s - 1)) in $s, going straight, therefore n[i] == l[i];
            // and 1 in $s that pipe makes a right or left turn
            //
            //     s * rand() / M - 1 == 0
            //     n[i] == -1
            //  => n[i] == l[i] + 1 or l[i] - 1
            // +_CP_newdir
            tls_set!(n, |nn| nn[i] = tls_get!(s) * rand() / M - 1);
            tls_set!(n, |nn| nn[i] = if nn[i] >= 0 {
                tls_get!(l)[i]
            } else {
                tls_get!(l)[i] + (2 * (rand() % 2) - 1)
            });
            tls_set!(n, |nn| nn[i] = (nn[i] + 4) % 4);
            // -_CP_newdir

            // Print:
            // +_CP_print
            let ii = tls_get!(v)[i] * 16 + tls_get!(l)[i] * 4 + tls_get!(n)[i];
            eprint!(
                "\u{1b}[{};{}H{}{}",
                tls_get!(y)[i] + 1,
                tls_get!(x)[i] + 1,
                tls_get!(c)[i],
                tls_get!(SETS)[ii as usize]
            );
            // -_CP_print
            tls_set!(l, |nl| nl[i] = tls_get!(n)[i]);
        }

        if tls_get!(r) > 0 && tls_get!(t) * tls_get!(p) >= tls_get!(r) {
            run_cmd!(
                tput reset;
                tput civis;
                stty -echo -isig -icanon min 0 time 0;
            )?;
            tls_set!(t, |nt| *nt = 0);
        } else {
            tls_set!(t, |nt| *nt += 1);
        }
    }
    Ok(())
}
