use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord)]
pub enum OpCode {
    VOID = 0,
    PUSH = 1,
    LOOKUP = 2,
    IF = 3,
    JMP = 4,
    FUNC = 5,
    SCLOSURE = 6,
    ECLOSURE = 7,
    // STRUCT,
    POP,
    BIND,
    SDEF,
    EDEF,
    POP_PURE,
    PASS,
    PUSHCONST,
    NDEFS,
    PANIC,
    CLEAR,
    TAILCALL,
    SET,
    METALOOKUP,
    READLOCAL,
    SETLOCAL,
    READUPVALUE,
    SETUPVALUE,
    FILLUPVALUE,
    COPYCAPTURESTACK,
    COPYCAPTURECLOSURE,
    COPYHEAPCAPTURECLOSURE,
    FIRSTCOPYHEAPCAPTURECLOSURE,
    FILLLOCALUPVALUE,
    CLOSEUPVALUE, // Should be 1 for close, 0 for not
    TCOJMP,
    CALLGLOBAL,
    CALLGLOBALTAIL,
    LOADINT0, // Load const 0
    LOADINT1,
    LOADINT2,
    CGLOCALCONST,
    // INNERSTRUCT,
    MOVEREADLOCAL,
    MOVEREADUPVALUE,
    READCAPTURED,
    MOVECGLOCALCONST,
    BEGINSCOPE,
    ENDSCOPE,
    LETENDSCOPE,
    PUREFUNC,
    FUNC0,
    ADD,
    SUB,
    MUL,
    DIV,
    EQUAL,
    LTE,
    NEWSCLOSURE,
    POPNEW,
    ADDREGISTER,
    SUBREGISTER,
    LTEREGISTER,
    SUBREGISTER1,
    ALLOC,
    READALLOC,
    SETCAPTURED,
    SETALLOC,
}

impl OpCode {
    // TODO better error handling here
    pub fn from_str(s: &str) -> Self {
        use OpCode::*;
        match s {
            "VOID" => VOID,
            "PUSH" => PUSH,
            "LOOKUP" => LOOKUP,
            "IF" => IF,
            "JMP" => JMP,
            "FUNC" => FUNC,
            "SCLOSURE" => SCLOSURE,
            "ECLOSURE" => ECLOSURE,
            // "STRUCT" => STRUCT,
            "POP" => POP,
            "BIND" => BIND,
            "SDEF" => SDEF,
            "EDEF" => EDEF,
            "PASS" => PASS,
            "PUSHCONST" => PUSHCONST,
            "NDEFS" => NDEFS,
            "PANIC" => PANIC,
            "CLEAR" => CLEAR,
            "TAILCALL" => TAILCALL,
            "SET" => SET,
            "METALOOKUP" => METALOOKUP,
            "READLOCAL" => READLOCAL,
            "SETLOCAL" => SETLOCAL,
            "READUPVALUE" => READUPVALUE,
            "SETUPVALUE" => SETUPVALUE,
            "FILLUPVALUE" => FILLUPVALUE,
            "FILLLOCALUPVALUE" => FILLLOCALUPVALUE,
            "CLOSEUPVALUE" => CLOSEUPVALUE, // Should be 1 for close, 0 for not
            "TCOJMP" => TCOJMP,
            "CALLGLOBAL" => CALLGLOBAL,
            "CALLGLOBALTAIL" => CALLGLOBALTAIL,
            "LOADINT0" => LOADINT0, // Load const 0
            "LOADINT1" => LOADINT1,
            "LOADINT2" => LOADINT2,
            "CGLOCALCONST" => CGLOCALCONST,
            // "INNERSTRUCT" => INNERSTRUCT,
            "MOVEREADLOCAL" => MOVEREADLOCAL,
            "MOVEREADUPVALUE" => MOVEREADUPVALUE,
            "MOVECGLOCALCONST" => MOVECGLOCALCONST,
            "BEGINSCOPE" => BEGINSCOPE,
            "ENDSCOPE" => ENDSCOPE,
            "FUNC0" => FUNC0,
            "ADD" => ADD,
            "SUB" => SUB,
            "MUL" => MUL,
            "DIV" => DIV,
            "EQUAL" => EQUAL,
            "LTE" => LTE,
            "LETENDSCOPE" => LETENDSCOPE,
            "PUREFUNC" => PUREFUNC,
            "POP_PURE" => POP_PURE,
            "READCAPTURED" => READCAPTURED,
            "COPYCAPTURESTACK" => COPYCAPTURESTACK,
            "COPYCAPTURECLOSURE" => COPYCAPTURECLOSURE,
            "COPYHEAPCAPTURECLOSURE" => COPYHEAPCAPTURECLOSURE,
            "NEWSCLOSURE" => NEWSCLOSURE,
            "POPNEW" => POPNEW,
            "ADDREGISTER" => ADDREGISTER,
            "SUBREGISTER" => SUBREGISTER,
            "LTEREGISTER" => LTEREGISTER,
            "SUBREGISTER1" => SUBREGISTER1,
            "ALLOC" => ALLOC,
            "READALLOC" => READALLOC,
            "SETALLOC" => SETALLOC,
            "SETCAPTURED" => SETCAPTURED,
            _ => panic!("Unable to map string to opcode"),
        }
    }

