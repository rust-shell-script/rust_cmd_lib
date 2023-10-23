use env_logger::Env;

pub fn try_init_default_logger() {
    let _ = env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_target(false)
        .format_timestamp(None)
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
        $crate::inner_log::debug!($($arg)*);
    }}
}

#[doc(hidden)]
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {{
        $crate::try_init_default_logger();
        $crate::inner_log::trace!($($arg)*);
    }}
}
