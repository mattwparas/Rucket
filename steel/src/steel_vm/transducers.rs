use im_lists::list::List;

// use super::{evaluation_progress::EvaluationProgress, stack::StackFrame, vm::VmCore};
use super::{
    lazy_stream::LazyStreamIter,
    options::{ApplyContracts, UseCallbacks},
    vm::{VmContext, VmCore},
};
use crate::{
    compiler::constants::ConstantTable,
    parser::span::Span,
    primitives::VectorOperations,
    rerrs::{ErrorKind, SteelErr},
    rvals::{Result, SteelVal},
    stop,
    values::transducers::{CollectionType, Transducers},
};

use std::cell::RefCell;
use std::rc::Rc;

/// Generates the take transducer - wrapper around the take iterator
macro_rules! generate_take {
    ($iter:expr, $num:expr, $cur_inst_span:expr) => {
        if let SteelVal::IntV(num) = $num {
            if *num < 0 {
                stop!(ContractViolation => "take transducer must have a position number"; *$cur_inst_span)
            }
            Box::new($iter.take(*num as usize))
        } else {
            stop!(TypeMismatch => "take transducer takes an integer"; *$cur_inst_span)
        }
    }
}

/// Generates the drop transducer - wrapper around the drop iterator
macro_rules! generate_drop {
    ($iter:expr, $num:expr, $cur_inst_span:expr) => {
        if let SteelVal::IntV(num) = $num {
            if *num < 0 {
                stop!(ContractViolation => "drop transducer must have a position number"; *$cur_inst_span)
            }
            Box::new($iter.skip(*num as usize))
        } else {
            stop!(TypeMismatch => "drop transducer takes an integer"; *$cur_inst_span)
        }
    }
}

pub(crate) const TRANSDUCE: SteelVal = SteelVal::BuiltIn(transduce);
pub(crate) const EXECUTE: SteelVal = SteelVal::BuiltIn(execute);

// Transduce - reducer has intitial value?
// Change this so -> first value is sequence
// sequence argument is a transducer or a sequence of transducers
// last argument must be a reducer
fn transduce(args: Vec<SteelVal>, ctx: &mut dyn VmContext) -> Result<SteelVal> {
    let mut arg_iter = args.into_iter();
    let transducer = arg_iter.next().unwrap();
    let reducer = arg_iter.next().unwrap();
    let initial_value = arg_iter.next().unwrap();
    let list = arg_iter.next().unwrap();

    if let SteelVal::IterV(transducer) = &transducer {
        ctx.call_transduce(&transducer.ops, list, initial_value, reducer)
    } else {
        stop!(Generic => format!("transduce must take a transducer, found: {}", transducer));
    }
}

// Execute and transduce should be able to be merged into one function
// Then based on the args we can determine which kind it is
fn execute(args: Vec<SteelVal>, ctx: &mut dyn VmContext) -> Result<SteelVal> {
    let mut arg_iter = args.into_iter();

    let transducer = arg_iter.next().unwrap();
    let list = arg_iter.next().unwrap();
    let output_type = arg_iter.next();

    if let SteelVal::IterV(transducer) = &transducer {
        ctx.call_execute(&transducer.ops, list, output_type)
    } else {
        stop!(Generic => format!("Transducer execute must take a transducer, found: {}", transducer));
    }
}

