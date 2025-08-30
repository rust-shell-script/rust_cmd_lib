use cmd_lib::{CmdResult, run_cmd};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};

#[cmd_lib::main]
fn main() -> CmdResult {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    copy_thing()?;

    Ok(())
}

#[tracing::instrument]
fn copy_thing() -> CmdResult {
    // Log output from stderr inherits the `copy_thing` span from this function
    run_cmd!(dd if=/dev/urandom of=/dev/null bs=1M count=1000)?;

    Ok(())
}
