// use crate::rvals::Result;
// #[macro_use]
use crate::{
    gc::Gc,
    primitives::{
        ListOperations, NumOperations, StringOperations, SymbolOperations, VectorOperations,
    },
    rerrs::{ErrorKind, SteelErr},
    rvals::{Result, SteelVal},
    stop,
};

use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Weak,
    sync::atomic::{AtomicUsize, Ordering},
};

// TODO
pub const fn _new_void() -> SteelVal {
    // VOID.with(Gc::clone)
    SteelVal::Void
}

// TODO
pub const fn _new_true() -> SteelVal {
    // TRUE.with(Gc::clone)
    SteelVal::BoolV(true)
}

// TODO
pub const fn _new_false() -> SteelVal {
    SteelVal::BoolV(false)
    // FALSE.with(Gc::clone)
}

pub static ENV_ID: AtomicUsize = AtomicUsize::new(0);

#[macro_use]
macro_rules! ensure_tonicity {
    ($check_fn:expr) => {{
        |args: &[SteelVal]| -> Result<SteelVal> {
            let mut args_iter = args.iter();
            let first = args_iter.next().ok_or(SteelErr::new(
                ErrorKind::ArityMismatch,
                "expected at least one argument".to_string(),
            ))?;
            fn f<'a>(prev: &SteelVal, mut xs: impl Iterator<Item = &'a SteelVal>) -> bool {
                match xs.next() {
                    Some(x) => $check_fn(prev, x) && f(x, xs),
                    None => true,
                }
            }
            let res = f(&first, args_iter);
            Ok(SteelVal::BoolV(res))
        }
    }};
}

// #[macro_use]
// macro_rules! ensure_tonicity_pointer_equality {
//     ($check_fn:expr) => {{
//         |args: &[SteelVal]| -> Result<SteelVal> {
//             // let args_iter: Vec<Rc<SteelVal>> = args.into_iter();
//             let mut args_iter = args.into_iter();
//             let first = args_iter.next().ok_or(SteelErr::ArityMismatch(
//                 "expected at least one argument".to_string(),
//                 None,
//             ))?;
//             fn f<'a>(prev: &SteelVal, mut xs: impl Iterator<Item = &'a SteelVal>) -> bool {
//                 match xs.next() {
//                     Some(x) => $check_fn(prev, x) && f(x, xs),
//                     None => true,
//                 }
//             };
//             let res = f(&first, args_iter);
//             Ok(SteelVal::BoolV(res))
//         }
//     }};
// }

#[macro_use]
macro_rules! gen_pred {
    ($variant:ident) => {{
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if let Some(first) = args.first() {
                if let SteelVal::$variant(..) = first {
                    return Ok(SteelVal::BoolV(true));
                }
            }
            Ok(SteelVal::BoolV(false))
        })
    }};

    ($variant1:ident, $variant2:ident) => {{
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if let Some(first) = args.first() {
                match first {
                    SteelVal::$variant1(..) | SteelVal::$variant2(..) => {
                        return Ok(SteelVal::BoolV(true));
                    }
                    _ => {}
                }
            }
            Ok(SteelVal::BoolV(false))
        })
    }};
}

// pub type RcRefCell<T> = Rc<RefCell<T>>;
// pub fn new_rc_ref_cell<T>(x: T) -> RcRefCell<T> {
//     Rc::new(RefCell::new(x))
// }

#[derive(Debug)]
pub struct Env {
    bindings_vec: Vec<SteelVal>,
    bindings_map: BTreeMap<usize, SteelVal>,
    offset: usize,
    sub_expression: Option<Weak<RefCell<Env>>>,
    is_binding_context: bool,
    is_binding_offset: bool,
    reachable: bool,
    id: usize,
}

impl Drop for Env {
    fn drop(&mut self) {
        // println!(
        //     "############### Dropping env with ID: {} #############",
        //     self.id
        // );
        self.bindings_map.clear();
    }
}

pub trait MacroEnv {
    fn validate_identifier(&self, name: &str) -> bool;
}