    pub fn width(&self) -> usize {
        match self {
            OpCode::VOID => todo!(),
            OpCode::PUSH => todo!(),
            OpCode::LOOKUP => todo!(),
            OpCode::IF => todo!(),
            OpCode::JMP => todo!(),
            OpCode::FUNC => todo!(),
            OpCode::SCLOSURE => todo!(),
            OpCode::ECLOSURE => todo!(),
            // OpCode::STRUCT => todo!(),
            OpCode::POP => todo!(),
            OpCode::BIND => todo!(),
            OpCode::SDEF => todo!(),
            OpCode::EDEF => todo!(),
            OpCode::POP_PURE => todo!(),
            OpCode::PASS => todo!(),
            OpCode::PUSHCONST => todo!(),
            OpCode::NDEFS => todo!(),
            OpCode::PANIC => todo!(),
            OpCode::CLEAR => todo!(),
            OpCode::TAILCALL => todo!(),
            OpCode::SET => todo!(),
            OpCode::METALOOKUP => todo!(),
            OpCode::READLOCAL => todo!(),
            OpCode::SETLOCAL => todo!(),
            OpCode::READUPVALUE => todo!(),
            OpCode::SETUPVALUE => todo!(),
            OpCode::FILLUPVALUE => todo!(),
            OpCode::COPYCAPTURESTACK => todo!(),
            OpCode::COPYCAPTURECLOSURE => todo!(),
            OpCode::FILLLOCALUPVALUE => todo!(),
            OpCode::CLOSEUPVALUE => todo!(),
            OpCode::TCOJMP => todo!(),
            OpCode::CALLGLOBAL => 2,
            OpCode::CALLGLOBALTAIL => todo!(),
            OpCode::LOADINT0 => todo!(),
            OpCode::LOADINT1 => todo!(),
            OpCode::LOADINT2 => todo!(),
            OpCode::CGLOCALCONST => todo!(),
            // OpCode::INNERSTRUCT => todo!(),
            OpCode::MOVEREADLOCAL => todo!(),
            OpCode::MOVEREADUPVALUE => todo!(),
            OpCode::READCAPTURED => todo!(),
            OpCode::MOVECGLOCALCONST => todo!(),
            OpCode::BEGINSCOPE => todo!(),
            OpCode::ENDSCOPE => todo!(),
            OpCode::LETENDSCOPE => todo!(),
            OpCode::PUREFUNC => todo!(),
            OpCode::FUNC0 => todo!(),
            OpCode::ADD => 2,
            OpCode::SUB => todo!(),
            OpCode::MUL => todo!(),
            OpCode::DIV => todo!(),
            OpCode::EQUAL => todo!(),
            OpCode::LTE => todo!(),
            OpCode::NEWSCLOSURE => todo!(),
            OpCode::POPNEW => 1,
            OpCode::ADDREGISTER => 2,
            OpCode::SUBREGISTER => 2,
            OpCode::LTEREGISTER => 2,
            OpCode::SUBREGISTER1 => todo!(),
            OpCode::ALLOC => todo!(),
            _ => todo!(),
        }
    }
}
