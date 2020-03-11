use crate::evaluator::Result;
// #[macro_use]
use crate::primitives::{Adder, Divider, Multiplier, SteelFunctor, Subtractor};
use crate::rerrs::SteelErr;
use crate::rvals::SteelVal;
use crate::rvals::SteelVal::*;
use crate::stop;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[macro_use]
macro_rules! ensure_tonicity {
    ($check_fn:expr) => {{
        |args: Vec<SteelVal>| -> Result<SteelVal> {
            // let floats = unwrap_list_of_floats(args)?;
            let mut args_iter = args.iter();
            let first = args_iter.next().ok_or(SteelErr::ContractViolation(
                "expected at least one argument".to_string(),
            ))?;
            // let rest = &floats[1..];
            fn f<'a>(prev: &SteelVal, mut xs: impl Iterator<Item = &'a SteelVal>) -> bool {
                match xs.next() {
                    Some(x) => $check_fn(prev, x) && f(x, xs),
                    None => true,
                }
            };
            Ok(SteelVal::BoolV(f(first, args_iter)))
        }
    }};
}

pub type RcRefCell<T> = Rc<RefCell<T>>;
pub fn new_rc_ref_cell<T>(x: T) -> RcRefCell<T> {
    Rc::new(RefCell::new(x))
}

pub struct Env {
    bindings: HashMap<String, SteelVal>,
    parent: Option<Rc<RefCell<Env>>>,
}
impl Env {
    /// Make a new `Env` from
    /// another parent `Env`.
    pub fn new(parent: &Rc<RefCell<Self>>) -> Self {
        Env {
            bindings: HashMap::new(),
            parent: Some(Rc::clone(&parent)),
        }
    }

    /// top level global env has no parent
    pub fn root() -> Self {
        Env {
            bindings: HashMap::new(),
            parent: None,
        }
    }

    pub fn clear_bindings(&mut self) {
        self.bindings.clear();
    }

    /// Within the current environment, bind
    /// identifier `key` to `val`
    pub fn define(&mut self, key: String, val: SteelVal) {
        self.bindings.insert(key, val);
    }
    /// Within the current environment,
    /// bind identifiers `keys` to `vals`
    /// throws arity mismatch if they don't have the same length
    pub fn define_all(&mut self, keys: &[String], vals: Vec<SteelVal>) -> Result<()> {
        let expected_len = keys.len();
        let actual_len = vals.len();
        if expected_len != actual_len {
            let e = format!(
                "function expected {} params, got {}",
                expected_len, actual_len
            );
            stop!(ArityMismatch => e);
        }
        let iter = keys.iter().map(String::as_ref).zip(vals.into_iter());
        self.define_zipped(iter);
        Ok(())
    }

