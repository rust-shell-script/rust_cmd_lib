#[macro_export]
macro_rules! proc_var {
    ($vis:vis $var:ident, $t:ty, $($var_init:tt)*) => {
        thread_local!{
            $vis static $var: std::cell::RefCell<$t> =
                std::cell::RefCell::new($($var_init)*);
        }
    };
}

#[macro_export]
macro_rules! proc_var_get {
    ($var:ident) => {
        $var.with(|var| var.borrow().clone())
    };
}

#[macro_export]
macro_rules! proc_var_set {
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
        proc_var!(LEN, u32, 100);
        proc_var_set!(LEN, |x| *x = 300);
        assert_eq!(proc_var_get!(LEN), 300);
    }

    #[test]
    fn test_proc_var_map() {
        use std::collections::HashMap;
        proc_var!(MAP, HashMap<String, String>, HashMap::new());
        proc_var_set!(MAP, |x| x.insert("a".to_string(), "b".to_string()));
        assert_eq!(proc_var_get!(MAP)["a"], "b".to_string());
    }

    #[test]
    fn test_proc_var_vec() {
        proc_var!(V, Vec<i32>, vec![]);
        proc_var_set!(V, |v| v.push(100));
        proc_var_set!(V, |v| v.push(200));
        assert_eq!(proc_var_get!(V)[0], 100);
    }
}