impl Env {
    /// Make a new `Env` from
    /// another parent `Env`.
    pub fn new(offset: usize) -> Self {
        Env {
            bindings_vec: Vec::new(),
            bindings_map: BTreeMap::default(),
            offset,
            sub_expression: None,
            is_binding_context: false,
            is_binding_offset: false,
            reachable: false,
            id: ENV_ID.fetch_add(1, Ordering::SeqCst),
        }
    }

    pub fn set_reachable(&mut self, b: bool) {
        self.reachable = b;
    }

    pub fn is_reachable(&self) -> bool {
        self.reachable
    }

    pub fn extract(&self, idx: usize) -> Option<SteelVal> {
        self.bindings_map.get(&idx).cloned()
    }

    pub fn len(&self) -> usize {
        self.bindings_vec.len()
    }

    pub fn local_offset(&self) -> usize {
        self.offset
    }

    pub fn new_subexpression(sub_expression: Weak<RefCell<Self>>, offset: usize) -> Self {
        Env {
            // bindings: HashMap::new(),
            bindings_vec: Vec::new(),
            // bindings_map: HashMap::default(),
            bindings_map: BTreeMap::default(),
            offset,
            // parent: None,
            sub_expression: Some(sub_expression),
            // weak_count: 0,
            // children: SmallVec::new(),
            // children: HashSet::new(),
            is_binding_context: false,
            is_binding_offset: false,
            // module: Vec::new(),
            // ndefs: 0,
            reachable: false,
            id: ENV_ID.fetch_add(1, Ordering::SeqCst),
        }
    }

    pub fn new_subexpression_with_capacity(
        sub_expression: Weak<RefCell<Self>>,
        offset: usize,
    ) -> Self {
        Env {
            // bindings: HashMap::new(),
            bindings_vec: Vec::new(),
            // bindings_map: HashMap::with_capacity_and_hasher(capacity, RandomState::new()),
            bindings_map: BTreeMap::default(),
            offset,
            // parent: None,
            sub_expression: Some(sub_expression),
            // weak_count: 0,
            // children: SmallVec::new(),
            // children: HashSet::new(),
            is_binding_context: false,
            is_binding_offset: false,
            // module: Vec::new(),
            // ndefs: 0,
            reachable: false,
            id: ENV_ID.fetch_add(1, Ordering::SeqCst),
        }
    }

    pub fn new_subexpression_with_capacity_without_offset(
        sub_expression: Weak<RefCell<Self>>,
    ) -> Self {
        Env {
            // bindings: HashMap::new(),
            bindings_vec: Vec::new(),
            // bindings_map: HashMap::with_capacity_and_hasher(capacity, RandomState::new()),
            bindings_map: BTreeMap::default(),
            offset: 0,
            // parent: None,
            sub_expression: Some(sub_expression),
            // weak_count: 0,
            // children: SmallVec::new(),
            // children: HashSet::new(),
            is_binding_context: false,
            is_binding_offset: false,
            // module: Vec::new(),
            // ndefs: 0,
            reachable: false,
            id: ENV_ID.fetch_add(1, Ordering::SeqCst),
        }
    }

    // pub fn set_ndefs(&mut self, ndefs: usize) {
    //     self.ndefs = ndefs
    // }

    pub fn is_binding_context(&self) -> bool {
        self.is_binding_context
    }

    pub fn set_binding_context(&mut self, b: bool) {
        self.is_binding_context = b;
    }

    pub fn is_binding_offset(&self) -> bool {
        self.is_binding_offset
    }

    pub fn set_binding_offset(&mut self, b: bool) {
        self.is_binding_offset = b;
    }

    pub fn is_root(&self) -> bool {
        self.sub_expression.is_none()
    }

