/// Declare a new thread local storage variable
/// ```
/// # use cmd_lib::*;
/// use std::collections::HashMap;
/// tls_init!(LEN, u32, 100);
/// tls_init!(MAP, HashMap<String, String>, HashMap::new());
/// ```
#[macro_export]
macro_rules! tls_init {
    ($vis:vis $var:ident, $t:ty, $($var_init:tt)*) => {
        thread_local!{
            $vis static $var: std::cell::RefCell<$t> =
                std::cell::RefCell::new($($var_init)*);
        }
    };
}

/// Get the value of a thread local storage variable
///
/// ```
/// # use cmd_lib::*;
/// // from examples/tetris.rs:
/// tls_init!(screen_buffer, String, "".to_string());
/// eprint!("{}", tls_get!(screen_buffer));
///
/// tls_init!(use_color, bool, true); // true if we use color, false if not
/// if tls_get!(use_color) {
///     // ...
/// }
/// ```
#[macro_export]
macro_rules! tls_get {
    ($var:ident) => {
        $var.with(|var| var.borrow().clone())
    };
}

/// Set the value of a thread local storage variable
/// ```
/// # use cmd_lib::*;
/// # let changes = "";
/// tls_init!(screen_buffer, String, "".to_string());
/// tls_set!(screen_buffer, |s| s.push_str(changes));
///
/// tls_init!(use_color, bool, true); // true if we use color, false if not
/// fn toggle_color() {
///     tls_set!(use_color, |x| *x = !*x);
///     // redraw_screen();
/// }
/// ```
#[macro_export]
macro_rules! tls_set {
    ($var:ident, |$v:ident| $($var_update:tt)*) => {
        $var.with(|$v| {
                let mut $v = $v.borrow_mut();
                $($var_update)*;
        });
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_proc_var_u32() {
        tls_init!(LEN, u32, 100);
        tls_set!(LEN, |x| *x = 300);
        assert_eq!(tls_get!(LEN), 300);
    }

    #[test]
    fn test_proc_var_map() {
        use std::collections::HashMap;
        tls_init!(MAP, HashMap<String, String>, HashMap::new());
        tls_set!(MAP, |x| x.insert("a".to_string(), "b".to_string()));
        assert_eq!(tls_get!(MAP)["a"], "b".to_string());
    }

    #[test]
    fn test_proc_var_vec() {
        tls_init!(V, Vec<i32>, vec![]);
        tls_set!(V, |v| v.push(100));
        tls_set!(V, |v| v.push(200));
        assert_eq!(tls_get!(V)[0], 100);
    }
}
