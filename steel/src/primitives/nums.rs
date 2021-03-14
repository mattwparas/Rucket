// use crate::env::{FALSE, TRUE};
use crate::rerrs::{ErrorKind, SteelErr};
// use crate::rvals::SteelVal::*;
use crate::rvals::{Result, SteelVal};
use crate::stop;
use rand::Rng;

pub struct NumOperations {}
impl NumOperations {
    pub fn random_int() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.is_empty() {
                stop!(ArityMismatch => "random-int requires an upper bound");
            }

            if args.len() > 1 {
                stop!(ArityMismatch => "random-int takes one argument")
            }

            if let SteelVal::IntV(upper_bound) = &args[0] {
                let mut rng = rand::thread_rng();
                return Ok(SteelVal::IntV(rng.gen_range(0..*upper_bound)));
            } else {
                stop!(TypeMismatch => "random-int requires an integer upper bound");
            }
        })
    }

    pub fn even() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "even? takes one argument")
            }
            if let SteelVal::IntV(n) = &args[0] {
                Ok(SteelVal::BoolV(n & 1 == 0))
            } else {
                stop!(TypeMismatch => "even? requires an integer")
            }
        })
    }

    pub fn odd() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "even? takes one argument")
            }

            if let SteelVal::IntV(n) = &args[0] {
                Ok(SteelVal::BoolV(n & 1 == 1))
            } else {
                stop!(TypeMismatch => "odd? requires an integer")
            }
        })
    }

    pub fn integer_add() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.is_empty() {
                stop!(ArityMismatch => "+ requires at least one argument")
            }

            let mut sum = 0;

            for arg in args {
                if let SteelVal::IntV(n) = arg {
                    sum += n;
                } else {
                    stop!(TypeMismatch => "+ expected a number, found {:?}", arg);
                }
            }

            Ok(SteelVal::IntV(sum))
        })
    }

    pub fn integer_sub() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.is_empty() {
                stop!(ArityMismatch => "+ requires at least one argument")
            }

            let mut sum = if let SteelVal::IntV(n) = &args[0] {
                *n
            } else {
                stop!(TypeMismatch => "- expected a number, found {:?}", &args[0])
            };

            for arg in &args[1..] {
                if let SteelVal::IntV(n) = arg {
                    sum -= n;
                } else {
                    stop!(TypeMismatch => "+ expected a number, found {:?}", arg);
                }
            }

            Ok(SteelVal::IntV(sum))
        })
    }

    pub fn float_add() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.is_empty() {
                stop!(ArityMismatch => "+ requires at least one argument")
            }

            let mut sum = 0.0;

            for arg in args {
                if let SteelVal::NumV(n) = arg {
                    sum += n;
                } else {
                    stop!(TypeMismatch => "+ expected a number, found {:?}", arg);
                }
            }

            Ok(SteelVal::NumV(sum))
        })
    }

    pub fn adder() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.is_empty() {
                stop!(ArityMismatch => "+ requires at least one argument")
            }

            let mut sum_int = 0;
            let mut sum_float = 0.0;
            let mut found_float = false;

            for arg in args {
                match arg {
                    SteelVal::IntV(n) => {
                        if found_float {
                            sum_float += *n as f64;
                        } else {
                            if let Some(res) = isize::checked_add(sum_int, *n) {
                                sum_int = res
                            } else {
                                found_float = true;
                                sum_float += *n as f64;
                            }
                        }
                    }
                    SteelVal::NumV(n) => {
                        if !found_float {
                            sum_float = sum_int as f64;
                            found_float = true
                        }
                        sum_float += n;
                    }
                    _ => {
                        let e = format!("+ expected a number, found {:?}", arg);
                        stop!(TypeMismatch => e);
                    }
                }
            }

            if found_float {
                Ok(SteelVal::NumV(sum_float))
            } else {
                Ok(SteelVal::IntV(sum_int))
            }
        })
    }

    pub fn multiply() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.is_empty() {
                stop!(ArityMismatch => "* requires at least one argument")
            }

            let mut sum_int = 1;
            let mut sum_float = 1.0;
            let mut found_float = false;

            for arg in args {
                match arg {
                    SteelVal::IntV(n) => {
                        if found_float {
                            sum_float *= *n as f64;
                        } else {
                            if let Some(res) = isize::checked_mul(sum_int, *n) {
                                sum_int = res
                            } else {
                                found_float = true;
                                sum_float *= *n as f64;
                            }
                        }
                    }
                    SteelVal::NumV(n) => {
                        if !found_float {
                            sum_float = sum_int as f64;
                            found_float = true
                        }
                        sum_float *= n;
                    }
                    _ => stop!(TypeMismatch => "* expected a number"),
                }
            }

            if found_float {
                Ok(SteelVal::NumV(sum_float))
            } else {
                Ok(SteelVal::IntV(sum_int))
            }
        })
    }

    // TODO implement the full numerical tower
    // For now, only support division into floats
    pub fn divide() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.is_empty() {
                stop!(ArityMismatch => "/ requires at least one argument")
            }

            let floats: Result<Vec<f64>> = args
                .iter()
                .map(|x| match x {
                    SteelVal::IntV(n) => Ok(*n as f64),
                    SteelVal::NumV(n) => Ok(*n),
                    _ => stop!(TypeMismatch => "division expects a number"),
                })
                .collect();

            let mut floats = floats?.into_iter();

            if let Some(first) = floats.next() {
                Ok(SteelVal::NumV(floats.fold(first, |acc, x| acc / x)))
            } else {
                stop!(ArityMismatch => "division requires at least one argument")
            }
        })
    }

    pub fn subtract() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.is_empty() {
                stop!(ArityMismatch => "- requires at least one argument")
            }

            let mut sum_int = 0;
            let mut sum_float = 0.0;
            let mut found_float = false;

            let mut args = args.into_iter();

            if let Some(first) = args.next() {
                match first {
                    SteelVal::IntV(n) => {
                        sum_int = *n;
                        // sum_float = *n as f64;
                    }
                    SteelVal::NumV(n) => {
                        found_float = true;
                        sum_float = *n;
                    }
                    _ => stop!(TypeMismatch => "'-' expected a number type"),
                }
            }

            for arg in args {
                match arg {
                    SteelVal::IntV(n) => {
                        if found_float {
                            sum_float -= *n as f64;
                        } else {
                            if let Some(res) = isize::checked_sub(sum_int, *n) {
                                sum_int = res
                            } else {
                                found_float = true;
                                sum_float -= *n as f64;
                            }
                        }
                    }
                    SteelVal::NumV(n) => {
                        if !found_float {
                            sum_float = sum_int as f64;
                            found_float = true
                        }
                        sum_float -= n;
                    }
                    _ => stop!(TypeMismatch => "- expected a number"),
                }
            }

            if found_float {
                Ok(SteelVal::NumV(sum_float))
            } else {
                Ok(SteelVal::IntV(sum_int))
            }
        })
    }
}

