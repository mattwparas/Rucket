use crate::parser::{
    ast::ExprKind,
    interner::InternedString,
    parser::{RawSyntaxObject, SyntaxObject},
    tokens::TokenType,
};
use crate::rvals::Result;
use crate::{
    compiler::constants::ConstantMap,
    core::{instructions::Instruction, opcode::OpCode},
    stop, SteelVal,
};
use crate::{core::instructions::DenseInstruction, parser::span::Span};

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::TryInto, rc::Rc, time::SystemTime};

#[cfg(feature = "profiling")]
use std::time::Instant;

#[cfg(feature = "profiling")]
use log::{debug, log_enabled};

use super::{compiler::DebruijnIndicesInterner, map::SymbolMap};

/// evaluates an atom expression in given environment
fn eval_atom(t: &SyntaxObject) -> Result<SteelVal> {
    match &t.ty {
        TokenType::BooleanLiteral(b) => Ok((*b).into()),
        // TokenType::Identifier(s) => env.borrow().lookup(&s),
        TokenType::NumberLiteral(n) => Ok(SteelVal::NumV(*n)),
        TokenType::StringLiteral(s) => Ok(SteelVal::StringV(s.into())),
        TokenType::CharacterLiteral(c) => Ok(SteelVal::CharV(*c)),
        TokenType::IntegerLiteral(n) => Ok(SteelVal::IntV(*n)),
        // TODO: Keywords shouldn't be misused as an expression - only in function calls are keywords allowed
        TokenType::Keyword(k) => Ok(SteelVal::SymbolV(k.clone().into())),
        what => {
            println!("getting here in the eval_atom - code_generator");
            stop!(UnexpectedToken => what; t.span)
        }
    }
}

// pub fn gimmick_super_instruction(instructions: &mut [Instruction]) {
//     for i in 0..instructions.len() {
//         let read_local = instructions.get(i);
//         let load_int = instructions.get(i + 1);
//         let lte = instructions.get(i + 2);
//         let pass_instr = instructions.get(i + 3);
//         let if_instr = instructions.get(i + 4);

//         match (read_local, load_int, lte, pass_instr, if_instr) {
//             (
//                 Some(Instruction {
//                     op_code: OpCode::READLOCAL,
//                     ..
//                 }),
//                 Some(Instruction {
//                     op_code: OpCode::LOADINT2,
//                     ..
//                 }),
//                 Some(Instruction {
//                     op_code: OpCode::LTE,
//                     ..
//                 }),
//                 Some(Instruction {
//                     op_code: OpCode::PASS,
//                     ..
//                 }),
//                 // HAS to be arity 2 in this case
//                 Some(Instruction {
//                     op_code: OpCode::IF,
//                     ..
//                 }),
//             ) => {
//                 if let Some(x) = instructions.get_mut(i) {
//                     x.op_code = OpCode::GIMMICK;
//                 }

//                 instructions[i + 1].op_code = OpCode::PASS;
//                 instructions[i + 2].op_code = OpCode::PASS;
//                 instructions[i + 4].op_code = OpCode::PASS;
//             }
//             _ => {}
//         }
//     }
// }

// pub fn move_read_local_call_global(instructions: &mut [Instruction]) {
//     for i in 0..instructions.len() {
//         let move_read_local = instructions.get(i);
//         let call_global = instructions.get(i + 1);

//         match (move_read_local, call_global) {
//             (
//                 Some(Instruction {
//                     op_code: OpCode::MOVEREADLOCAL,
//                     ..
//                 }),
//                 Some(Instruction {
//                     op_code: OpCode::CALLGLOBAL,
//                     ..
//                 }),
//             ) => {
//                 if let Some(x) = instructions.get_mut(i) {
//                     x.op_code = OpCode::MOVEREADLOCALCALLGLOBAL;
//                 }
//             }
//             _ => {}
//         }
//     }
// }