    /// top level global env has no parent
    pub fn root() -> Self {
        Env {
            // bindings: HashMap::new(),
            bindings_vec: Vec::new(),
            // bindings_map: HashMap::default(),
            bindings_map: BTreeMap::default(),
            offset: 0,
            // parent: None,
            sub_expression: None,
            // weak_count: 0,
            // children: SmallVec::new(),
            is_binding_context: false,
            is_binding_offset: false,
            // module: Vec::new(),
            // ndefs: 0,
            reachable: true,
            id: ENV_ID.fetch_add(1, Ordering::SeqCst),
        }
    }

    pub fn sub_expression(&self) -> &Option<Weak<RefCell<Env>>> {
        &self.sub_expression
    }

    // pub fn bindings_map(&self) -> &HashMap<usize, SteelVal, RandomState> {
    //     &self.bindings_map
    // }

    pub fn bindings_map(&self) -> &BTreeMap<usize, SteelVal> {
        &self.bindings_map
    }

    pub fn string_bindings_vec(&self) -> String {
        // format!("{:?}", self.bindings_map)
        let mut vec_str = self
            .bindings_map
            .iter()
            .map(|x| format!("{:?}", x))
            .collect::<Vec<String>>();
        vec_str.sort();
        format!("{:?}", vec_str)
    }

    pub fn define_idx(&mut self, idx: usize, val: SteelVal) {
        // println!("Defining value: {:?} at idx: {}", val, idx);
        // println!("Current bindings: {:?}", self.bindings_vec);

        if idx < self.bindings_vec.len() {
            // let e = mem::replace(&mut self.bindings_vec[idx], val);
            // println!("Strong count: {}", Rc::strong_count(&e));
            // std::mem::drop(e);
            // println!("Strong count: {}", Rc::strong_count(&e));
            self.bindings_vec[idx] = val;
        } else if idx == self.bindings_vec.len() {
            self.bindings_vec.push(val);
        } else {
            // Add void padding so that this doesn't happen
            // This _should_ be unreachable outside of a repl
            // This allows for redefinitions and additions into scripts
            for _ in 0..(idx - self.bindings_vec.len()) {
                self.bindings_vec.push(SteelVal::Void)
            }

            self.bindings_vec.push(val);

            // println!("Length at panic: {}", self.bindings_vec.len());
            // println!("Attempting to define: {} @ {}", idx, val);
            // panic!("Out of bounds define statement");
        }
        // println!("{:?}", self.bindings_vec);
        // self.offset += 1;
    }

    pub fn reserve_defs(&mut self, ndefs: usize) {
        for _ in 0..ndefs {
            self.bindings_vec.push(SteelVal::Void);
        }
    }

    pub fn pop_last(&mut self) {
        self.bindings_vec.pop();
    }

    /// Search starting from the current environment
    /// for `idx`, looking through the parent chain in order.
    ///
    /// if found, return that value
    ///
    /// Otherwise, error with `FreeIdentifier`
    // #[inline]
    pub fn repl_lookup_idx(&self, idx: usize) -> Result<SteelVal> {
        // unimplemented!()
        // println!("{:?}", self.bindings.keys());
        if let Some(v) = self.bindings_map.get(&idx) {
            // value needs to be cloned because
            // user needs to be able to own a persistent value
            // from Cell that may be modified later
            Ok(v.clone())
        } else {
            match &self.sub_expression {
                Some(par) => match par.upgrade() {
                    Some(x) => x.borrow().repl_lookup_idx(idx),
                    None => {
                        println!("Bindings at quit: {:?}", self.bindings_map());
                        println!("Current Env ID: {}", self.id);
                        stop!(Generic => "Parent subexpression was dropped looking for {} repl_lookup_idx", idx)
                    }
                },
                None => {
                    println!(
                        "Keys at lookup: {:?}",
                        self.bindings_map.keys().collect::<Vec<&usize>>()
                    );
                    stop!(FreeIdentifier => idx);
                }
            }
        }
    }

    #[inline]
    pub fn repl_define_idx(&mut self, idx: usize, val: SteelVal) {
        self.bindings_map.insert(idx, val);
        // unimplemented!()
    }

