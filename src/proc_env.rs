use std::cell::RefCell;
use std::collections::HashMap;

thread_local!{
    pub static ENV_VARS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}
#[doc(hidden)]
pub struct Env {
    vars_saved: HashMap<String, String>,
}

impl Env {
    pub fn new() -> Self {
        Self {
            vars_saved: HashMap::new(),
        }
    }

    pub fn set_var(&mut self, key: String, value: String) {
        ENV_VARS.with(|vars| {
            if let Some(old_value) = vars.borrow().get(&key) {
                self.vars_saved.insert(key.clone(), old_value.to_owned());
            } else {
                self.vars_saved.insert(key.clone(), "".to_owned());
            }
            vars.borrow_mut().insert(key, value);
        });
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        for (key, value) in &self.vars_saved {
            if value != "" {
                ENV_VARS.with(|vars| {
                    vars.borrow_mut().insert(key.to_owned(), value.to_owned());
                });
            } else {
                ENV_VARS.with(|vars| {
                    vars.borrow_mut().remove(key);
                });
            }
        }
    }
}

#[macro_export]
macro_rules! proc_env_set {
    () => {};
    (&$env: expr) => {};
    (&$env: expr, $key:ident = $v:tt $($other:tt)*) => {
        $env.set_var(stringify!($key).to_string(), $v.to_string());
        proc_env_set!(&$env $($other)*);
    };
    ($key:ident = $v:tt $($other:tt)*) => {
        let mut _cmdlib_env = $crate::Env::new();
        _cmdlib_env.set_var(stringify!($key).to_string(), $v.to_string());
        proc_env_set!(&_cmdlib_env $($other)*);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_pwd_set() {
        {
            proc_env_set!(PWD = "/tmp", DEBUG = 1);
            ENV_VARS.with(|vars| {
                assert!(vars.borrow().get("PWD") == Some(&"/tmp".to_string()));
                assert!(vars.borrow().get("DEBUG") == Some(&"1".to_string()));
            });
        }
        ENV_VARS.with(|vars| {
            assert!(vars.borrow().get("PWD").is_none());
        });
    }
}
