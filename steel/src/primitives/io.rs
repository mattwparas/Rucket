use crate::rerrs::{ErrorKind, SteelErr};
use crate::rvals::{Result, SteelVal};
use crate::stop;
use std::io;
// use std::rc::Rc;

// mod primitives;

pub struct IoFunctions {}
impl IoFunctions {
    pub fn display() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() == 1 {
                let print_val = &args[0];

                match &print_val {
                    SteelVal::StringV(s) => print!("{}", s),
                    _ => print!("{}", print_val),
                }

                // print!("{}", print_val);
                Ok(SteelVal::Void)
            } else {
                stop!(ArityMismatch => "display takes one argument");
            }
        })
    }

    pub fn newline() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.is_empty() {
                println!();
                Ok(SteelVal::Void)
            } else {
                stop!(ArityMismatch => "newline takes no arguments");
            }
        })
    }

    pub fn read_to_string() -> SteelVal {
        SteelVal::FuncV(|_args: &[SteelVal]| -> Result<SteelVal> {
            let mut input_text = String::new();
            io::stdin().read_line(&mut input_text)?;
            Ok(SteelVal::StringV(input_text.trim_end().into()))
        })
    }
}
