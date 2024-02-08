use crate::rvals::{IntoSteelVal, Result, SteelVal};
use crate::steel_vm::primitives::numberp;
use crate::stop;
use num::{BigInt, BigRational, CheckedAdd, CheckedMul, Integer, Rational32, ToPrimitive};
use std::ops::Neg;

fn ensure_args_are_numbers(op: &str, args: &[SteelVal]) -> Result<()> {
    for arg in args {
        if !numberp(arg) {
            stop!(TypeMismatch => "{op} expects a number, found: {:?}", arg)
        }
    }
    Ok(())
}

/// # Precondition
/// - `x` and `y` must be valid numerical types.
fn multiply_2_impl(x: &SteelVal, y: &SteelVal) -> Result<SteelVal> {
    match (x, y) {
        (SteelVal::NumV(x), SteelVal::NumV(y)) => (x * y).into_steelval(),
        (SteelVal::NumV(x), SteelVal::IntV(y)) | (SteelVal::IntV(y), SteelVal::NumV(x)) => {
            (x * *y as f64).into_steelval()
        }
        (SteelVal::NumV(x), SteelVal::BigNum(y)) | (SteelVal::BigNum(y), SteelVal::NumV(x)) => {
            (x * y.to_f64().unwrap()).into_steelval()
        }
        (SteelVal::NumV(x), SteelVal::FractV(y)) | (SteelVal::FractV(y), SteelVal::NumV(x)) => {
            (x * y.to_f64().unwrap()).into_steelval()
        }
        (SteelVal::NumV(x), SteelVal::BigFract(y)) | (SteelVal::BigFract(y), SteelVal::NumV(x)) => {
            (x * y.to_f64().unwrap()).into_steelval()
        }
        (SteelVal::IntV(x), SteelVal::IntV(y)) => match x.checked_mul(y) {
            Some(res) => res.into_steelval(),
            None => {
                let mut res = BigInt::from(*x);
                res *= *y;
                res.into_steelval()
            }
        },
        (SteelVal::IntV(x), SteelVal::BigNum(y)) | (SteelVal::BigNum(y), SteelVal::IntV(x)) => {
            (y.as_ref() * x).into_steelval()
        }
        (SteelVal::IntV(x), SteelVal::FractV(y)) | (SteelVal::FractV(y), SteelVal::IntV(x)) => {
            match i32::try_from(*x) {
                Ok(x) => match y.checked_mul(&Rational32::new(x, 1)) {
                    Some(res) => res.into_steelval(),
                    None => {
                        let mut res =
                            BigRational::new(BigInt::from(*y.numer()), BigInt::from(*y.denom()));
                        res *= BigInt::from(x);
                        res.into_steelval()
                    }
                },
                Err(_) => {
                    let mut res =
                        BigRational::new(BigInt::from(*y.numer()), BigInt::from(*y.denom()));
                    res *= BigInt::from(*x);
                    res.into_steelval()
                }
            }
        }
        (SteelVal::IntV(x), SteelVal::BigFract(y)) | (SteelVal::BigFract(y), SteelVal::IntV(x)) => {
            let mut res = y.as_ref().clone();
            res *= BigInt::from(*x);
            res.into_steelval()
        }
        (SteelVal::FractV(x), SteelVal::FractV(y)) => match x.checked_mul(y) {
            Some(res) => res.into_steelval(),
            None => {
                let mut res = BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom()));
                res *= BigRational::new(BigInt::from(*y.numer()), BigInt::from(*y.denom()));
                res.into_steelval()
            }
        },
        (SteelVal::FractV(x), SteelVal::BigNum(y)) | (SteelVal::BigNum(y), SteelVal::FractV(x)) => {
            let mut res = BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom()));
            res *= y.as_ref();
            res.into_steelval()
        }
        (SteelVal::BigFract(x), SteelVal::BigFract(y)) => (x.as_ref() * y.as_ref()).into_steelval(),
        (SteelVal::BigFract(x), SteelVal::BigNum(y))
        | (SteelVal::BigNum(y), SteelVal::BigFract(x)) => (x.as_ref() * y.as_ref()).into_steelval(),
        (SteelVal::BigNum(x), SteelVal::BigNum(y)) => (x.as_ref() * y.as_ref()).into_steelval(),
        _ => unreachable!(),
    }
}

