use std::rc::Weak;

use crate::values::lists::List;
use fxhash::FxHashMap;
use weak_table::WeakKeyHashMap;

use crate::{rvals::Custom, values::functions::ByteCodeLambda, SteelVal};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FunctionArgs {
    function: SteelVal,
    arguments: Vec<SteelVal>,
}

#[derive(Clone, Debug)]
// For now this has... no capacity, and no eviction strategy
pub struct MemoizationTable {
    table: FxHashMap<FunctionArgs, SteelVal>,
}

impl MemoizationTable {
    pub fn new() -> Self {
        Self {
            table: FxHashMap::default(),
        }
    }

    pub fn insert(&mut self, function: SteelVal, arguments: Vec<SteelVal>, value: SteelVal) {
        self.table.insert(
            FunctionArgs {
                function,
                arguments,
            },
            value,
        );
    }

    pub fn get(&self, function: SteelVal, arguments: Vec<SteelVal>) -> Option<SteelVal> {
        self.table
            .get(&FunctionArgs {
                function,
                arguments,
            })
            .cloned()
    }
}

pub struct WeakMemoizationTable {
    table: WeakKeyHashMap<Weak<ByteCodeLambda>, FxHashMap<List<SteelVal>, SteelVal>>,
}

impl WeakMemoizationTable {
    pub fn new() -> Self {
        Self {
            table: WeakKeyHashMap::default(),
        }
    }

    pub fn insert(
        &mut self,
        function: SteelVal,
        arguments: List<SteelVal>,
        value: SteelVal,
    ) -> crate::rvals::Result<()> {
        // println!("Inserting args: {:?}", arguments);

        if let SteelVal::Closure(l) = function {
            if let Some(map) = self.table.get_mut(&l) {
                map.insert(arguments, value);
            } else {
                let mut map = FxHashMap::default();
                map.insert(arguments, value);

                self.table.insert(l.0, map);
            }
        } else {
            stop!(TypeMismatch => "memoization table expected a function, found: {:?}", function);
        }

        Ok(())
    }

    pub fn get(
        &self,
        function: SteelVal,
        arguments: List<SteelVal>,
    ) -> crate::rvals::Result<Option<SteelVal>> {
        if let SteelVal::Closure(l) = function {
            Ok(self.table.get(&l).and_then(|x| x.get(&arguments)).cloned())
        } else {
            stop!(TypeMismatch => "memoization table expected a function, found: {:?}", function);
        }
    }
}

impl Custom for WeakMemoizationTable {}
