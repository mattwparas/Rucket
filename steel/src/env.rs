use crate::rvals::{Result, SteelVal};

// TODO
pub const fn _new_void() -> SteelVal {
    SteelVal::Void
}

// TODO
pub const fn _new_true() -> SteelVal {
    SteelVal::BoolV(true)
}

// TODO
pub const fn _new_false() -> SteelVal {
    SteelVal::BoolV(false)
}

#[derive(Debug)]
pub struct Env {
    pub(crate) bindings_vec: Vec<SteelVal>,
}

pub trait MacroEnv {
    fn validate_identifier(&self, name: &str) -> bool;
}

impl Env {
    pub fn extract(&self, idx: usize) -> Option<SteelVal> {
        self.bindings_vec.get(idx).cloned()
    }

    /// top level global env has no parent
    pub fn root() -> Self {
        Env {
            bindings_vec: Vec::new(),
        }
    }

    /// Search starting from the current environment
    /// for `idx`, looking through the parent chain in order.
    ///
    /// if found, return that value
    ///
    /// Otherwise, error with `FreeIdentifier`
    // #[inline]
    pub fn repl_lookup_idx(&self, idx: usize) -> Result<SteelVal> {
        Ok(self.bindings_vec[idx].clone())
    }

    #[inline]
    pub fn repl_define_idx(&mut self, idx: usize, val: SteelVal) {
        // self.bindings_map.insert(idx, val);
        // unimplemented!()
        if idx < self.bindings_vec.len() {
            self.bindings_vec[idx] = val;
        } else {
            // println!("Index: {}, length: {}", idx, self.bindings_vec.len());
            self.bindings_vec.push(val);
            assert_eq!(self.bindings_vec.len() - 1, idx);
        }
    }

    pub fn repl_set_idx(&mut self, idx: usize, val: SteelVal) -> Result<SteelVal> {
        let output = self.bindings_vec[idx].clone();
        self.bindings_vec[idx] = val;
        Ok(output)
    }

    #[inline]
    pub fn add_root_value(&mut self, idx: usize, val: SteelVal) {
        // self.bindings_map.insert(idx, val);
        self.repl_define_idx(idx, val);
    }
}
