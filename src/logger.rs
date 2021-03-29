use log::{Level, LevelFilter, Metadata, Record};

static LOGGER: CmdLogger = CmdLogger;

/// Initializes the builtin cmd_lib logger
///
/// This is to make examples in this library work, and users should usually use a real logger
/// instead. When being used, it should be called early in the main() function. Default log level
/// is set to `debug`.
///
/// # Panics
///
/// This function will panic if it is called more than once, or if another
/// library has already initialized a global logger.
pub fn init_builtin_logger() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Debug))
        .unwrap();
}

struct CmdLogger;
impl log::Log for CmdLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