// pub fn specialize_exit_jmp(instructions: &mut [Instruction]) {
//     for i in 0..instructions.len() {
//         if let Some(Instruction {
//             op_code: OpCode::JMP,
//             payload_size,
//             ..
//         }) = instructions.get(i)
//         {
//             if let Some((OpCode::POPPURE, payload_size_pop)) = instructions
//                 .get(i + *payload_size)
//                 .map(|x| (x.op_code, x.payload_size))
//             {
//                 let guard = instructions.get_mut(i).unwrap();
//                 guard.op_code = OpCode::POPPURE;
//                 guard.payload_size = payload_size_pop;
//                 continue;
//             }
//         }
//     }
// }

pub fn specialize_read_local(instructions: &mut [Instruction]) {
    for i in 0..instructions.len() {
        let read_local = instructions.get(i);

        match read_local {
            Some(Instruction {
                op_code: OpCode::MOVEREADLOCAL,
                payload_size,
                ..
            }) => {
                let op_code = match payload_size {
                    0 => OpCode::MOVEREADLOCAL0,
                    1 => OpCode::MOVEREADLOCAL1,
                    2 => OpCode::MOVEREADLOCAL2,
                    3 => OpCode::MOVEREADLOCAL3,
                    _ => continue,
                };

                if let Some(x) = instructions.get_mut(i) {
                    x.op_code = op_code;
                }
            }

            Some(Instruction {
                op_code: OpCode::READLOCAL,
                payload_size,
                ..
            }) => {
                let op_code = match payload_size {
                    0 => OpCode::READLOCAL0,
                    1 => OpCode::READLOCAL1,
                    2 => OpCode::READLOCAL2,
                    3 => OpCode::READLOCAL3,
                    _ => continue,
                };

                if let Some(x) = instructions.get_mut(i) {
                    x.op_code = op_code;
                }
            } // instructions[i + 1].op_code = OpCode::PASS;
            // instructions[i + 2].op_code = OpCode::PASS;
            // instructions[i + 4].op_code = OpCode::PASS;
            _ => continue,
        }
    }
}

// Often, there may be a loop condition with something like (= x 10000)
// this identifies these and lazily applies the function, only pushing on to the stack
// until it absolutely needs to
// pub fn loop_condition_local_const_arity_two(instructions: &mut [Instruction]) {
//     for i in 0..instructions.len() {
//         let read_local = instructions.get(i);
//         let push_const = instructions.get(i + 1);
//         let call_global = instructions.get(i + 2);
//         let pass = instructions.get(i + 3);

//         match (read_local, push_const, call_global, pass) {
//             (
//                 Some(Instruction {
//                     op_code: OpCode::READLOCAL,
//                     payload_size: local_idx,
//                     ..
//                 }),
//                 Some(Instruction {
//                     op_code: OpCode::PUSHCONST,
//                     payload_size: const_idx,
//                     ..
//                 }),
//                 Some(Instruction {
//                     op_code: OpCode::CALLGLOBAL,
//                     payload_size: ident,
//                     contents: identifier,
//                     ..
//                 }),
//                 // HAS to be arity 2 in this case
//                 Some(Instruction {
//                     op_code: OpCode::PASS,
//                     payload_size: 2,
//                     ..
//                 }),
//             ) => {
//                 let local_idx = *local_idx;
//                 let const_idx = *const_idx;
//                 let ident = *ident;
//                 let identifier = identifier.clone();

//                 if let Some(x) = instructions.get_mut(i) {
//                     x.op_code = OpCode::CGLOCALCONST;
//                     x.payload_size = ident;
//                     x.contents = identifier;
//                 }

//                 if let Some(x) = instructions.get_mut(i + 1) {
//                     x.op_code = OpCode::READLOCAL;
//                     x.payload_size = local_idx;
//                 }

//                 if let Some(x) = instructions.get_mut(i + 2) {
//                     x.op_code = OpCode::PUSHCONST;
//                     x.payload_size = const_idx;
//                 }
//             }
//             (
//                 Some(Instruction {
//                     op_code: OpCode::MOVEREADLOCAL,
//                     payload_size: local_idx,
//                     ..
//                 }),
//                 Some(Instruction {
//                     op_code: OpCode::PUSHCONST,
//                     payload_size: const_idx,
//                     ..
//                 }),
//                 Some(Instruction {
//                     op_code: OpCode::CALLGLOBAL,
//                     payload_size: ident,
//                     contents: identifier,
//                     ..
//                 }),
//                 // HAS to be arity 2 in this case
//                 Some(Instruction {
//                     op_code: OpCode::PASS,
//                     payload_size: 2,
//                     ..
//                 }),
//             ) => {
//                 let local_idx = *local_idx;
//                 let const_idx = *const_idx;
//                 let ident = *ident;
//                 let identifier = identifier.clone();

