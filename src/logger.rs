use log::{Level, LevelFilter, Metadata, Record};

static LOGGER: CmdLogger = CmdLogger;

pub fn init_builtin_log() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
        .unwrap();
}

struct CmdLogger;
impl log::Log for CmdLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("{}", record.args());
        }
    }

    fn flush(&self) {}
}
