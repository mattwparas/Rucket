use std::{cell::RefCell, ops::DerefMut};

use crate::gc::Gc;
use crate::rvals::SteelVal::*;
use crate::rvals::{Result, SteelVal};
use crate::steel_vm::vm::VmCore;
use crate::stop;
use im_rc::Vector;

pub struct VectorOperations {}
impl VectorOperations {
    pub fn vec_construct() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            Ok(SteelVal::VectorV(
                Gc::new(args.iter().cloned().collect::<Vector<_>>()).into(),
            ))
        })
    }

    // TODO
    pub fn mut_vec_construct() -> SteelVal {
        SteelVal::BuiltIn(
            |ctx: &mut VmCore, args: &[SteelVal]| -> Option<Result<SteelVal>> {
                Some(Ok(ctx.make_mutable_vector(args.to_vec())))
            },
        )
    }

    pub fn mut_vec_to_list() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "mutable-vector->list takes one argument, found: {:?}", args.len())
            }

            let vec = &args[0];

            if let SteelVal::MutableVector(v) = vec {
                let ptr = v.strong_ptr();
                let guard = &mut ptr.borrow_mut().value;

                let new = std::mem::replace(guard, Vec::new());

                Ok(SteelVal::ListV(new.into()))

                // let inner = std::mem::take(guard);

                // Ok(SteelVal::ListV(inner.into()))
            } else {
                stop!(TypeMismatch => "mutable-vector->list expects a vector, found: {:?}", vec);
            }
        })
    }

    pub fn vec_length() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "vector-length expects one argument found: {:?}", args.len());
            }

            let vec = args[0].clone();

            if let SteelVal::VectorV(v) = vec {
                Ok(SteelVal::IntV(v.len() as isize))
            } else {
                stop!(TypeMismatch => "vector-length expects a mutable vector, found: {:?}", vec);
            }
        })
    }

    pub fn mut_vec_length() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "mut-vec-length expects one argument found: {:?}", args.len());
            }

            let vec = args[0].clone();

            if let SteelVal::MutableVector(v) = vec {
                Ok(SteelVal::IntV(v.get().len() as isize))
            } else {
                stop!(TypeMismatch => "mut-vec-length expects a mutable vector, found: {:?}", vec);
            }
        })
    }

    // (vector-set! vec pos value)
    pub fn mut_vec_set() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 3 {
                stop!(ArityMismatch => "vector-set! takes three arguments, found: {:?}", args.len())
            }

            let vec = args[0].clone();
            let pos = args[1].clone();

            if let SteelVal::MutableVector(v) = &vec {
                if let SteelVal::IntV(i) = pos {
                    if i < 0 {
                        stop!(Generic => "vector-set! expects a positive integer, found: {:?}", vec);
                    }

                    let ptr = v.strong_ptr();

                    let guard = &mut ptr.borrow_mut().value;

                    if i as usize > guard.len() {
                        stop!(Generic => "index out of bounds, index given: {:?}, length of vector: {:?}", i, guard.len());
                    }

                    // TODO: disallow cyclical references on construction

                    // Update the vector position
                    guard[i as usize] = args[2].clone();

                    Ok(SteelVal::Void)
                } else {
                    stop!(TypeMismatch => "vector-set! expects an integer, found: {:?}", pos);
                }
            } else {
                stop!(TypeMismatch => "vector-set! expects a vector, found: {:?}", vec);
            }
        })
    }

    pub fn mut_vec_get() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 2 {
                stop!(ArityMismatch => "mut-vector-ref takes two arguments, found: {:?}", args.len())
            }

            let vec = args[0].clone();
            let pos = args[1].clone();

            if let SteelVal::MutableVector(v) = &vec {
                if let SteelVal::IntV(i) = pos {
                    if i < 0 {
                        stop!(Generic => "mut-vector-ref expects a positive integer, found: {:?}", vec);
                    }

                    let ptr = v.strong_ptr();

                    let guard = &mut ptr.borrow_mut().value;

                    if i as usize >= guard.len() {
                        stop!(Generic => "index out of bounds, index given: {:?}, length of vector: {:?}", i, guard.len());
                    }

                    // Grab the value out of the vector
                    Ok(guard[i as usize].clone())
                } else {
                    stop!(TypeMismatch => "mut-vector-ref expects an integer, found: {:?}", pos);
                }
            } else {
                stop!(TypeMismatch => "mut-vector-ref expects a vector, found: {:?}", vec);
            }
        })
    }

    pub fn mut_vec_push() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 2 {
                stop!(ArityMismatch => "vector-push! takes two arguments, found: {:?}", args.len())
            }

            let vec = &args[0];

            if let SteelVal::MutableVector(v) = vec {
                // TODO -> make sure this is the correct thing
                // if vec.other_contains_self(&args[1]) {
                //     stop!(Generic => "vector push would create a cyclical reference, which would cause a memory leak")
                // }

                // TODO: disallow cyclical references on construction
                v.strong_ptr().borrow_mut().value.push(args[1].clone());
                Ok(SteelVal::Void)
            } else {
                stop!(TypeMismatch => "vector-push! expects a vector, found: {:?}", vec);
            }
        })
    }

    pub fn mut_vec_append() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 2 {
                stop!(ArityMismatch => "vector-append! takes two arguments, found: {:?}", args.len())
            }

            let vec = args[0].clone();
            let other_vec = args[1].clone();

            if let SteelVal::MutableVector(left) = vec {
                if let SteelVal::MutableVector(right) = other_vec {
                    left.strong_ptr()
                        .borrow_mut()
                        .value
                        .append(&mut right.strong_ptr().borrow_mut().value);
                    Ok(SteelVal::Void)
                } else {
                    stop!(TypeMismatch => "vetor-append! expects a vector in the second position, found: {:?}", other_vec);
                }
            } else {
                stop!(TypeMismatch => "vector-append! expects a vector in the first position, found: {:?}", vec);
            }
        })
    }

    pub fn vec_construct_iter<I: Iterator<Item = Result<SteelVal>>>(arg: I) -> Result<SteelVal> {
        let res: Result<Vector<SteelVal>> = arg.collect();
        Ok(SteelVal::VectorV(Gc::new(res?).into()))
    }

    pub fn vec_construct_iter_normal<I: Iterator<Item = SteelVal>>(arg: I) -> Result<SteelVal> {
        Ok(SteelVal::VectorV(
            Gc::new(arg.collect::<Vector<SteelVal>>()).into(),
        ))
    }

    pub fn vec_append() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            let lsts: Vector<SteelVal> = unwrap_list_of_lists(args.to_vec())?
                .into_iter()
                .flatten()
                .collect();
            Ok(SteelVal::VectorV(Gc::new(lsts).into()))
        })
    }

    pub fn vec_ref() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 2 {
                stop!(ArityMismatch => "vector-ref takes two arguments");
            }
            let mut args = args.iter();
            match (args.next(), args.next()) {
                (Some(vec), Some(idx)) => {
                    if let (VectorV(vec), IntV(idx)) = (vec, idx) {
                        if idx < &0 {
                            stop!(TypeMismatch => "vector-ref expected a positive integer");
                        }

                        let idx: usize = *idx as usize;

                        if idx < vec.len() {
                            Ok(vec[idx].clone())
                        } else {
                            let e = format!("Index out of bounds - attempted to access index: {} with length: {}", idx, vec.len());
                            stop!(Generic => e);
                        }
                    } else {
                        stop!(TypeMismatch => format!("vector-ref expected a vector and a number, found: {vec} and {idx}"))
                    }
                }
                _ => stop!(ArityMismatch => "vector-ref takes two arguments"),
            }
        })
    }

    pub fn vec_range() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 2 {
                stop!(ArityMismatch => "range takes two arguments");
            }
            let mut args = args.iter();
            match (args.next(), args.next()) {
                (Some(elem), Some(lst)) => {
                    if let (IntV(lower), IntV(upper)) = (elem, lst) {
                        Ok(SteelVal::VectorV(
                            Gc::new(
                                (*lower as usize..*upper as usize)
                                    .into_iter()
                                    .map(|x| SteelVal::IntV(x as isize))
                                    .collect::<Vector<_>>(),
                            )
                            .into(),
                        ))
                    } else {
                        stop!(TypeMismatch => "range expected number")
                    }
                }
                _ => stop!(ArityMismatch => "range takes two arguments"),
            }
        })
    }

    pub fn vec_push() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 2 {
                stop!(ArityMismatch => "push takes two arguments");
            }
            let mut args = args.iter();
            match (args.next(), args.next()) {
                (Some(elem), Some(lst)) => {
                    if let SteelVal::VectorV(l) = lst {
                        let mut l = l.0.unwrap();
                        l.push_back(elem.clone());
                        Ok(SteelVal::VectorV(Gc::new(l).into()))
                    } else {
                        let mut new = Vector::new();
                        new.push_front(elem.clone());
                        new.push_front(lst.clone());
                        Ok(SteelVal::VectorV(Gc::new(new).into()))
                    }
                }
                _ => stop!(ArityMismatch => "push takes two arguments"),
            }
        })
    }

    pub fn vec_cons() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 2 {
                stop!(ArityMismatch => "cons takes two arguments")
            }
            let mut args = args.iter();
            match (args.next(), args.next()) {
                (Some(elem), Some(lst)) => {
                    if let SteelVal::VectorV(l) = lst {
                        let mut l = l.0.unwrap();
                        l.push_front(elem.clone());
                        Ok(SteelVal::VectorV(Gc::new(l).into()))
                    } else {
                        let mut new = Vector::new();
                        new.push_front(lst.clone());
                        new.push_front(elem.clone());
                        Ok(SteelVal::VectorV(Gc::new(new).into()))
                    }
                }
                _ => stop!(ArityMismatch => "cons takes two arguments"),
            }
        })
    }

    pub fn vec_car() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "car takes one argument");
            }
            if let Some(first) = args.iter().next() {
                match first {
                    SteelVal::VectorV(e) => {
                        let mut e = e.0.unwrap();
                        match e.pop_front() {
                            Some(e) => Ok(e),
                            None => stop!(ContractViolation => "car expects a non empty list"),
                        }
                    }
                    e => {
                        stop!(TypeMismatch => "car takes a list, given: {}", e);
                    }
                }
            } else {
                stop!(ArityMismatch => "car takes one argument");
            }
        })
    }

    pub fn vec_cdr() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "cdr takes one argument");
            }
            if let Some(first) = args.iter().next() {
                match first {
                    SteelVal::VectorV(e) => {
                        let mut e = e.0.unwrap();
                        if !e.is_empty() {
                            e.pop_front();
                            Ok(SteelVal::VectorV(Gc::new(e).into()))
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
        })
    }

    pub fn list_vec_null() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() == 1 {
                match &args[0] {
                    SteelVal::ListV(l) => Ok(l.is_empty().into()),
                    SteelVal::VectorV(v) => Ok(v.is_empty().into()),
                    _ => Ok(SteelVal::BoolV(false)),
                }
            } else {
                stop!(ArityMismatch => "null? takes one argument");
            }
        })
    }
}