/// # Precondition
/// All types in `args` must be numerical types.
fn multiply_primitive_impl(args: &[SteelVal]) -> Result<SteelVal> {
    match args {
        [] => 1.into_steelval(),
        [x] => x.clone().into_steelval(),
        [x, y] => multiply_2_impl(x, y).into_steelval(),
        [x, y, zs @ ..] => {
            let mut res = multiply_2_impl(x, y)?;
            for z in zs {
                // TODO: This use case could be optimized to reuse state instead of creating a new
                // object each time.
                res = multiply_2_impl(&res, &z)?;
            }
            res.into_steelval()
        }
    }
}

#[steel_derive::native(name = "*", constant = true, arity = "AtLeast(0)")]
pub fn multiply_primitive(args: &[SteelVal]) -> Result<SteelVal> {
    ensure_args_are_numbers("*", args)?;
    multiply_primitive_impl(args)
}

pub fn quotient(l: isize, r: isize) -> isize {
    l / r
}

#[steel_derive::native(name = "/", constant = true, arity = "AtLeast(1)")]
pub fn divide_primitive(args: &[SteelVal]) -> Result<SteelVal> {
    ensure_args_are_numbers("/", args)?;
    let recip = |x: &SteelVal| -> Result<SteelVal> {
        match x {
            SteelVal::IntV(n) => match i32::try_from(*n) {
                Ok(n) => Rational32::new(1, n).into_steelval(),
                Err(_) => BigRational::new(BigInt::from(1), BigInt::from(*n)).into_steelval(),
            },
            SteelVal::NumV(n) => n.recip().into_steelval(),
            SteelVal::FractV(f) => f.recip().into_steelval(),
            SteelVal::BigFract(f) => f.recip().into_steelval(),
            SteelVal::BigNum(n) => BigRational::new(1.into(), n.as_ref().clone()).into_steelval(),
            unexpected => {
                stop!(TypeMismatch => "/ expects a number, but found: {:?}", unexpected)
            }
        }
    };
    match &args {
        [] => stop!(ArityMismatch => "/ requires at least one argument"),
        [x] => recip(x),
        // TODO: Provide custom implementation to optimize by joining the multiply and recip calls.
        [x, y] => multiply_2_impl(x, &recip(y)?),
        [x, ys @ ..] => {
            let d = multiply_primitive_impl(ys)?;
            multiply_2_impl(&x, &recip(&d)?)
        }
    }
}

#[steel_derive::native(name = "-", constant = true, arity = "AtLeast(1)")]
pub fn subtract_primitive(args: &[SteelVal]) -> Result<SteelVal> {
    ensure_args_are_numbers("-", args)?;
    let negate = |x: &SteelVal| match x {
        SteelVal::NumV(x) => (-x).into_steelval(),
        SteelVal::IntV(x) => match x.checked_neg() {
            Some(res) => res.into_steelval(),
            None => BigInt::from(*x).neg().into_steelval(),
        },
        SteelVal::FractV(x) => match 0i32.checked_sub(*x.numer()) {
            Some(n) => Rational32::new(n, *x.denom()).into_steelval(),
            None => BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom()))
                .neg()
                .into_steelval(),
        },
        SteelVal::BigFract(x) => x.as_ref().neg().into_steelval(),
        SteelVal::BigNum(x) => x.as_ref().clone().neg().into_steelval(),
        _ => unreachable!(),
    };
    match args {
        [] => stop!(TypeMismatch => "- requires at least one argument"),
        [x] => negate(x),
        [x, ys @ ..] => {
            let y = negate(&add_primitive(ys)?)?;
            add_primitive(&[x.clone(), y])
        }
    }
}