    pub fn repl_set_idx(&mut self, idx: usize, val: SteelVal) -> Result<SteelVal> {
        if self.bindings_map.contains_key(&idx) {
            Ok(self.bindings_map.insert(idx, val).unwrap())
        } else {
            match &self.sub_expression {
                Some(par) => match par.upgrade() {
                    Some(x) => x.borrow_mut().repl_set_idx(idx, val),
                    None => {
                        stop!(Generic => "Parent subexpression was dropped looking for {}", idx)
                    }
                },
                None => stop!(FreeIdentifier => idx),
            }
        }
    }

    pub fn lookup_idx(&self, idx: usize) -> Result<SteelVal> {
        let offset = self.offset;
        // println!("Looking up {}, with offset: {}", idx, offset);
        // println!("{:?}", self.bindings_vec);

        // if offset != 0 {
        //     offset = offset - 1;
        // }

        // if self.sub_expression.is_some() {
        //     offset = offset + 1;
        // }

        if idx >= offset {
            let lookup = idx - offset;

            // let lookup = if self.sub_expression.is_some() {
            //     (self.bindings_vec.len() - 1) - (idx - offset + 1)
            // } else {
            //     idx - offset
            // };

            // let updated_offset = if offset != 0 { offset + 1 } else { offset };

            if let Some(v) = self.bindings_vec.get(lookup) {
                // println!("Found {:?}", v);
                Ok(v.clone())
            } else {
                // println!(
                //     "Looking up idx: {} with length {}",
                //     idx,
                //     self.bindings_vec.len()
                // );

                stop!(FreeIdentifier => "Internal Compiler Error - unable to find idx: {} with length: {}", lookup, self.bindings_vec.len());
            }
        // }
        // if let Some(v) = self.bindings_vec.get(idx - self.offset) {
        //     Ok(Rc::clone(v))
        } else {
            match &self.sub_expression {
                Some(par) => match par.upgrade() {
                    Some(x) => x.borrow().lookup_idx(idx),
                    None => {
                        stop!(Generic => "Parent subexpression was dropped looking for {}", idx)
                    }
                },
                None => stop!(FreeIdentifier => idx),
            }
        }
    }

    #[inline]
    pub fn add_root_value(&mut self, idx: usize, val: SteelVal) {
        self.bindings_map.insert(idx, val);
    }

    // bit of a quality of life hack
    pub fn constant_env_to_hashmap() -> SteelVal {
        let mut hm = crate::im_rc::HashMap::new();
        for (name, val) in Env::constant_core() {
            hm.insert(SteelVal::SymbolV(name.into()), val);
        }
        SteelVal::HashMapV(Gc::new(hm))
    }

    // TODO actually determine which functions are const
    pub fn constant_core() -> Vec<(&'static str, SteelVal)> {
        vec![
            ("+", NumOperations::adder()),
            ("i+", NumOperations::integer_add()),
            ("f+", NumOperations::float_add()),
            ("*", NumOperations::multiply()),
            ("/", NumOperations::divide()),
            ("-", NumOperations::subtract()),
            // ("list", ListOperations::list()),
            ("car", ListOperations::car()),
            ("cdr", ListOperations::cdr()),
            ("first", ListOperations::car()),
            ("rest", ListOperations::cdr()),
            // ("cons", ListOperations::cons()),
            // ("append", ListOperations::append()),
            // ("push-back", ListOperations::push_back()),
            ("range", ListOperations::range()),
            // ("list->vector", ListOperations::list_to_vec()),
            ("null?", VectorOperations::list_vec_null()),
            ("int?", gen_pred!(IntV)),
            ("float?", gen_pred!(NumV)),
            ("number?", gen_pred!(NumV, IntV)),
            ("string?", gen_pred!(StringV)),
            ("symbol?", gen_pred!(SymbolV)),
            ("vector?", gen_pred!(VectorV)),
            ("list?", gen_pred!(Pair)),
            ("integer?", gen_pred!(IntV)),
            ("boolean?", gen_pred!(BoolV)),
            ("function?", gen_pred!(Closure, FuncV)),
            ("=", SteelVal::FuncV(ensure_tonicity!(|a, b| a == b))),
            ("equal?", SteelVal::FuncV(ensure_tonicity!(|a, b| a == b))),
            // (
            //     "eq?",
            //     SteelVal::FuncV(ensure_tonicity_pointer_equality!(|a, b| Gc::ptr_eq(a, b))),
            // ),
            (">", SteelVal::FuncV(ensure_tonicity!(|a, b| a > b))),
            (">=", SteelVal::FuncV(ensure_tonicity!(|a, b| a >= b))),
            ("<", SteelVal::FuncV(ensure_tonicity!(|a, b| a < b))),
            ("<=", SteelVal::FuncV(ensure_tonicity!(|a, b| a <= b))),
            ("string-append", StringOperations::string_append()),
            ("string->list", StringOperations::string_to_list()),
            ("string-upcase", StringOperations::string_to_upper()),
            ("string-lowercase", StringOperations::string_to_lower()),
            ("trim", StringOperations::trim()),
            ("trim-start", StringOperations::trim_start()),
            ("trim-end", StringOperations::trim_end()),
            ("split-whitespace", StringOperations::split_whitespace()),
            ("void", SteelVal::Void),
            ("list->string", ListOperations::list_to_string()),
            ("concat-symbols", SymbolOperations::concat_symbols()),
            ("symbol->string", SymbolOperations::symbol_to_string()),
            ("string->int", StringOperations::string_to_int()),
            ("even?", NumOperations::even()),
            ("odd?", NumOperations::odd()),
        ]
    }
}