fn unwrap_list_of_lists(args: Vec<SteelVal>) -> Result<Vec<Vector<SteelVal>>> {
    args.iter().map(unwrap_single_list).collect()
}

fn unwrap_single_list(exp: &SteelVal) -> Result<Vector<SteelVal>> {
    match exp {
        SteelVal::VectorV(lst) => Ok(lst.0.unwrap()),
        _ => stop!(TypeMismatch => "expected a list"),
    }
}

#[cfg(test)]
mod vector_prim_tests {
    use super::*;
    use crate::throw;
    use im_rc::vector;

    fn apply_function(func: SteelVal, args: Vec<SteelVal>) -> Result<SteelVal> {
        func.func_or_else(throw!(BadSyntax => "string tests"))
            .unwrap()(&args)
    }

    #[test]
    fn vec_construct_test() {
        let args = vec![SteelVal::IntV(1), SteelVal::IntV(2), SteelVal::IntV(3)];
        let res = apply_function(VectorOperations::vec_construct(), args);
        let expected = vector![SteelVal::IntV(1), SteelVal::IntV(2), SteelVal::IntV(3)].into();
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn vec_append_test_good_inputs() {
        let args = vec![
            vector![SteelVal::IntV(1), SteelVal::IntV(2), SteelVal::IntV(3)].into(),
            vector![SteelVal::IntV(1), SteelVal::IntV(2), SteelVal::IntV(3)].into(),
            vector![SteelVal::IntV(1), SteelVal::IntV(2), SteelVal::IntV(3)].into(),
        ];

        let res = apply_function(VectorOperations::vec_append(), args);
        let expected = vector![
            SteelVal::IntV(1),
            SteelVal::IntV(2),
            SteelVal::IntV(3),
            SteelVal::IntV(1),
            SteelVal::IntV(2),
            SteelVal::IntV(3),
            SteelVal::IntV(1),
            SteelVal::IntV(2),
            SteelVal::IntV(3)
        ]
        .into();
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn vec_append_test_bad_inputs() {
        let args = vec![
            vector![SteelVal::IntV(1), SteelVal::IntV(2), SteelVal::IntV(3)].into(),
            SteelVal::StringV("foo".into()),
            vector![SteelVal::IntV(1), SteelVal::IntV(2), SteelVal::IntV(3)].into(),
        ];
        let res = apply_function(VectorOperations::vec_append(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_range_test_arity_too_few() {
        let args = vec![SteelVal::NumV(1.0)];

        let res = apply_function(VectorOperations::vec_range(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_range_test_arity_too_many() {
        let args = vec![SteelVal::IntV(1), SteelVal::IntV(1), SteelVal::IntV(1)];

        let res = apply_function(VectorOperations::vec_range(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_range_test_bad_input() {
        let args = vec![SteelVal::StringV("1".into()), SteelVal::NumV(1.0)];

        let res = apply_function(VectorOperations::vec_range(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_range_test_normal() {
        let args = vec![SteelVal::IntV(0), SteelVal::IntV(4)];

        let res = apply_function(VectorOperations::vec_range(), args);
        let expected = vector![
            SteelVal::IntV(0),
            SteelVal::IntV(1),
            SteelVal::IntV(2),
            SteelVal::IntV(3)
        ]
        .into();
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn vec_push_arity_too_few() {
        let args = vec![SteelVal::StringV("foo".into())];
        let res = apply_function(VectorOperations::vec_push(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_push_arity_too_many() {
        let args = vec![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("foo".into()),
        ];
        let res = apply_function(VectorOperations::vec_push(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_push_test_good_input_pair() {
        let args = vec![
            SteelVal::StringV("baz".into()),
            SteelVal::StringV("bar".into()),
        ];
        let res = apply_function(VectorOperations::vec_push(), args);
        let expected = vector![
            SteelVal::StringV("bar".into()),
            SteelVal::StringV("baz".into()),
        ]
        .into();
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn vec_push_test_good_input() {
        let args = vec![
            SteelVal::StringV("baz".into()),
            vector![
                SteelVal::StringV("foo".into()),
                SteelVal::StringV("bar".into())
            ]
            .into(),
        ];
        let res = apply_function(VectorOperations::vec_push(), args);
        let expected = vector![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("bar".into()),
            SteelVal::StringV("baz".into())
        ]
        .into();
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn vec_cons_test_arity_too_few() {
        let args = vec![];
        let res = apply_function(VectorOperations::vec_cons(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_cons_test_arity_too_many() {
        let args = vec![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("foo".into()),
        ];
        let res = apply_function(VectorOperations::vec_cons(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_cons_pair() {
        let args = vec![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("bar".into()),
        ];
        let res = apply_function(VectorOperations::vec_cons(), args);
        let expected = vector![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("bar".into())
        ]
        .into();
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn vec_cons_elem_vector() {
        let args = vec![
            SteelVal::StringV("foo".into()),
            vector![
                SteelVal::StringV("bar".into()),
                SteelVal::StringV("baz".into())
            ]
            .into(),
        ];
        let res = apply_function(VectorOperations::vec_cons(), args);
        let expected = vector![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("bar".into()),
            SteelVal::StringV("baz".into())
        ]
        .into();
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn vec_car_arity_too_few() {
        let args = vec![];
        let res = apply_function(VectorOperations::vec_car(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_car_arity_too_many() {
        let args = vec![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("bar".into()),
        ];
        let res = apply_function(VectorOperations::vec_car(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_car_bad_input() {
        let args = vec![SteelVal::StringV("foo".into())];
        let res = apply_function(VectorOperations::vec_car(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_car_normal_input() {
        let args = vec![vector![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("bar".into())
        ]
        .into()];
        let res = apply_function(VectorOperations::vec_car(), args);
        let expected = SteelVal::StringV("foo".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn vec_cdr_arity_too_few() {
        let args = vec![];
        let res = apply_function(VectorOperations::vec_cdr(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_cdr_arity_too_many() {
        let args = vec![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("bar".into()),
        ];
        let res = apply_function(VectorOperations::vec_cdr(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_cdr_bad_input() {
        let args = vec![SteelVal::NumV(1.0)];
        let res = apply_function(VectorOperations::vec_cdr(), args);
        assert!(res.is_err());
    }

    #[test]
    fn vec_cdr_normal_input() {
        let args = vec![vector![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("bar".into())
        ]
        .into()];
        let res = apply_function(VectorOperations::vec_cdr(), args);
        let expected = vector![SteelVal::StringV("bar".into())].into();
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn vec_cdr_empty_list() {
        let args = vec![Vector::new().into()];
        let res = apply_function(VectorOperations::vec_cdr(), args);
        assert!(res.is_err());
    }

    #[test]
    fn list_vec_arity() {
        let args = vec![];
        let res = apply_function(VectorOperations::list_vec_null(), args);
        assert!(res.is_err());
    }

    #[test]
    fn list_vec_anything_but_null() {
        let args = vec![SteelVal::StringV("foo".into())];
        let res = apply_function(VectorOperations::list_vec_null(), args);
        let expected = SteelVal::BoolV(false);
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn list_vec_non_empty_vec() {
        let args = vec![vector![
            SteelVal::StringV("foo".into()),
            SteelVal::StringV("bar".into())
        ]
        .into()];
        let res = apply_function(VectorOperations::list_vec_null(), args);
        let expected = SteelVal::BoolV(false);
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn list_vec_empty_vec() {
        let args = vec![Vector::new().into()];
        let res = apply_function(VectorOperations::list_vec_null(), args);
        let expected = SteelVal::BoolV(true);
        assert_eq!(res.unwrap(), expected);
    }
}
