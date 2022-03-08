use crate::rvals::{Result, SteelVal};

use crate::parser::{
    ast::ExprKind,
    parser::{ParseError, Parser},
};

use std::convert::TryFrom;

// TODO add the serializing and deserializing for constants
// use serde::{Deserialize, Serialize};

use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub struct ConstantMap(Vec<SteelVal>);

// #[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
// struct ConstantExprMap {
//     map: Vec<Expr>,
// }

impl ConstantMap {
    pub fn new() -> ConstantMap {
        ConstantMap(Vec::new())
    }

    pub fn from_vec(vec: Vec<SteelVal>) -> ConstantMap {
        ConstantMap(vec)
    }

    fn to_constant_expr_map(&self) -> Vec<String> {
        let result: std::result::Result<Vec<_>, _> =
            self.0.iter().map(|x| ExprKind::try_from(x)).collect();

        result.unwrap().into_iter().map(|x| x.to_string()).collect()
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        // let vector_val = self.to_constant_expr_map()?;

        let str_vector = self.to_constant_expr_map();
        let result = bincode::serialize(&str_vector);

        Ok(result.unwrap())
    }

    pub fn from_bytes(encoded: &[u8]) -> Result<ConstantMap> {
        let str_vector: Vec<String> = bincode::deserialize(encoded).unwrap();

        // the interner needs to be fixed but for now it just is here for legacy reasons
        // it currently does no allocation
        let mut intern = HashMap::new();

        str_vector
            .into_iter()
            .map(|x| {
                // Parse the input
                let parsed: std::result::Result<Vec<ExprKind>, ParseError> =
                    Parser::new(&x, &mut intern).collect();
                let parsed = parsed?;

                Ok(SteelVal::try_from(parsed[0].clone()).unwrap())
            })
            .collect::<Result<Vec<_>>>()
            .map(ConstantMap)
    }

    // pub fn from_bytes(encoded: &[u8]) -> ConstantMap {
    //     bincode::deserialize(encoded).unwrap()
    // }
}

impl ConstantTable for ConstantMap {
    fn add(&mut self, val: SteelVal) -> usize {
        let idx = self.0.len();
        self.0.push(val);
        idx
    }

    // Fallible
    fn get(&self, idx: usize) -> SteelVal {
        self.0[idx].clone()
    }

    fn try_get(&self, idx: usize) -> Option<SteelVal> {
        self.0.get(idx).cloned()
    }

    fn add_or_get(&mut self, val: SteelVal) -> usize {
        // unimplemented!()
        if let Some(idx) = self.0.iter().position(|x| x == &val) {
            idx
        } else {
            self.add(val)
        }
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn roll_back(&mut self, idx: usize) {
        self.0.truncate(idx);
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.0.clear()
    }
}

pub trait ConstantTable {
    fn add(&mut self, val: SteelVal) -> usize;
    fn get(&self, idx: usize) -> SteelVal;
    fn try_get(&self, idx: usize) -> Option<SteelVal>;
    fn add_or_get(&mut self, val: SteelVal) -> usize;
    fn len(&self) -> usize;
    fn roll_back(&mut self, idx: usize);
    fn is_empty(&self) -> bool;

    #[cfg(test)]
    fn clear(&mut self);
}

#[cfg(test)]
pub mod constant_table_tests {
    use super::*;

    #[test]
    fn run_tests_constant_map() {
        let mut instance = ConstantMap::new();
        test_add(&mut instance);

        let mut instance = ConstantMap::new();
        test_get(&mut instance);
    }

    fn test_add<CT: ConstantTable>(instance: &mut CT) {
        assert_eq!(instance.len(), 0);
        let val1 = SteelVal::BoolV(true);
        let val2 = SteelVal::BoolV(false);
        assert_eq!(instance.add(val1), 0);
        assert_eq!(instance.add(val2), 1);
    }

    fn test_get<CT: ConstantTable>(instance: &mut CT) {
        assert_eq!(instance.len(), 0);
        let val1 = SteelVal::BoolV(true);
        let val2 = SteelVal::BoolV(false);
        assert_eq!(instance.add(val1), 0);
        assert_eq!(instance.add(val2), 1);

        assert_eq!(instance.get(0), SteelVal::BoolV(true));
        assert_eq!(instance.get(1), SteelVal::BoolV(false));
    }
}