#[steel_derive::native(name = "+", constant = true, arity = "AtLeast(0)")]
pub fn add_primitive(args: &[SteelVal]) -> Result<SteelVal> {
    ensure_args_are_numbers("+", args)?;
    let add = |x: &SteelVal, y: &SteelVal| match (x, y) {
        // Simple integer case. Probably very common.
        (SteelVal::IntV(x), SteelVal::IntV(y)) => match x.checked_add(y) {
            Some(res) => res.into_steelval(),
            None => {
                let mut res = BigInt::from(*x);
                res += *y;
                res.into_steelval()
            }
        },
        // Cases that return an `f64`.
        (SteelVal::NumV(x), SteelVal::NumV(y)) => (x + y).into_steelval(),
        (SteelVal::NumV(x), SteelVal::IntV(y)) | (SteelVal::IntV(y), SteelVal::NumV(x)) => {
            (x + *y as f64).into_steelval()
        }
        (SteelVal::NumV(x), SteelVal::BigNum(y)) | (SteelVal::BigNum(y), SteelVal::NumV(x)) => {
            (x + y.to_f64().unwrap()).into_steelval()
        }
        (SteelVal::NumV(x), SteelVal::FractV(y)) | (SteelVal::FractV(y), SteelVal::NumV(x)) => {
            (x + y.to_f64().unwrap()).into_steelval()
        }
        (SteelVal::NumV(x), SteelVal::BigFract(y)) | (SteelVal::BigFract(y), SteelVal::NumV(x)) => {
            (x + y.to_f64().unwrap()).into_steelval()
        }
        // Cases that interact with `FractV`.
        (SteelVal::FractV(x), SteelVal::FractV(y)) => (x + y).into_steelval(),
        (SteelVal::FractV(x), SteelVal::IntV(y)) | (SteelVal::IntV(y), SteelVal::FractV(x)) => {
            match i32::try_from(*y) {
                Ok(y) => match x.checked_add(&Rational32::new(y, 1)) {
                    Some(res) => res.into_steelval(),
                    None => {
                        let res =
                            BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom()))
                                * BigInt::from(y);
                        res.into_steelval()
                    }
                },
                Err(_) => {
                    let res = BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom()))
                        * BigInt::from(*y);
                    res.into_steelval()
                }
            }
        }
        (SteelVal::FractV(x), SteelVal::BigNum(y)) | (SteelVal::BigNum(y), SteelVal::FractV(x)) => {
            let res =
                BigRational::new(BigInt::from(*x.numer()), BigInt::from(*x.denom())) * y.as_ref();
            res.into_steelval()
        }
        // Cases that interact with `BigFract`. Hopefully not too common, for performance reasons.
        (SteelVal::BigFract(x), SteelVal::BigFract(y)) => (x.as_ref() + y.as_ref()).into_steelval(),
        (SteelVal::BigFract(x), SteelVal::IntV(y)) | (SteelVal::IntV(y), SteelVal::BigFract(x)) => {
            (x.as_ref() + BigInt::from(*y)).into_steelval()
        }
        (SteelVal::BigFract(x), SteelVal::BigNum(y))
        | (SteelVal::BigNum(y), SteelVal::BigFract(x)) => (x.as_ref() * y.as_ref()).into_steelval(),
        // Remaining cases that interact with `BigNum`. Probably not too common.
        (SteelVal::BigNum(x), SteelVal::BigNum(y)) => {
            let mut res = x.as_ref().clone();
            res += y.as_ref();
            res.into_steelval()
        }
        (SteelVal::BigNum(x), SteelVal::IntV(y)) | (SteelVal::IntV(y), SteelVal::BigNum(x)) => {
            let mut res = x.as_ref().clone();
            res += *y;
            res.into_steelval()
        }
        _ => unreachable!(),
    };
    match args {
        [] => 0.into_steelval(),
        [x] => x.clone().into_steelval(),
        [x, y] => add(x, y),
        [x, y, zs @ ..] => {
            let mut res = add(x, y)?;
            for z in zs {
                res = add(&res, z)?;
            }
            res.into_steelval()
        }
    }
}

