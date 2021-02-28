use cmd_lib::{run_cmd, CmdResult};

fn main() -> CmdResult {
    cmd_lib::set_debug(true);
    // let dirs = vec!["/", "/var"];
    // run_cmd!(ls $[dirs])?;

    let gopts = vec![vec!["-l", "-a", "/"], vec!["-a", "/var"]];
    for opts in gopts {
      run_cmd!(ls $[opts]).unwrap();
    }

    Ok(())
}
