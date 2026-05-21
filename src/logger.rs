use std::io::{IsTerminal, Write};

use env_logger::Env;

pub fn try_init_default_logger() {
    let _ = env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_target(false)
        .format_timestamp(None)
        .format(|buf, record| {
            // Use CRLF when writing to a terminal. A child process such as
            // `sudo` (with `use_pty`) may put the controlling terminal into
            // raw mode while it runs, leaving ONLCR disabled — so a bare LF
            // from the stderr-relay thread would not return the cursor to
            // column 0 and subsequent log lines would cascade to the right.
            let eol = if std::io::stderr().is_terminal() {
                "\r\n"
            } else {
                "\n"
            };
            let level_style = buf.default_level_style(record.level());
            write!(
                buf,
                "[{level_style}{:<5}{level_style:#}] {}{eol}",
                record.level(),
                record.args(),
            )
        })
        .try_init();
}

#[doc(hidden)]
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        $crate::try_init_default_logger();
        $crate::inner_log::error!($($arg)*);
    }}
}

#[doc(hidden)]
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        $crate::try_init_default_logger();
        $crate::inner_log::warn!($($arg)*);
    }}
}

#[doc(hidden)]
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        $crate::try_init_default_logger();
        $crate::inner_log::info!($($arg)*);
    }}
}

#[doc(hidden)]
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
        $crate::try_init_default_logger();
        #[cfg(feature = "build-print")]
        $crate::inner_log::info!($($arg)*);
        #[cfg(not(feature = "build-print"))]
        $crate::inner_log::debug!($($arg)*);
    }}
}

#[doc(hidden)]
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {{
        $crate::try_init_default_logger();
        #[cfg(feature = "build-print")]
        $crate::inner_log::info!($($arg)*);
        #[cfg(not(feature = "build-print"))]
        $crate::inner_log::trace!($($arg)*);
    }}
}
