#[macro_export]
macro_rules! tls_init {
    ($vis:vis $var:ident, $t:ty, $($var_init:tt)*) => {
        thread_local!{
            $vis static $var: std::cell::RefCell<$t> =
                std::cell::RefCell::new($($var_init)*);
        }
    };
}

#[macro_export]
macro_rules! tls_get {
    ($var:ident) => {
        $var.with(|var| var.borrow().clone())
    };
}

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