//                 if let Some(x) = instructions.get_mut(i) {
//                     x.op_code = OpCode::MOVECGLOCALCONST;
//                     x.payload_size = ident;
//                     x.contents = identifier;
//                 }

//                 if let Some(x) = instructions.get_mut(i + 1) {
//                     x.op_code = OpCode::MOVEREADLOCAL;
//                     x.payload_size = local_idx;
//                 }

//                 if let Some(x) = instructions.get_mut(i + 2) {
//                     x.op_code = OpCode::PUSHCONST;
//                     x.payload_size = const_idx;
//                 }
//             }
//             _ => {}
//         }
//     }
// }

pub fn specialize_constants(instructions: &mut [Instruction]) -> Result<()> {
    if instructions.is_empty() {
        return Ok(());
    }

    for i in 0..instructions.len() {
        match instructions.get(i) {
            Some(Instruction {
                op_code: OpCode::PUSHCONST,
                contents:
                    Some(SyntaxObject {
                        ty: TokenType::Identifier(_),
                        ..
                    }),
                ..
            }) => continue,
            Some(Instruction {
                op_code: OpCode::PUSHCONST,
                contents: Some(syn),
                ..
            }) => {
                let value = eval_atom(syn)?;

                let opcode = match &value {
                    SteelVal::IntV(0) => OpCode::LOADINT0,
                    SteelVal::IntV(1) => OpCode::LOADINT1,
                    SteelVal::IntV(2) => OpCode::LOADINT2,
                    _ => continue,
                };

                instructions.get_mut(i).unwrap().op_code = opcode;
            }
            _ => continue,
        }
    }

    Ok(())
}

pub fn convert_call_globals(instructions: &mut [Instruction]) {
    if instructions.is_empty() {
        return;
    }

    for i in 0..instructions.len() - 1 {
        let push = instructions.get(i);
        let func = instructions.get(i + 1);

        match (push, func) {
            (
                Some(Instruction {
                    op_code: OpCode::PUSH,
                    payload_size: index,
                    ..
                }),
                Some(Instruction {
                    op_code: OpCode::FUNC,
                    payload_size: arity,
                    ..
                }),
            ) => {
                let arity = *arity;
                let index = *index;

                // println!("Converting call global");
                // println!("Arity: {}", arity);
                // println!("Index: {}", index);

                if let Some(x) = instructions.get_mut(i) {
                    x.op_code = OpCode::CALLGLOBAL;
                    x.payload_size = arity;
                }

                if let Some(x) = instructions.get_mut(i + 1) {
                    x.op_code = OpCode::Arity;
                    x.payload_size = index;
                }
            }
            (
                Some(Instruction {
                    op_code: OpCode::PUSH,
                    payload_size: index,
                    ..
                }),
                Some(Instruction {
                    op_code: OpCode::TAILCALL,
                    payload_size: arity,
                    ..
                }),
            ) => {
                let arity = *arity;
                let index = *index;

                if let Some(x) = instructions.get_mut(i) {
                    x.op_code = OpCode::CALLGLOBALTAIL;
                    x.payload_size = arity;
                }

                if let Some(x) = instructions.get_mut(i + 1) {
                    x.op_code = OpCode::Arity;
                    x.payload_size = index;
                }
            }
            _ => {}
        }
    }
}

