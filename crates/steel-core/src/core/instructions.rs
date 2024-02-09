use crate::core::opcode::OpCode;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

use super::labels::Expr;

/// Instruction loaded with lots of information prior to being condensed
/// Includes the opcode and the payload size, plus some information
/// used for locating spans and pretty error messages
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instruction {
    pub op_code: OpCode,
    pub payload_size: usize,
    pub contents: Option<Expr>,
}

impl Instruction {
    pub fn new_from_parts(
        op_code: OpCode,
        payload_size: usize,
        contents: Option<Expr>,
    ) -> Instruction {
        Instruction {
            op_code,
            payload_size,
            contents,
        }
    }
}

// Want to turn a steel struct directly into this struct
// If we values themselves can be mapped
// impl TryFrom<SteelStruct> for Instruction {
//     type Error = crate::SteelErr;

//     fn try_from(value: SteelStruct) -> Result<Self, Self::Error> {
//         if value.name == "Instruction" {

//         }
//     }
// }

pub fn densify(instructions: Vec<Instruction>) -> Vec<DenseInstruction> {
    instructions.into_iter().map(|x| x.into()).collect()
}

pub fn pretty_print_dense_instructions(instrs: &[DenseInstruction]) {
    for (i, instruction) in instrs.iter().enumerate() {
        println!(
            "{}    {:?} : {}",
            i, instruction.op_code, instruction.payload_size
        );
    }
}

pub fn disassemble(instructions: &[Instruction]) -> String {
    let first_column_width = instructions.len().to_string().len();
    let second_column_width = instructions
        .iter()
        .map(|x| format!("{:?}", x.op_code).len())
        .max()
        .unwrap();
    let third_column_width = instructions
        .iter()
        .map(|x| x.payload_size.to_string().len())
        .max()
        .unwrap();

    let mut buffer = String::new();

    for (i, instruction) in instructions.iter().enumerate() {
        let index = i.to_string();

        buffer.push_str(index.as_str());
        for _ in 0..(first_column_width - index.len()) {
            buffer.push(' ');
        }

        buffer.push_str("    ");

        let op_code = format!("{:?}", instruction.op_code);
        buffer.push_str(op_code.as_str());
        for _ in 0..(second_column_width - op_code.len()) {
            buffer.push(' ');
        }

        buffer.push_str(" : ");

        let payload_size = instruction.payload_size.to_string();
        buffer.push_str(payload_size.as_str());
        for _ in 0..(third_column_width - payload_size.len()) {
            buffer.push(' ');
        }

        buffer.push_str("    ");

        if let Some(syn) = instruction.contents.as_ref() {
            match syn {
                Expr::Atom(syn) => {
                    let contents = syn.ty.to_string();
                    buffer.push_str(contents.as_str());
                }
                Expr::List(l) => {
                    let contents = l.to_string();
                    buffer.push_str(contents.as_str());
                }
            }
        }

        buffer.push('\n');
    }

    buffer
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DenseInstruction {
    pub op_code: OpCode,
    // Function IDs need to be interned _again_ before patched into the code?
    // Also: We should be able to get away with a u16 here. Just grab places where u16
    // won't fit and convert to something else.
    pub payload_size: u32,
}

impl DenseInstruction {
    pub fn new(op_code: OpCode, payload_size: u32) -> DenseInstruction {
        DenseInstruction {
            op_code,
            payload_size,
        }
    }
}

// TODO don't actually pass around the span w/ the instruction
// pass around an index into the span to reduce the size of the instructions
// generate an equivalent
impl From<Instruction> for DenseInstruction {
    fn from(val: Instruction) -> DenseInstruction {
        DenseInstruction::new(val.op_code, val.payload_size.try_into().unwrap())
    }
}
