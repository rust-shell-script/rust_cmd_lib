use cmd_lib::{ CmdResult, run_cmd };
fn main() -> CmdResult {
    // raw literal string
    run_cmd!(ping -c 10 www.google.com | awk r#"/time/ {print $(NF-3) " " $(NF-1) " " $NF}"#)?;

    // string interpolation
    let key_word = "time";
    let awk_opts = format!(r#"/{}/ {{print $(NF-3) " " $(NF-1) " " $NF}}"#, key_word);
    run_cmd!(ping -c 10 www.google.com | awk $awk_opts)?;

    Ok(())
}