lazy_static::lazy_static! {
    static ref PLUS: InternedString = "+".into();
    static ref MINUS: InternedString = "-".into();
    static ref DIV: InternedString = "/".into();
    static ref STAR: InternedString = "*".into();
    static ref EQUAL: InternedString = "equal?".into();
    static ref LTE: InternedString = "<=".into();
    pub static ref UNREADABLE_MODULE_GET: InternedString = "##__module-get".into();
    pub static ref STANDARD_MODULE_GET: InternedString = "%module-get%".into();
    pub static ref CONTRACT_OUT: InternedString = "contract/out".into();
    pub static ref PROVIDE: InternedString = "provide".into();
    pub static ref FOR_SYNTAX: InternedString = "for-syntax".into();
    pub static ref DATUM_SYNTAX: InternedString = "datum->syntax".into();
    pub static ref IF: InternedString = "if".into();
    pub static ref DEFINE: InternedString = "define".into();
    pub static ref LET: InternedString = "let".into();
    pub static ref QUOTE: InternedString = "quote".into();
    pub static ref RETURN: InternedString = "return!".into();
    pub static ref REQUIRE: InternedString = "require".into();
    pub static ref SET: InternedString = "set!".into();
    pub static ref LAMBDA: InternedString = "lambda".into();
    pub static ref LAMBDA_SYMBOL: InternedString = "λ".into();
    pub static ref LAMBDA_FN: InternedString = "fn".into();
    pub static ref BEGIN: InternedString = "begin".into();
    pub static ref DOC_MACRO: InternedString = "@doc".into();
    pub static ref REQUIRE_BUILTIN: InternedString = "require-builtin".into();
    pub static ref STRUCT_KEYWORD: InternedString = "struct".into();
    pub static ref AS_KEYWORD: InternedString = "as".into();
    pub static ref SYNTAX_CONST_IF: InternedString = "syntax-const-if".into();
    pub static ref UNQUOTE: InternedString = "unquote".into();
    pub static ref UNQUOTE_SPLICING: InternedString = "unquote-splicing".into();
    pub static ref QUASIQUOTE: InternedString = "quasiquote".into();
}

pub fn inline_num_operations(instructions: &mut [Instruction]) {
    for i in 0..instructions.len() - 1 {
        let push = instructions.get(i);
        let func = instructions.get(i + 1);

        if let (
            Some(Instruction {
                op_code: OpCode::PUSH,
                ..
            }),
            Some(Instruction {
                op_code: OpCode::FUNC | OpCode::TAILCALL,
                contents:
                    Some(RawSyntaxObject {
                        ty: TokenType::Identifier(ident),
                        ..
                    }),
                payload_size,
                ..
            }),
        ) = (push, func)
        {
            let replaced = match *ident {
                x if x == *PLUS && *payload_size == 2 => Some(OpCode::BINOPADD),
                x if x == *PLUS => Some(OpCode::ADD),
                x if x == *MINUS => Some(OpCode::SUB),
                x if x == *DIV => Some(OpCode::DIV),
                x if x == *STAR => Some(OpCode::MUL),
                x if x == *EQUAL => Some(OpCode::EQUAL),
                x if x == *LTE => Some(OpCode::LTE),
                _ => None,
            };

            if let Some(new_op_code) = replaced {
                let payload_size = *payload_size;
                if let Some(x) = instructions.get_mut(i) {
                    x.op_code = new_op_code;
                    x.payload_size = payload_size;
                }

                if let Some(x) = instructions.get_mut(i + 1) {
                    x.op_code = OpCode::PASS;
                }
            }
        }
    }
}

pub fn merge_conditions_with_if(instructions: &mut [Instruction]) {
    for i in 0..instructions.len() - 1 {
        let condition = instructions.get(i);
        let guard = instructions.get(i + 2);

        if let (
            Some(Instruction {
                op_code: OpCode::LTEIMMEDIATE,
                ..
            }),
            Some(Instruction {
                op_code: OpCode::IF,
                ..
            }),
        ) = (condition, guard)
        {
            if let Some(x) = instructions.get_mut(i) {
                x.op_code = OpCode::LTEIMMEDIATEIF;
            }

            // let replaced = match *ident {
            //     x if x == *PLUS && *payload_size == 2 => Some(OpCode::BINOPADD),
            //     x if x == *PLUS => Some(OpCode::ADD),
            //     x if x == *MINUS => Some(OpCode::SUB),
            //     x if x == *DIV => Some(OpCode::DIV),
            //     x if x == *STAR => Some(OpCode::MUL),
            //     x if x == *EQUAL => Some(OpCode::EQUAL),
            //     x if x == *LTE => Some(OpCode::LTE),
            //     _ => None,
            // };

            // if let Some(new_op_code) = replaced {
            //     let payload_size = *payload_size;
            //     if let Some(x) = instructions.get_mut(i) {
            //         x.op_code = new_op_code;
            //         x.payload_size = payload_size;
            //     }

            //     if let Some(x) = instructions.get_mut(i + 1) {
            //         x.op_code = OpCode::PASS;
            //     }
            // }
        }
    }
}