    pub fn define_zipped<'a>(&mut self, zipped: impl Iterator<Item = (&'a str, SteelVal)>) {
        zipped.for_each(|(param, arg)| self.define(param.to_string(), arg))
    }

    /// Search starting from the current environment
    /// for `key`, looking through the parent chain in order.
    ///
    /// If found, update binding for
    /// `key` with `val` and return old value.
    ///
    /// Otherwise, error with `FreeIdentifier`
    pub fn set(&mut self, key: String, val: SteelVal) -> Result<SteelVal> {
        if self.bindings.contains_key(&key) {
            self.bindings
                .insert(key.clone(), val)
                .ok_or_else(|| SteelErr::FreeIdentifier(key.to_string()))
        } else {
            match &self.parent {
                Some(par) => par.borrow_mut().set(key, val),
                None => stop!(FreeIdentifier => key), // Err(SteelErr::FreeIdentifier(key)),
            }
        }
    }

    /// Search starting from the current environment
    /// for `key`, looking through the parent chain in order.
    ///
    /// If found, remove the binding and return the value
    ///
    /// Otherwise, error with `FreeIdentifier`
    pub fn remove(&mut self, key: &str) -> Result<SteelVal> {
        if self.bindings.contains_key(key) {
            self.bindings
                .remove(key)
                .ok_or_else(|| SteelErr::FreeIdentifier(key.to_string()))
        } else {
            match &self.parent {
                Some(par) => par.borrow_mut().remove(key),
                None => stop!(FreeIdentifier => key), // Err(SteelErr::FreeIdentifier(key.to_string())),
            }
        }
    }

    /// Search starting from the current environment
    /// for `key`, looking through the parent chain in order.
    ///
    /// if found, return that value
    ///
    /// Otherwise, error with `FreeIdentifier`
    pub fn lookup(&self, name: &str) -> Result<SteelVal> {
        if self.bindings.contains_key(name) {
            // value needs to be cloned because
            // user needs to be able to own a persistent value
            // from Cell that may be modified later
            Ok(self.bindings[name].clone())
        } else {
            match &self.parent {
                Some(par) => par.borrow().lookup(name),
                None => stop!(FreeIdentifier => name), // Err(SteelErr::FreeIdentifier(name.to_string())),
            }
        }
    }
    /// default environment contains bindings for
    /// implementations of constants and things like
    /// `car`, `cdr`, `+`
    pub fn default_env() -> Env {
        let mut env = Env::root();
        env.define_zipped(Env::default_bindings().into_iter());
        env
    }
    pub fn default_bindings() -> Vec<(&'static str, SteelVal)> {
        vec![
            ("+", SteelVal::FuncV(Adder::new_func())),
            ("*", SteelVal::FuncV(Multiplier::new_func())),
            ("/", SteelVal::FuncV(Divider::new_func())),
            ("-", SteelVal::FuncV(Subtractor::new_func())),
            (
                "list",
                SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal> {
                    Ok(SteelVal::ListV(args))
                }),
            ),
            (
                "cons",
                SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal> {
                    let mut args = args.into_iter();
                    match (args.next(), args.next()) {
                        (Some(elem), Some(lst)) => {
                            if let SteelVal::ListV(mut l) = lst {
                                l.insert(0, elem);
                                Ok(SteelVal::ListV(l))
                            } else {
                                Ok(SteelVal::ListV(vec![elem, lst]))
                            }
                        }
                        _ => stop!(ArityMismatch => "cons takes two arguments"),
                    }
                }),
            ),
            (
                "null?",
                SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal> {
                    if args.len() == 1 {
                        match &args[0] {
                            SteelVal::ListV(v) => {
                                if v.is_empty() {
                                    Ok(SteelVal::BoolV(true))
                                } else {
                                    Ok(SteelVal::BoolV(false))
                                }
                            }
                            _ => Ok(SteelVal::BoolV(false)),
                        }
                    } else {
                        stop!(ArityMismatch => "car takes one argument");
                    }
                }),
            ),
            (
                "append",
                SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal> {
                    let lsts: Vec<SteelVal> =
                        unwrap_list_of_lists(args)?.into_iter().flatten().collect();
                    Ok(SteelVal::ListV(lsts))
                }),
            ),
            (
                "car",
                SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal> {
                    if let Some(first) = args.into_iter().next() {
                        match first {
                            SteelVal::ListV(e) => match e.into_iter().next() {
                                Some(e) => Ok(e),
                                None => stop!(ContractViolation => "car expects a non empty list"),
                            },
                            e => {
                                stop!(TypeMismatch => "car takes a list, given: {}", e);
                            }
                        }
                    } else {
                        stop!(ArityMismatch => "car takes one argument");
                    }
                }),
            ),
            (
                "cdr",
                SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal> {
                    if let Some(first) = args.into_iter().next() {
                        match first {
                            SteelVal::ListV(e) => {
                                if !e.is_empty() {
                                    Ok(SteelVal::ListV(e.into_iter().skip(1).collect()))
                                } else {
                                    stop!(ContractViolation => "cdr expects a non empty list")
                                }
                            }
                            e => {
                                stop!(TypeMismatch => "cdr takes a list, given: {}", e);
                            }
                        }
                    } else {
                        stop!(ArityMismatch => "cdr takes one argument");
                    }
                }),
            ),
            (
                "number?",
                SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal> {
                    match args.first() {
                        Some(NumV(_)) => Ok(BoolV(true)),
                        _ => Ok(BoolV(false)),
                    }
                }),
            ),
            (
                "string?",
                SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal> {
                    match args.first() {
                        Some(StringV(_)) => Ok(BoolV(true)),
                        _ => Ok(BoolV(false)),
                    }
                }),
            ),
            (
                "symbol?",
                SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal> {
                    match args.first() {
                        Some(SymbolV(_)) => Ok(BoolV(true)),
                        _ => Ok(BoolV(false)),
                    }
                }),
            ),
            (
                "list?",
                SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal> {
                    match args.first() {
                        Some(ListV(_)) => Ok(BoolV(true)),
                        _ => Ok(BoolV(false)),
                    }
                }),
            ),
            ("=", SteelVal::FuncV(ensure_tonicity!(|a, b| a == b))),
            ("equal?", SteelVal::FuncV(ensure_tonicity!(|a, b| a == b))),
            (">", SteelVal::FuncV(ensure_tonicity!(|a, b| a > b))),
            (">=", SteelVal::FuncV(ensure_tonicity!(|a, b| a >= b))),
            ("<", SteelVal::FuncV(ensure_tonicity!(|a, b| a < b))),
            ("<=", SteelVal::FuncV(ensure_tonicity!(|a, b| a <= b))),
        ]
    }
}

fn unwrap_list_of_lists(args: Vec<SteelVal>) -> Result<Vec<Vec<SteelVal>>> {
    args.iter().map(|x| unwrap_single_list(x)).collect()
}

fn unwrap_single_list(exp: &SteelVal) -> Result<Vec<SteelVal>> {
    match exp {
        SteelVal::ListV(lst) => Ok(lst.clone()),
        _ => stop!(TypeMismatch => "expected a list"),
    }
}

#[cfg(test)]
mod env_tests {
    use super::*;
    fn unwrap_single_float(exp: &SteelVal) -> Result<f64> {
        match exp {
            SteelVal::NumV(num) => Ok(*num),
            _ => stop!(TypeMismatch => "expected a number"),
        }
    }
    #[test]
    fn env_basic() {
        // default_env <- c1 <- c2
        let default_env = Rc::new(RefCell::new(Env::default_env()));
        assert!(default_env.borrow().lookup("+").is_ok());
        let c1 = Rc::new(RefCell::new(Env::new(&default_env)));
        c1.borrow_mut().define("x".to_owned(), SteelVal::NumV(1.0));
        let c2 = Rc::new(RefCell::new(Env::new(&c1)));
        c2.borrow_mut().define("y".to_owned(), SteelVal::NumV(2.0));
        assert!(default_env.borrow_mut().lookup("+").is_ok());
        assert!(c2.borrow_mut().lookup("+").is_ok());
        assert_eq!(
            unwrap_single_float(&c2.borrow_mut().lookup("y").unwrap()).unwrap(),
            2.0
        );
        assert_eq!(
            unwrap_single_float(&c2.borrow_mut().lookup("x").unwrap()).unwrap(),
            1.0
        );
        assert!(c2.borrow_mut().lookup("z").is_err());
    }
}
