use crate::env::{FALSE, TRUE};
use crate::gc::Gc;
use crate::rerrs::SteelErr;
use crate::rvals::SteelVal::*;
use crate::rvals::{Result, SteelVal};
use crate::stop;
use im_rc::HashMap;

pub struct HashMapOperations {}

impl HashMapOperations {
    pub fn hm_construct() -> SteelVal {
        SteelVal::FuncV(|args: &[Gc<SteelVal>]| -> Result<Gc<SteelVal>> {
            let mut hm = HashMap::new();

            let mut arg_iter = args.into_iter().map(Gc::clone);

            loop {
                match (arg_iter.next(), arg_iter.next()) {
                    (Some(key), Some(value)) => {
                        hm.insert(key, value);
                    }
                    (None, None) => break,
                    _ => {
                        stop!(ArityMismatch => "hash map must have a value for every key!");
                    }
                }
            }

            Ok(Gc::new(SteelVal::HashMapV(hm)))
        })
    }

    pub fn hm_insert() -> SteelVal {
        SteelVal::FuncV(|args: &[Gc<SteelVal>]| -> Result<Gc<SteelVal>> {
            if args.len() != 3 {
                stop!(ArityMismatch => "hm insert takes 3 arguments")
            }

            let hashmap = Gc::clone(&args[0]);
            let key = Gc::clone(&args[1]);
            let value = Gc::clone(&args[2]);

            if let SteelVal::HashMapV(hm) = hashmap.as_ref() {
                let mut hm = hm.clone();
                hm.insert(key, value);
                Ok(Gc::new(SteelVal::HashMapV(hm)))
            } else {
                stop!(TypeMismatch => "hm insert takes a hashmap")
            }
        })
    }

    pub fn hm_get() -> SteelVal {
        SteelVal::FuncV(|args: &[Gc<SteelVal>]| -> Result<Gc<SteelVal>> {
            if args.len() != 2 {
                stop!(ArityMismatch => "hm get takes 2 arguments")
            }

            let hashmap = Gc::clone(&args[0]);
            let key = &args[1];

            if let SteelVal::HashMapV(hm) = hashmap.as_ref() {
                match hm.get(key) {
                    Some(v) => Ok(Gc::clone(v)),
                    None => stop!(Generic => "hash map key not found!"),
                }
            } else {
                stop!(TypeMismatch => "hm insert takes a hashmap")
            }
        })
    }
}
