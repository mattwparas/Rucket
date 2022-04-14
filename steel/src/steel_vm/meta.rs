// pub type BuiltInSignature = fn(Vec<SteelVal>, &mut dyn VmContext) -> Result<SteelVal>;`

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use im_lists::list::List;

use crate::{parser::ast::ExprKind, rerrs::ErrorKind, rvals::Custom, SteelErr, SteelVal};
use crate::{parser::expander::LocalMacroManager, rvals::Result};
use crate::{parser::parser::ParseError, steel_vm::engine::Engine};

use crate::stop;

#[derive(Clone)]
pub(crate) struct EngineWrapper(Rc<RefCell<Engine>>);

impl EngineWrapper {
    pub(crate) fn new() -> Self {
        EngineWrapper(Rc::new(RefCell::new(Engine::new())))
    }

    // TODO: Warning, here be dragons
    pub(crate) fn call_fn(self, function_name: SteelVal, args: SteelVal) -> Result<SteelVal> {
        let function = match function_name {
            SteelVal::SymbolV(expr) | SteelVal::StringV(expr) => {
                self.0.borrow().extract_value(expr.as_ref())?
            }
            _ => {
                stop!(TypeMismatch => "get-value expected either a string or a symbol, found: {}", function_name)
            }
        };

        if let SteelVal::ListV(list) = args {
            let arguments = list.into_iter().collect();

            println!("Calling with arguments: {:?}", arguments);

            self.0
                .borrow_mut()
                .call_function_with_args(function, arguments)
        } else {
            stop!(TypeMismatch => "call-function-in-env expects a list for the arguments")
        }
    }

    pub(crate) fn call(self, expr: SteelVal) -> Result<List<SteelVal>> {
        match expr {
            SteelVal::StringV(expr) => self
                .0
                .borrow_mut()
                .compile_and_run_raw_program(expr.as_ref())
                .map(|x| x.into()),
            SteelVal::ListV(list) => {
                // let values = list
                //     .iter()
                //     .map(|x| {
                //         ExprKind::try_from(x)
                //             .map_err(|x| SteelErr::new(ErrorKind::ConversionError, x.to_string()))
                //     })
                //     .collect::<Result<Vec<ExprKind>>>()?;

                // println!(
                //     "Expressions: {:#?}",
                //     values // values.iter().map(|x| x.to_string()).collect::<Vec<_>>()
                // );

                // Ok(self
                //     .0
                //     .borrow_mut()
                //     .run_raw_program_from_exprs(values)?
                //     .into_iter()
                //     .collect::<List<_>>()
                //     .into())

                let values = list
                    .iter()
                    .map(|x| x.to_string())
                    .map(|x| {
                        println!("Evaluating: {:?}", x.trim_start_matches('\''));

                        self.0
                            .borrow_mut()
                            .compile_and_run_raw_program(x.trim_start_matches('\''))
                    })
                    .collect::<Result<Vec<Vec<SteelVal>>>>();

                Ok(values?.into_iter().flatten().collect::<List<_>>().into())
            }
            _ => {
                stop!(TypeMismatch => "run! expects either a list of expressions, or a string")
            }
        }
    }

    // TODO: Warning, here be dragons
    pub(crate) fn get_value(self, expr: SteelVal) -> Result<SteelVal> {
        match expr {
            SteelVal::SymbolV(expr) | SteelVal::StringV(expr) => {
                match self.0.borrow().extract_value(expr.as_ref())? {
                    // SteelVal::Closure(_) => {
                    //     stop!(Generic => "a closure cannot be used outside of its defining environment")
                    // }
                    other => Ok(other),
                }
            }
            _ => {
                stop!(TypeMismatch => "get-value expected either a string or a symbol, found: {}", expr)
            }
        }
    }
}

impl Custom for EngineWrapper {}

impl std::fmt::Debug for EngineWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "#<SteelEngine>")
    }
}

// TODO: move this to an iterator utils class
pub fn separate_by<T, F: Fn(&T) -> bool>(
    iter: impl IntoIterator<Item = T>,
    pred: F,
) -> (Vec<T>, Vec<T>) {
    let mut left = Vec::new();
    let mut right = Vec::new();

    for item in iter {
        if pred(&item) {
            left.push(item)
        } else {
            right.push(item)
        }
    }

    (left, right)
}

pub fn read(args: &[SteelVal]) -> Result<SteelVal> {
    if args.len() != 1 {
        stop!(ArityMismatch => "read expects a string as its sole argument")
    }

    assert!(args.len() == 1);

    args[0]
        .string_or_else(throw!(TypeMismatch => "read expects a string"))
        .map(crate::parser::parser::Parser::parse)??
        .into_iter()
        .map(SteelVal::try_from)
        .collect::<Result<List<_>>>()
        .map(SteelVal::ListV)
}

// Takes in a quoted list, put it back to a raw string representation, re-parse it
// back as a typed ast, parse and expand macros, re-emit it _back_ as a list of symbols
pub fn expand_macros(arguments: &[SteelVal]) -> Result<SteelVal> {
    if arguments.len() != 1 {
        stop!(ArityMismatch => "expand! expects a list of quoted expressions as its sole argument")
    }

    assert!(arguments.len() == 1);

    // This should be the '( expressions ... )
    // This should also expand the reader macros as well
    let parsed = arguments[0]
        .list_or_else(throw!(TypeMismatch =>  "expand! expected a list in the first position"))?
        .iter()
        .map(|x| x.to_string())
        // Here we might need to trim the start of the string representation
        // In order to be actually parsable - might be worth doing ExprKind::try_from
        // instead of writing to a string and reparsing directly...
        .map(|x| crate::parser::parser::Parser::parse(x.trim_start_matches('\'')))
        .collect::<std::result::Result<Vec<Vec<_>>, ParseError>>()?;

    // Separate by define-syntax
    let (macros, non_macros) = separate_by(parsed.into_iter().flatten(), |x| {
        matches!(x, ExprKind::Macro(_))
    });

    let macro_manager = LocalMacroManager::from_exprs(macros)?;

    // Expand the macros, convert them back to a consumable expression
    macro_manager
        .expand(non_macros)?
        .into_iter()
        .map(SteelVal::try_from)
        .collect::<Result<List<_>>>()
        .map(SteelVal::ListV)
}

/// Eval with a completely fresh environment
pub fn eval(arguments: &[SteelVal]) -> Result<SteelVal> {
    if arguments.len() != 1 {
        stop!(ArityMismatch => "eval! expects a list of quoted expressions as its sole argument")
    }

    assert!(arguments.len() == 1);

    let mut engine = Engine::new();

    let values = arguments[0]
        .list_or_else(throw!(TypeMismatch =>  "eval! expected a list in the first position"))?
        .iter()
        .map(|x| x.to_string())
        // Here we might need to trim the start of the string representation
        // In order to be actually parsable - might be worth doing ExprKind::try_from
        // instead of writing to a string and reparsing directly...
        .map(|x| engine.compile_and_run_raw_program(x.trim_start_matches('\'')))
        .collect::<Result<Vec<Vec<SteelVal>>>>();

    Ok(values?.into_iter().flatten().collect::<List<_>>().into())
}