impl<'global, 'a, CT: ConstantTable, U: UseCallbacks, A: ApplyContracts> VmCore<'a, CT, U, A> {
    // With transducers, we also need reducers
    // reducers should define _how_ a value is going to be converted away
    // from the iterator stream
    // This could either be a function that returns a single value, or a generic collection type
    pub(crate) fn res_iterator(
        value: &'global SteelVal,
        vm_ctx: Rc<RefCell<&'global mut Self>>,
        cur_inst_span: &'global Span,
    ) -> Result<Box<dyn Iterator<Item = Result<SteelVal>> + 'global>> {
        match value {
            SteelVal::VectorV(v) => Ok(Box::new(v.iter().cloned().map(Ok))),
            SteelVal::StreamV(lazy_stream) => Ok(Box::new(LazyStreamIter::new(
                lazy_stream.unwrap(),
                vm_ctx,
                cur_inst_span,
            ))),
            SteelVal::StringV(s) => Ok(Box::new(s.chars().map(|x| Ok(SteelVal::CharV(x))))),
            SteelVal::ListV(l) => Ok(Box::new(l.iter().cloned().map(Ok))),
            SteelVal::StructV(s) => Ok(Box::new(s.iter().cloned().map(Ok))),
            SteelVal::HashSetV(hs) => Ok(Box::new(hs.iter().cloned().map(Ok))),
            SteelVal::HashMapV(hm) => {
                Ok(Box::new(hm.iter().map(|x| {
                    Ok(SteelVal::ListV(im_lists::list![x.0.clone(), x.1.clone()]))
                })))
            }
            _ => {
                stop!(TypeMismatch => format!("value unable to be converted to an iterable: {}", value))
            }
        }
    }

    pub(crate) fn run(
        &mut self,
        ops: &[Transducers],
        root: SteelVal,
        collection_type: Option<SteelVal>,
        cur_inst_span: &Span,
    ) -> Result<SteelVal> {
        // By default, match the output type to the input type
        let output_type = match root {
            SteelVal::VectorV(_) => CollectionType::Vector,
            _ => CollectionType::List,
        };

        let vm = Rc::new(RefCell::new(self));

        let mut iter = Self::res_iterator(&root, Rc::clone(&vm), cur_inst_span)?;

        for t in ops {
            iter = match t {
                Transducers::Map(stack_func) => {
                    let vm_copy = Rc::clone(&vm);

                    let switch_statement = move |arg| {
                        vm_copy.borrow_mut().call_func_or_else(
                            stack_func,
                            arg?,
                            cur_inst_span,
                            throw!(TypeMismatch => "map expected a function"; *cur_inst_span),
                        )
                    };

                    Box::new(iter.map(switch_statement))
                }
                Transducers::Filter(stack_func) => {
                    let vm_copy = Rc::clone(&vm);

                    let switch_statement = move |arg: Result<SteelVal>| match arg {
                        Ok(arg) => {
                            let res = vm_copy.borrow_mut().call_func_or_else(
                                stack_func,
                                arg.clone(),
                                cur_inst_span,
                                throw!(TypeMismatch => "filter expected a function"; *cur_inst_span)
                            );

                            match res {
                                Ok(k) => match k {
                                    SteelVal::BoolV(true) => Some(Ok(arg)),
                                    SteelVal::BoolV(false) => None,
                                    _ => None,
                                },
                                Err(e) => Some(Err(e)),
                            }
                        }

                        _ => Some(arg),
                    };

                    Box::new(iter.filter_map(switch_statement))
                }
                Transducers::FlatMap(stack_func) => {
                    let vm_copy = Rc::clone(&vm);

                    let switch_statement =
                        move |arg: Result<SteelVal>| -> Box<dyn Iterator<Item = Result<SteelVal>>> {
                            match arg {
                                Ok(arg) => {
                                    let res = vm_copy.borrow_mut().call_func_or_else(
                                    stack_func,
                                    arg,
                                    cur_inst_span,
                                    throw!(TypeMismatch => "map expected a function"; *cur_inst_span),
                                );

                                    match res {
                                        Ok(x) => {
                                            match x {
                                                SteelVal::VectorV(v) => {
                                                    Box::new(v.unwrap().into_iter().map(Ok))
                                                }
                                                // TODO this needs to be fixed
                                                SteelVal::StringV(s) => Box::new(
                                                    s.chars()
                                                        .map(|x| Ok(SteelVal::CharV(x)))
                                                        .collect::<Vec<_>>()
                                                        .into_iter(),
                                                ),
                                                SteelVal::ListV(l) => {
                                                    Box::new(l.into_iter().map(Ok))
                                                }
                                                SteelVal::StructV(s) => {
                                                    Box::new(s.unwrap().fields.into_iter().map(Ok))
                                                }
                                                els => {
                                                    let err = SteelErr::new(ErrorKind::TypeMismatch, format!("flatten expected a traversable value, found: {}", els)).with_span(*cur_inst_span);

                                                    Box::new(std::iter::once(Err(err)))
                                                }
                                            }
                                        }
                                        err => Box::new(std::iter::once(err)),
                                    }
                                }

                                err => Box::new(std::iter::once(err)),
                            }
                        };

                    Box::new(iter.flat_map(switch_statement))
                }
                Transducers::Flatten => {
                    // TODO check if arg is iterable then iterator over it
                    // for things that aren't iterable, throw an error
                    // otherwise, do the generic iterable and extend it
                    // todo!()
                    // let switch_statement = move |arg| {

                    // }

                    let switch_statement =
                        move |arg: Result<SteelVal>| -> Box<dyn Iterator<Item = Result<SteelVal>>> {
                            match arg {
                                Ok(x) => {
                                    match x {
                                        SteelVal::VectorV(v) => {
                                            Box::new(v.unwrap().into_iter().map(Ok))
                                        }
                                        // TODO this needs to be fixed
                                        SteelVal::StringV(s) => Box::new(
                                            s.chars()
                                                .map(|x| Ok(SteelVal::CharV(x)))
                                                .collect::<Vec<_>>()
                                                .into_iter(),
                                        ),
                                        SteelVal::ListV(l) => Box::new(l.into_iter().map(Ok)),
                                        SteelVal::StructV(s) => {
                                            Box::new(s.unwrap().fields.into_iter().map(Ok))
                                        }
                                        els => {
                                            let err = SteelErr::new(ErrorKind::TypeMismatch, format!("flatten expected a traversable value, found: {}", els)).with_span(*cur_inst_span);

                                            Box::new(std::iter::once(Err(err)))
                                        }
                                    }
                                }
                                err => Box::new(std::iter::once(err)),
                            }
                        };

                    Box::new(iter.flat_map(switch_statement))
                }
                Transducers::Window(num) => {
                    todo!()
                }
                Transducers::TakeWhile(func) => {
                    todo!()
                }
                Transducers::DropWhile(func) => {
                    todo!()
                }
                Transducers::Cycle => {
                    todo!()
                }
                Transducers::Extend(collection) => {
                    let extender: Box<dyn Iterator<Item = Result<SteelVal>>> = match collection
                        .clone()
                    {
                        SteelVal::VectorV(v) => Box::new(v.unwrap().into_iter().map(Ok)),
                        // TODO this needs to be fixed
                        SteelVal::StringV(s) => Box::new(
                            s.chars()
                                .map(|x| Ok(SteelVal::CharV(x)))
                                .collect::<Vec<_>>()
                                .into_iter(),
                        ),
                        SteelVal::ListV(l) => Box::new(l.into_iter().map(Ok)),
                        SteelVal::StructV(s) => Box::new(s.unwrap().fields.into_iter().map(Ok)),
                        els => {
                            let err = SteelErr::new(
                                ErrorKind::TypeMismatch,
                                format!("extending expected a traversable value, found: {}", els),
                            )
                            .with_span(*cur_inst_span);

                            Box::new(std::iter::once(Err(err)))
                        }
                    };

                    Box::new(iter.chain(extender))
                }
                Transducers::Take(num) => generate_take!(iter, num, cur_inst_span),
                Transducers::Drop(num) => generate_drop!(iter, num, cur_inst_span),
            }
        }

        // If an output type is given, use that one
        if let Some(collection_type) = collection_type {
            if let SteelVal::SymbolV(n) = collection_type {
                match n.as_ref() {
                    // "list" => ListOperations::built_in_list_normal_iter(iter),
                    "vector" => VectorOperations::vec_construct_iter(iter),
                    "list" => iter.collect::<Result<List<_>>>().map(SteelVal::ListV),
                    _ => stop!(Generic => "Cannot collect into an undefined type"),
                }
            } else {
                stop!(Generic => "execute takes a symbol")
            }
        } else {
            match output_type {
                CollectionType::List => iter.collect::<Result<List<_>>>().map(SteelVal::ListV),
                CollectionType::Vector => VectorOperations::vec_construct_iter(iter),
            }
        }
    }

    pub(crate) fn transduce(
        &mut self,
        ops: &[Transducers],
        root: SteelVal,
        initial_value: SteelVal,
        reducer: SteelVal,
        cur_inst_span: &Span,
    ) -> Result<SteelVal> {
        let vm = Rc::new(RefCell::new(self));

        let mut iter = Self::res_iterator(&root, Rc::clone(&vm), cur_inst_span)?;

        for t in ops {
            iter = match t {
                Transducers::Map(stack_func) => {
                    let vm_copy = Rc::clone(&vm);

                    let switch_statement = move |arg| {
                        vm_copy.borrow_mut().call_func_or_else(
                            stack_func,
                            arg?,
                            cur_inst_span,
                            throw!(TypeMismatch => "map expected a function"; *cur_inst_span),
                        )
                    };

                    Box::new(iter.map(switch_statement))
                }
                Transducers::Filter(stack_func) => {
                    let vm_copy = Rc::clone(&vm);

                    let switch_statement = move |arg: Result<SteelVal>| match arg {
                        Ok(arg) => {
                            let res = vm_copy.borrow_mut().call_func_or_else(
                                stack_func,
                                arg.clone(),
                                cur_inst_span,
                                throw!(TypeMismatch => "filter expected a function"; *cur_inst_span)
                            );

                            match res {
                                Ok(k) => match k {
                                    SteelVal::BoolV(true) => Some(Ok(arg)),
                                    SteelVal::BoolV(false) => None,
                                    _ => None,
                                },
                                Err(e) => Some(Err(e)),
                            }
                        }

                        _ => Some(arg),
                    };

                    Box::new(iter.filter_map(switch_statement))
                }
                Transducers::FlatMap(stack_func) => {
                    todo!()
                }
                Transducers::Flatten => {
                    // TODO figure out how to use strings here
                    let switch_statement =
                        move |arg: Result<SteelVal>| -> Box<dyn Iterator<Item = Result<SteelVal>>> {
                            match arg {
                                Ok(x) => {
                                    match x {
                                        SteelVal::VectorV(v) => {
                                            Box::new(v.unwrap().into_iter().map(Ok))
                                        }
                                        // TODO this needs to be fixed
                                        SteelVal::StringV(s) => Box::new(
                                            s.chars()
                                                .map(|x| Ok(SteelVal::CharV(x)))
                                                .collect::<Vec<_>>()
                                                .into_iter(),
                                        ),
                                        SteelVal::ListV(l) => Box::new(l.into_iter().map(Ok)),
                                        SteelVal::StructV(s) => {
                                            Box::new(s.unwrap().fields.into_iter().map(Ok))
                                        }
                                        els => {
                                            let err = SteelErr::new(ErrorKind::TypeMismatch, format!("flatten expected a traversable value, found: {}", els)).with_span(*cur_inst_span);

                                            Box::new(std::iter::once(Err(err)))
                                        }
                                    }
                                }
                                err => Box::new(std::iter::once(err)),
                            }
                        };

                    Box::new(iter.flat_map(switch_statement))

                    // todo!()
                }
                Transducers::Window(num) => {
                    todo!()
                }
                Transducers::TakeWhile(func) => {
                    todo!()
                }
                Transducers::DropWhile(func) => {
                    todo!()
                }
                Transducers::Extend(collection) => {
                    todo!()
                }
                Transducers::Cycle => {
                    todo!()
                }
                Transducers::Take(num) => generate_take!(iter, num, cur_inst_span),
                Transducers::Drop(num) => generate_drop!(iter, num, cur_inst_span),
            }
        }

        let vm_copy = Rc::clone(&vm);

        let switch_statement = move |acc, x| {
            vm_copy.borrow_mut().call_func_or_else_two_args(
                &reducer,
                acc?,
                x?,
                cur_inst_span,
                throw!(TypeMismatch => "reduce expected a function"; *cur_inst_span),
            )
        };

        iter.fold(Ok(initial_value), switch_statement)
    }
}
