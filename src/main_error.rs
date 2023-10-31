use std::error::Error;
use std::fmt::{self, Debug, Display};

// Modified from https://github.com/danleh/main_error
pub struct MainError(Box<dyn Error>);

impl<E: Into<Box<dyn Error>>> From<E> for MainError {
    fn from(e: E) -> Self {
        MainError(e.into())
    }
}

impl Display for MainError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

// Impl Debug (to satisfy trait bound for main()-Result error reporting), but use Display of wrapped
// error internally (for nicer output).
impl Debug for MainError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

/// Convenient type as a shorthand return type for `main()`.
pub type MainResult = Result<(), MainError>;