pub struct ProgramBuilder(Vec<Vec<DenseInstruction>>);
impl Default for ProgramBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgramBuilder {
    pub fn new() -> Self {
        ProgramBuilder(Vec::new())
    }

    pub fn push(&mut self, val: Vec<DenseInstruction>) {
        self.0.push(val);
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerializableProgram {
    pub instructions: Vec<Vec<DenseInstruction>>,
    pub constant_map: Vec<u8>,
}

impl SerializableProgram {
    pub fn write_to_file(&self, filename: &str) -> Result<()> {
        use std::io::prelude::*;

        let mut file = File::create(format!("{filename}.txt")).unwrap();

        let buffer = bincode::serialize(self).unwrap();

        file.write_all(&buffer)?;
        Ok(())
    }

    pub fn read_from_file(filename: &str) -> Result<Self> {
        use std::io::prelude::*;

        let mut file = File::open(format!("{filename}.txt")).unwrap();

        let mut buffer = Vec::new();

        let _ = file.read_to_end(&mut buffer).unwrap();

        let program: SerializableProgram = bincode::deserialize(&buffer).unwrap();

        Ok(program)
    }

    pub fn into_program(self) -> Program {
        let constant_map = ConstantMap::from_bytes(&self.constant_map).unwrap();
        Program {
            constant_map,
            instructions: self.instructions,
            ast: HashMap::new(),
        }
    }
}

/// Represents a Steel program
/// The program holds the instructions and the constant map, serialized to bytes
pub struct Program {
    pub instructions: Vec<Vec<DenseInstruction>>,
    pub constant_map: ConstantMap,
    pub ast: HashMap<usize, ExprKind>,
}

impl Program {
    pub fn new(
        instructions: Vec<Vec<DenseInstruction>>,
        constant_map: ConstantMap,
        ast: HashMap<usize, ExprKind>,
    ) -> Self {
        Program {
            instructions,
            constant_map,
            ast,
        }
    }

    pub fn into_serializable_program(self) -> Result<SerializableProgram> {
        Ok(SerializableProgram {
            instructions: self.instructions,
            constant_map: self.constant_map.to_bytes()?,
        })
    }
}

// An inspectable program with debug symbols still included on the instructions
// ConstantMap needs to get passed in to the run time to execute the program
// This way, the VM knows where to look up values
#[derive(Clone)]
pub struct RawProgramWithSymbols {
    // struct_functions: Vec<StructFuncBuilderConcrete>,
    instructions: Vec<Vec<Instruction>>,
    pub(crate) constant_map: ConstantMap,
    version: String, // TODO -> this should be semver
}

#[derive(Serialize, Deserialize)]
pub struct SerializableRawProgramWithSymbols {
    // struct_functions: Vec<StructFuncBuilderConcrete>,
    instructions: Vec<Vec<Instruction>>,
    constant_map: Vec<u8>,
    version: String,
}

impl SerializableRawProgramWithSymbols {
    pub fn write_to_file(&self, filename: &str) -> Result<()> {
        use std::io::prelude::*;

        let mut file = File::create(format!("{filename}.txt")).unwrap();

        let buffer = bincode::serialize(self).unwrap();

        file.write_all(&buffer)?;
        Ok(())
    }

    pub fn read_from_file(filename: &str) -> Result<Self> {
        use std::io::prelude::*;

        let mut file = File::open(format!("{filename}.txt")).unwrap();
        let mut buffer = Vec::new();
        let _ = file.read_to_end(&mut buffer).unwrap();
        let program: Self = bincode::deserialize(&buffer).unwrap();

        Ok(program)
    }

    pub fn into_raw_program(self) -> RawProgramWithSymbols {
        let constant_map = ConstantMap::from_bytes(&self.constant_map).unwrap();
        RawProgramWithSymbols {
            // struct_functions: self.struct_functions,
            instructions: self.instructions,
            constant_map,
            version: self.version,
        }
    }
}

use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
fn _read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

// trait Profiler {
//     #[inline(always)]
//     fn process() -> bool;

//     fn report(&self);
// }

impl RawProgramWithSymbols {
    pub fn new(
        // struct_functions: Vec<StructFuncBuilderConcrete>,
        instructions: Vec<Vec<Instruction>>,
        constant_map: ConstantMap,
        version: String,
    ) -> Self {
        Self {
            // struct_functions,
            instructions,
            constant_map,
            version,
        }
    }

    pub fn profile_instructions(&self) {
        let iter = self
            .instructions
            .iter()
            .flat_map(|x| x.iter())
            .filter(|x| !matches!(x.op_code, OpCode::PASS));

        let mut occurrences = HashMap::new();
        for instr in iter {
            *occurrences.entry(instr.op_code).or_default() += 1;
        }

        let total: usize = occurrences.values().sum();

        let mut counts = occurrences
            .into_iter()
            .map(|x| (x.0, (x.1 as f64 / total as f64) * 100.0))
            .collect::<Vec<(OpCode, f64)>>();

        counts.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());

        println!("{counts:#?}");
    }

    // Definitely can be improved
    // pub fn parse_from_self_hosted_file<P>(file: P) -> Result<Self>
    // where
    //     P: AsRef<Path>,
    // {
    //     let mut lines = read_lines(file)?;

    //     // First line should be the constant map label
    //     // let constant_map =

    //     if let Some(constant_map_label) = lines.next() {
    //         if constant_map_label? != "'ConstantMap" {
    //             stop!(Generic => "Compiled file expected constant map label")
    //         }
    //     } else {
    //         stop!(Generic => "Missing constant map label")
    //     }

    //     // Temportary interner
    //     let mut intern = HashMap::new();

    //     let constant_map = if let Some(constant_map) = lines.next() {
    //         let constant_map = constant_map?;

    //         let constant_map = constant_map
    //             .trim_start_matches('[')
    //             .trim_end_matches(']')
    //             .split(',')
    //             .map(|x| {
    //                 // Parse the input
    //                 let parsed: std::result::Result<Vec<ExprKind>, ParseError> =
    //                     Parser::new(&x, &mut intern).collect();
    //                 let parsed = parsed?;

    //                 Ok(SteelVal::try_from(parsed[0].clone()).unwrap())
    //             })
    //             .collect::<Result<Vec<_>>>()
    //             .map(ConstantMap::from_vec)?;

    //         constant_map
    //     } else {
    //         stop!(Generic => "Missing constant map")
    //     };

    //     if let Some(instructions_label) = lines.next() {
    //         if instructions_label? != "'Instructions" {
    //             stop!(Generic => "Compiled file expected instructions label")
    //         }
    //     } else {
    //         stop!(Generic => "Missing instructions label")
    //     }

    //     let mut instruction_set = Vec::new();

    //     let mut instructions = Vec::new();

    //     // Skip past the first 'Expression
    //     lines.next();

    //     for instruction_string in lines {
    //         let instruction_string = instruction_string?;

    //         if instruction_string == "'Expression" {
    //             // instructions = Vec::new();
    //             // if instruction_set.is_empty() {
    //             instruction_set.push(instructions);
    //             instructions = Vec::new();
    //             // }

    //             continue;
    //         }

    //         let parsed: std::result::Result<Vec<ExprKind>, ParseError> =
    //             Parser::new(&instruction_string, &mut intern).collect();
    //         let parsed = parsed?;

    //         let value = SteelVal::try_from(parsed[0].clone()).unwrap();

    //         if let SteelVal::ListV(v) = value {
    //             // Get the op code here
    //             let op_code =
    //                 OpCode::from_str(v.get(1).unwrap().symbol_or_else(|| unreachable!()).unwrap());

    //             // Get the payload
    //             let payload = v.get(2).unwrap().int_or_else(|| unreachable!()).unwrap() as usize;

    //             // Get the contents
    //             // If I can't parse the object, just move on
    //             let contents = ExprKind::try_from(v.get(3).unwrap())
    //                 .ok()
    //                 .and_then(|x| x.atom_syntax_object().cloned());

    //             let instruction = Instruction::new_from_parts(op_code, payload, contents);

    //             instructions.push(instruction)
    //         } else {
    //             stop!(Generic => "Instruction serialized incorrectly")
    //         }
    //     }

    //     instruction_set.push(instructions);

    //     Ok(Self::new(
    //         instruction_set,
    //         constant_map,
    //         "0.0.1".to_string(),
    //     ))
    // }

    pub fn into_serializable_program(self) -> Result<SerializableRawProgramWithSymbols> {
        Ok(SerializableRawProgramWithSymbols {
            instructions: self.instructions,
            constant_map: self.constant_map.to_bytes()?,
            version: self.version,
        })
    }

    pub fn debug_print(&self) {
        self.instructions
            .iter()
            .for_each(|i| println!("{}\n\n", crate::core::instructions::disassemble(i)))
    }

    /// Applies a peephole style optimization to the underlying instruction set
    pub fn with_optimization<F: Fn(&mut [Instruction])>(&mut self, f: F) {
        for instructions in &mut self.instructions {
            f(instructions)
        }
    }

    // Apply the optimizations to raw bytecode
    pub(crate) fn apply_optimizations(&mut self) -> &mut Self {
        // if std::env::var("CODE_GEN_V2").is_err() {
        // Run down the optimizations here
        for instructions in &mut self.instructions {
            inline_num_operations(instructions);
            convert_call_globals(instructions);

            // gimmick_super_instruction(instructions);
            // move_read_local_call_global(instructions);
            specialize_read_local(instructions);

            merge_conditions_with_if(instructions);

            // specialize_exit_jmp(instructions);

            // loop_condition_local_const_arity_two(instructions);
        }

        self
    }

    pub fn debug_build(mut self, _name: String, symbol_map: &mut SymbolMap) -> Result<()> {
        #[cfg(feature = "profiling")]
        let now = Instant::now();

        // let mut struct_instructions = Vec::new();

        // for builder in &self.struct_functions {
        //     // Add the eventual function names to the symbol map
        //     let indices = symbol_map.insert_struct_function_names_from_concrete(builder);

        //     // Get the value we're going to add to the constant map for eventual use
        //     // Throw the bindings in as well
        //     let constant_values = builder.to_constant_val(indices);
        //     let idx = self.constant_map.add_or_get(constant_values);

        //     struct_instructions.push(vec![Instruction::new_struct(idx), Instruction::new_pop()]);
        // }

        let mut interner = DebruijnIndicesInterner::default();

        for expression in &mut self.instructions {
            interner.collect_first_pass_defines(expression, symbol_map)?
        }

        for expression in &mut self.instructions {
            interner.collect_second_pass_defines(expression, symbol_map)?
        }

        // TODO try here - the loop condition local const arity two seems to rely on the
        // existence of having been already adjusted by the interner
        for instructions in &mut self.instructions {
            // loop_condition_local_const_arity_two(instructions);
            specialize_constants(instructions)?;
        }

        // Put the new struct functions at the front
        // struct_instructions.append(&mut self.instructions);
        // self.instructions = struct_instructions;

        self.instructions
            .iter()
            .for_each(|i| println!("{}\n\n", crate::core::instructions::disassemble(i)));

        #[cfg(feature = "profiling")]
        if log_enabled!(target: "pipeline_time", log::Level::Debug) {
            debug!(target: "pipeline_time", "Executable Build Time: {:?}", now.elapsed());
        }

        // let mut sorted_symbol_map = symbol_map.map.iter().collect::<Vec<_>>();
        // sorted_symbol_map.sort_by_key(|x| x.1);

        // println!("Symbol Map: {:#?}", sorted_symbol_map);

        Ok(())
    }

    // TODO -> check out the spans part of this
    // also look into having the constant map be correct mapping
    // I think the run time will have to swap the constant map in and out
    pub fn build(mut self, name: String, symbol_map: &mut SymbolMap) -> Result<Executable> {
        #[cfg(feature = "profiling")]
        let now = Instant::now();

        let mut interner = DebruijnIndicesInterner::default();

        for expression in &mut self.instructions {
            interner.collect_first_pass_defines(expression, symbol_map)?
        }

        for expression in &mut self.instructions {
            interner.collect_second_pass_defines(expression, symbol_map)?
        }

        // if std::env::var("CODE_GEN_V2").is_err() {
        // TODO try here - the loop condition local const arity two seems to rely on the
        // existence of having been already adjusted by the interner
        for instructions in &mut self.instructions {
            // TODO: Re-enable optimizations
            // loop_condition_local_const_arity_two(instructions);
            specialize_constants(instructions)?;
            // gimmick_super_instruction(instructions);
            // move_read_local_call_global(instructions);
            specialize_read_local(instructions);
        }
        // }

        let (spans, instructions) = extract_spans(self.instructions);

        // let mut sorted_symbol_map = symbol_map.map.iter().collect::<Vec<_>>();
        // sorted_symbol_map.sort_by_key(|x| x.1);

        // println!("Symbol Map: {:#?}", sorted_symbol_map);

        #[cfg(feature = "profiling")]
        if log_enabled!(target: "pipeline_time", log::Level::Debug) {
            debug!(target: "pipeline_time", "Executable Build Time: {:?}", now.elapsed());
        }

        Ok(Executable {
            name: Rc::new(name),
            version: Rc::new(self.version),
            time_stamp: SystemTime::now(),
            instructions: instructions
                .into_iter()
                .map(|x| Rc::from(x.into_boxed_slice()))
                .collect(),
            constant_map: self.constant_map,
            spans,
        })
    }
}

// TODO -> replace spans on instructions with index into span vector
// this is kinda nasty but it _should_ work
fn extract_spans(
    instructions: Vec<Vec<Instruction>>,
) -> (Vec<Rc<[Span]>>, Vec<Vec<DenseInstruction>>) {
    // let mut span_vec = Vec::with_capacity(instructions.iter().map(|x| x.len()).sum());

    // for instruction_set in &instructions {
    //     for instruction in instruction_set {
    //         if let Some(syn) = &instruction.contents {
    //             span_vec.push(syn.span)
    //         } else {
    //             span_vec.push(Span::default())
    //         }
    //     }
    // }

    let span_vec = instructions
        .iter()
        .map(|x| {
            x.iter()
                .map(|x| x.contents.as_ref().map(|x| x.span).unwrap_or_default())
                .collect()
        })
        .collect();

    let instructions: Vec<_> = instructions
        .into_iter()
        .map(|x| {
            // let len = x.len();
            x.into_iter()
                .map(|i| {
                    DenseInstruction::new(
                        i.op_code,
                        i.payload_size.try_into().unwrap_or_else(|_| {
                            // println!("{:?}", len);
                            println!("{:?}", i);
                            panic!("Uh oh ")
                        }),
                    )
                })
                .collect()
        })
        .collect();

    (span_vec, instructions)
}

// A program stripped of its debug symbols, but only constructable by running a pass
// over it with the symbol map to intern all of the symbols in the order they occurred
#[allow(unused)]
#[derive(Clone)]
pub struct Executable {
    pub(crate) name: Rc<String>,
    pub(crate) version: Rc<String>,
    pub(crate) time_stamp: SystemTime, // TODO -> don't use system time, probably not as portable, prefer date time
    pub(crate) instructions: Vec<Rc<[DenseInstruction]>>,
    pub(crate) constant_map: ConstantMap,
    pub(crate) spans: Vec<Rc<[Span]>>,
}

impl Executable {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn time_stamp(&self) -> &SystemTime {
        &self.time_stamp
    }
}
