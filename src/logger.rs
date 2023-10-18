use env_logger::Env;
use log::SetLoggerError;

#[doc(hidden)]
pub fn try_init_default_logger() -> Result<(), SetLoggerError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_target(false)
        .format_timestamp(None)
        .try_init()
}
