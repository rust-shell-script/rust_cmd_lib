use cmd_lib::{ CmdResult, run_cmd, run_fun };

fn main() -> CmdResult {
    let red = run_fun!(tput setaf 1)?;
    let green = run_fun!(tput setaf 2)?;
    let reset = run_fun!(tput sgr0)?;
    run_cmd!(|red, green, reset| /bin/echo "1: ${red}red text ${green}green text${reset}")?;
    println!("2: \x1b[0;31mred text \x1b[0;32mgreen text\x1b[0m");
    run_cmd!(bash -c r#"echo -e "3: \x1b[0;31mred text \x1b[0;32mgreen text\x1b[0m""#)?;
    run_cmd!(bash -c "echo -e '4: \x1b[0;31mred text \x1b[0;32mgreen text\x1b[0m'")?;
    Ok(())
}