#[cfg(test)]
mod num_op_tests {

    use super::*;
    use crate::rvals::SteelVal::*;
    use crate::throw;

    fn apply_function(func: SteelVal, args: Vec<SteelVal>) -> Result<SteelVal> {
        func.func_or_else(throw!(BadSyntax => "num op tests"))
            .unwrap()(&args)
    }

    #[test]
    fn division_test() {
        let args = vec![IntV(10), IntV(2)];

        let output = apply_function(NumOperations::divide(), args).unwrap();
        let expected = NumV(5.0);
        assert_eq!(output.to_string(), expected.to_string());
    }

    #[test]
    fn multiplication_test() {
        let args = vec![IntV(10), IntV(2)];

        let output = apply_function(NumOperations::multiply(), args).unwrap();
        let expected = IntV(20);
        assert_eq!(output, expected);
    }

    #[test]
    fn multiplication_different_types() {
        let args = vec![IntV(10), NumV(2.0)];

        let output = apply_function(NumOperations::multiply(), args).unwrap();
        let expected = NumV(20.0);
        assert_eq!(output.to_string(), expected.to_string());
    }

    #[test]
    fn addition_different_types() {
        let args = vec![IntV(10), NumV(2.0)];

        let output = apply_function(NumOperations::adder(), args).unwrap();
        let expected = NumV(12.0);
        assert_eq!(output.to_string(), expected.to_string());
    }

    #[test]
    fn subtraction_different_types() {
        let args = vec![IntV(10), NumV(2.0)];

        let output = apply_function(NumOperations::subtract(), args).unwrap();
        let expected = NumV(8.0);
        assert_eq!(output.to_string(), expected.to_string());
    }

    #[test]
    fn test_integer_add() {
        let args = vec![IntV(10), IntV(2)];

        let output = apply_function(NumOperations::integer_add(), args).unwrap();
        let expected = IntV(12);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_integer_sub() {
        let args = vec![IntV(10), IntV(2)];

        let output = apply_function(NumOperations::integer_sub(), args).unwrap();
        let expected = IntV(8);
        assert_eq!(output, expected);
    }
}
