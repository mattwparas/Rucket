use crate::env::VOID;
use crate::rerrs::SteelErr;
use crate::rvals::{Result, SteelVal};
use crate::stop;
use std::io;
use std::rc::Rc;

// mod primitives;

pub struct IoFunctions {}
impl IoFunctions {
    pub fn display() -> SteelVal {
        SteelVal::FuncV(|args: &[Rc<SteelVal>]| -> Result<Rc<SteelVal>> {
            if args.len() == 1 {
                let print_val = (*args[0]).clone();

                match &print_val {
                    SteelVal::StringV(s) => print!("{}", s),
                    _ => print!("{}", print_val),
                }

                // print!("{}", print_val);
                Ok(VOID.with(|f| Rc::clone(f)))
            } else {
                stop!(ArityMismatch => "display takes one argument");
            }
        })
    }

    pub fn newline() -> SteelVal {
        SteelVal::FuncV(|args: &[Rc<SteelVal>]| -> Result<Rc<SteelVal>> {
            if args.is_empty() {
                println!();
                Ok(VOID.with(|f| Rc::clone(f)))
            } else {
                stop!(ArityMismatch => "newline takes no arguments");
            }
        })
    }

    pub fn read_to_string() -> SteelVal {
        SteelVal::FuncV(|_args: &[Rc<SteelVal>]| -> Result<Rc<SteelVal>> {
            let mut input_text = String::new();
            io::stdin().read_line(&mut input_text)?;
            Ok(Rc::new(SteelVal::StringV(
                input_text.trim_end().to_string(),
            )))
        })
    }

    // pub fn read_file_to_string() -> SteelVal {
    //     SteelVal::FuncV(|args: Vec<Rc<SteelVal>>| -> Result<Rc<SteelVal>> {
    //         let mut
    //     })
    // }
}