#[steel_derive::function(name = "exact?", constant = true)]
pub fn exactp(value: &SteelVal) -> bool {
    matches!(
        value,
        SteelVal::IntV(_) | SteelVal::BigNum(_) | SteelVal::FractV(_) | SteelVal::BigFract(_)
    )
}

#[steel_derive::function(name = "inexact?", constant = true)]
pub fn inexactp(value: &SteelVal) -> bool {
    matches!(value, SteelVal::NumV(_))
}

pub struct NumOperations {}
impl NumOperations {
    pub fn arithmetic_shift() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 2 {
                stop!(ArityMismatch => "arithmetic-shift takes 2 arguments")
            }
            let n = args[0].clone();
            let m = args[1].clone();

            match (n, m) {
                (SteelVal::IntV(n), SteelVal::IntV(m)) => {
                    if m >= 0 {
                        Ok(SteelVal::IntV(n << m))
                    } else {
                        Ok(SteelVal::IntV(n >> -m))
                    }
                }
                _ => stop!(TypeMismatch => "arithmetic-shift expected 2 integers"),
            }
        })
    }

    pub fn even() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "even? takes one argument")
            }

            match &args[0] {
                SteelVal::IntV(n) => Ok(SteelVal::BoolV(n & 1 == 0)),
                SteelVal::BigNum(n) => Ok(SteelVal::BoolV(n.is_even())),
                SteelVal::NumV(n) if n.fract() == 0.0 => (*n as i64).is_even().into_steelval(),
                _ => {
                    stop!(TypeMismatch => format!("even? requires an integer, found: {:?}", &args[0]))
                }
            }
        })
    }

    pub fn odd() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "odd? takes one argument")
            }

            match &args[0] {
                SteelVal::IntV(n) => Ok(SteelVal::BoolV(n & 1 == 1)),
                SteelVal::BigNum(n) => Ok(SteelVal::BoolV(n.is_odd())),
                SteelVal::NumV(n) if n.fract() == 0.0 => (*n as i64).is_odd().into_steelval(),
                _ => {
                    stop!(TypeMismatch => format!("odd? requires an integer, found: {:?}", &args[0]))
                }
            }
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
        let got = divide_primitive(&args).unwrap();
        let expected = IntV(5);
        assert_eq!(got.to_string(), expected.to_string());
    }

    #[test]
    fn multiplication_test() {
        let args = vec![IntV(10), IntV(2)];
        let got = multiply_primitive(&args).unwrap();
        let expected = IntV(20);
        assert_eq!(got, expected);
    }

    #[test]
    fn multiplication_different_types() {
        let args = vec![IntV(10), NumV(2.0)];
        let got = multiply_primitive(&args).unwrap();
        let expected = NumV(20.0);
        assert_eq!(got.to_string(), expected.to_string());
    }

    #[test]
    fn addition_different_types() {
        let args = vec![IntV(10), NumV(2.0)];
        let got = add_primitive(&args).unwrap();
        let expected = NumV(12.0);
        assert_eq!(got.to_string(), expected.to_string());
    }

    #[test]
    fn subtraction_different_types() {
        let args = vec![IntV(10), NumV(2.0)];
        let got = subtract_primitive(&args).unwrap();
        let expected = NumV(8.0);
        assert_eq!(got.to_string(), expected.to_string());
    }

    #[test]
    fn test_integer_add() {
        let args = vec![IntV(10), IntV(2)];
        let got = add_primitive(&args).unwrap();
        let expected = IntV(12);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_integer_sub() {
        let args = vec![IntV(10), IntV(2)];
        let got = subtract_primitive(&args).unwrap();
        let expected = IntV(8);
        assert_eq!(got, expected);
    }
}