// // embed an async function into steel
// // lets... see how this goes
// fn test_function(_args: &[Gc<SteelVal>]) -> FutureResult {
//     FutureResult::new(Box::pin(async {
//         Ok(Gc::new(SteelVal::StringV("hello-world".to_string())))
//     }))
// }

// fn get(_args: &[Gc<SteelVal>]) -> FutureResult {
//     FutureResult::new(Box::pin(async {
//         // let resp = reqwest::get("https://httpbin.org/ip")
//         //     .await
//         //     .unwrap()
//         //     .json::<HashMap<String, String>>()
//         //     .await
//         //     .unwrap();
//         // println!("{:#?}", resp);

//         // println!("Status: {}", res.status());
//         // println!("Headers:\n{:#?}", res.headers());

//         let res = reqwest::get("http://httpbin.org/get").await.unwrap();

//         let body = res.text().await.unwrap();
//         println!("Body:\n{}", body);

//         Ok(Gc::new(SteelVal::StringV(body)))
//     }))
// }

#[cfg(test)]
mod env_tests {
    use super::*;
    fn _unwrap_single_float(exp: &SteelVal) -> Result<f64> {
        match exp {
            SteelVal::NumV(num) => Ok(*num),
            _ => stop!(TypeMismatch => "expected a number"),
        }
    }
    // #[test]
    // fn env_basic() {
    //     // default_env <- c1 <- c2
    //     let default_env = Rc::new(RefCell::new(Env::default_env()));
    //     assert!(default_env.borrow().lookup("+").is_ok());
    //     let c1 = Rc::new(RefCell::new(Env::new(&default_env, 0)));
    //     c1.borrow_mut()
    //         .define("x".to_owned(), Gc::new(SteelVal::NumV(1.0)));
    //     let c2 = Rc::new(RefCell::new(Env::new(&c1, 0)));
    //     c2.borrow_mut()
    //         .define("y".to_owned(), Gc::new(SteelVal::NumV(2.0)));
    //     assert!(default_env.borrow_mut().lookup("+").is_ok());
    //     assert!(c2.borrow_mut().lookup("+").is_ok());
    //     assert_eq!(
    //         unwrap_single_float(&c2.borrow_mut().lookup("y").unwrap()).unwrap(),
    //         2.0
    //     );
    //     assert_eq!(
    //         unwrap_single_float(&c2.borrow_mut().lookup("x").unwrap()).unwrap(),
    //         1.0
    //     );
    //     assert!(c2.borrow_mut().lookup("z").is_err());
    // }
}
