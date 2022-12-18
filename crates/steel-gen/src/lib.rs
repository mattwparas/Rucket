// TODO: Create stack to ssa representation of the op codes, via macros

pub mod opcode;
use std::borrow::Cow;

pub use opcode::OpCode;

use codegen::{Block, Function, Scope};

macro_rules! opcode_to_function {
    (VOID) => {
        void_handler
    };
    (PUSH) => {
        push_handler
    };
    (FUNC) => {
        func_handler
    };
    (BIND) => {
        bind_handler
    };
    (PUSHCONST) => {
        push_const_handler
    };
    (PANIC) => {
        panic_handler
    };
    (SET) => {
        set_handler
    };
    (READLOCAL0) => {
        local_handler0
    };
    (LOADINT2) => {
        handle_load_int2
    };
    (LTE) => {
        lte_handler
    };
    (MOVEREADLOCAL0) => {
        move_local_handler0
    };
    (SUB) => {
        sub_handler
    };
    (LOADINT1) => {
        handle_load_int1
    };
    (MUL) => {
        multiply_handler
    };
    (MOVEREADLOCAL1) => {
        move_local_handler1
    };
    (READLOCAL1) => {
        local_handler1
    };
    (READLOCAL2) => {
        local_handler2
    };
    (READLOCAL3) => {
        local_handler3
    };
    (LOADINT0) => {
        handle_load_int0
    };
    (CALLGLOBAL) => {
        call_global_handler
    };
    (READCAPTURED) => {
        read_captured_handler
    };
    (IF) => {
        if_handler
    };
    (EQUAL) => {
        equality_handler
    };
    (JMP) => {
        jump_handler
    };
    (ADD) => {
        add_handler
    };
    (TAILCALL) => {
        tail_call_handler
    };
}

// If the op code requires the original payload from the instruction that we're overwriting, we should
// attach it to the basic block, because otherwise we'll have lost the payload
// fn op_code_requires_payload(op_code: OpCode) -> Option<&'static str> {
//     match op_code {
//         OpCode::VOID => None,
//         OpCode::PUSH => Some("push_handler_with_payload"),
//         OpCode::IF => todo!(),
//         OpCode::JMP => todo!(),
//         OpCode::FUNC => Some("func_handler_with_payload"),
//         OpCode::SCLOSURE => todo!(),
//         OpCode::ECLOSURE => todo!(),
//         OpCode::BIND => Some("bind_handler_with_payload"),
//         OpCode::SDEF => todo!(),
//         OpCode::EDEF => todo!(),
//         OpCode::POPPURE => todo!(),
//         OpCode::PASS => todo!(),
//         OpCode::PUSHCONST => Some("push_const_handler_with_payload"),
//         OpCode::NDEFS => todo!(),
//         OpCode::PANIC => None,
//         OpCode::TAILCALL => todo!(),
//         OpCode::SET => Some("set_handler_with_payload"),
//         OpCode::READLOCAL => Some("local_handler_with_payload"),
//         OpCode::READLOCAL0 => None,
//         OpCode::READLOCAL1 => None,
//         OpCode::READLOCAL2 => None,
//         OpCode::READLOCAL3 => None,
//         OpCode::SETLOCAL => Some("set_local_handler_with_payload"),
//         OpCode::COPYCAPTURESTACK => todo!(),
//         OpCode::COPYCAPTURECLOSURE => todo!(),
//         OpCode::COPYHEAPCAPTURECLOSURE => todo!(),
//         OpCode::FIRSTCOPYHEAPCAPTURECLOSURE => todo!(),
//         OpCode::TCOJMP => todo!(),
//         OpCode::CALLGLOBAL => Some("call_global_handler_with_payload"),
//         OpCode::CALLGLOBALTAIL => todo!(),
//         OpCode::LOADINT0 => None,
//         OpCode::LOADINT1 => None,
//         OpCode::LOADINT2 => None,
//         OpCode::CGLOCALCONST => todo!(),
//         OpCode::MOVEREADLOCAL => Some("move_local_handler_with_payload"),
//         OpCode::MOVEREADLOCAL0 => None,
//         OpCode::MOVEREADLOCAL1 => None,
//         OpCode::MOVEREADLOCAL2 => None,
//         OpCode::MOVEREADLOCAL3 => None,
//         OpCode::READCAPTURED => Some("read_captured_handler_with_payload"),
//         OpCode::MOVECGLOCALCONST => todo!(),
//         OpCode::BEGINSCOPE => None,
//         OpCode::LETENDSCOPE => Some("let_end_scope_handler_with_payload"),
//         OpCode::PUREFUNC => Some("pure_function_handler_with_payload"),
//         OpCode::ADD => Some("add_handler_payload"),
//         OpCode::SUB => Some("sub_handler_payload"),
//         OpCode::MUL => Some("multiply_handler_payload"),
//         OpCode::DIV => Some("division_handler_payload"),
//         OpCode::EQUAL => Some("equality_handler_payload"),
//         OpCode::LTE => Some("lte_handler_payload"),
//         OpCode::NEWSCLOSURE => Some("new_sclosure_handler_with_payload"),
//         OpCode::ADDREGISTER => todo!(),
//         OpCode::SUBREGISTER => todo!(),
//         OpCode::LTEREGISTER => todo!(),
//         OpCode::SUBREGISTER1 => todo!(),
//         OpCode::ALLOC => None,
//         OpCode::READALLOC => Some("read_alloc_handler_with_payload"),
//         OpCode::SETALLOC => Some("set_alloc_handler_with_payload"),
//         // OpCode::GIMMICK => todo!(),
//         // OpCode::MOVEREADLOCALCALLGLOBAL => Some(move_read_local_call_global_handler_payload),
//         OpCode::DynSuperInstruction => todo!(),
//         _ => None,
//     }
// }

// #[repr(u8)]
// #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
// pub enum OpCode {
//     VOID = 0,
//     PUSH = 1,
//     IF = 2,
//     JMP = 3,
//     FUNC = 4,
//     SCLOSURE = 5,
//     ECLOSURE = 6,
//     BIND,
//     SDEF,
//     EDEF,
//     POPPURE,
//     PASS,
//     PUSHCONST,
//     NDEFS,
//     PANIC,
//     TAILCALL,
//     SET,
//     READLOCAL,
//     READLOCAL0,
//     READLOCAL1,
//     READLOCAL2,
//     READLOCAL3,
//     SETLOCAL,
//     COPYCAPTURESTACK,
//     COPYCAPTURECLOSURE,
//     COPYHEAPCAPTURECLOSURE,
//     FIRSTCOPYHEAPCAPTURECLOSURE,
//     TCOJMP,
//     CALLGLOBAL,
//     CALLGLOBALTAIL,
//     LOADINT0, // Load const 0
//     LOADINT1,
//     LOADINT2,
//     CGLOCALCONST,
//     MOVEREADLOCAL,
//     MOVEREADLOCAL0,
//     MOVEREADLOCAL1,
//     MOVEREADLOCAL2,
//     MOVEREADLOCAL3,
//     READCAPTURED,
//     MOVECGLOCALCONST,
//     BEGINSCOPE,
//     LETENDSCOPE,
//     PUREFUNC,
//     ADD,
//     SUB,
//     MUL,
//     DIV,
//     EQUAL,
//     LTE,
//     NEWSCLOSURE,
//     ADDREGISTER,
//     SUBREGISTER,
//     LTEREGISTER,
//     SUBREGISTER1,
//     ALLOC,
//     READALLOC,
//     SETALLOC,
//     // GIMMICK,
//     // MOVEREADLOCALCALLGLOBAL,
//     DynSuperInstruction,
//     Arity,
// }

// If we can provide hints on the types, this can help with constant folding of operations
// that we know the types of
//
// For instance, this can also allow specializing _lots_ of the list operations if we can pull it off
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TypeHint {
    Int,
    Bool,
    Float,
    Void,
    None,
}

#[derive(Clone, Debug)]
struct LocalVariable {
    id: u16,
    type_hint: TypeHint,
}

impl LocalVariable {
    pub fn kind(mut self, type_hint: TypeHint) -> Self {
        self.type_hint = type_hint;
        self
    }
}

struct GenSym {
    prefix: &'static str,
    count: u16,
}

impl GenSym {
    pub fn new() -> Self {
        Self {
            prefix: "x",
            count: 0,
        }
    }

    // Generate a fresh variable
    fn fresh(&mut self) -> LocalVariable {
        let count = self.count;
        self.count += 1;
        LocalVariable {
            id: count,
            type_hint: TypeHint::None,
        }
    }
}

impl std::fmt::Display for LocalVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "x{}", self.id)
    }
}

fn op_code_to_handler(op_code: Pattern) -> String {
    format!("opcode_to_ssa_handler!({})", op_code)
}

struct StackToSSAConverter {
    generator: GenSym,
    stack: Vec<LocalVariable>,
    local_offset: Option<usize>,
}

#[derive(Clone, Debug, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pattern {
    Single(OpCode),
    Double(OpCode, usize),
}

impl std::fmt::Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Pattern::Single(op) => write!(f, "{:?}", op),
            Pattern::Double(op, payload) => write!(f, "{:?}{}", op, payload),
        }
    }
}

impl StackToSSAConverter {
    pub fn new() -> Self {
        Self {
            generator: GenSym::new(),
            stack: Vec::new(),
            local_offset: None,
        }
    }

    pub fn reset(&mut self) {
        self.generator.count = 0;
        self.stack.clear();
        self.local_offset = None;
    }

    // Get the current local offset
    // pub fn local_offset(&self) -> usize {
    //     self.local_offset
    // }

    pub fn push(&mut self) -> LocalVariable {
        let var = self.generator.fresh();
        self.stack.push(var.clone());
        var
    }

    pub fn push_with_hint(&mut self, type_hint: TypeHint) -> LocalVariable {
        let var = self.generator.fresh().kind(type_hint);
        self.stack.push(var.clone());
        var
    }

    pub fn pop(&mut self) -> LocalVariable {
        self.stack.pop().unwrap()
    }

    pub fn process_sequence(&mut self, op_codes: &[Pattern]) -> Function {
        use OpCode::*;
        use Pattern::*;

        // let mut scope = Scope::new();
        let mut function = Function::new(
            op_codes
                .iter()
                .map(|x| format!("{}", x))
                .collect::<String>(),
        );
        function.arg("ctx", codegen::Type::new("&mut VmCore<'_>"));
        function.arg("payload", codegen::Type::new("usize"));
        function.ret(codegen::Type::new("Result<()>"));

        // READLOCAL0,
        // LOADINT2,
        // MUL,
        // MOVEREADLOCAL1,
        // LOADINT1,
        // SUB,
        // READLOCAL2,
        // LOADINT1,
        // SUB,
        // READLOCAL3,
        // CALLGLOBAL

        let last = op_codes.len();

        for (index, op) in op_codes.iter().enumerate() {
            match op {
                Double(BEGINSCOPE, n) => {
                    self.local_offset = Some(*n);
                    function.line("ctx.ip += 1;");
                }
                // Single(LetVar) => {
                //     let local = self.pop();
                //     function.line(format!("ctx.stack.push({}.into());", local));
                // }
                Single(VOID) => {
                    let local = self.push_with_hint(TypeHint::Void);
                    function.line(format!("let {} = SteelVal::Void;", local));
                    function.line("ctx.ip += 1;");
                }
                Single(LOADINT0) => {
                    let local = self.push_with_hint(TypeHint::Int);
                    // Load the immediate for 0
                    function.line(format!("let {} = {};", local, 0));
                    function.line("ctx.ip += 1;");
                }
                Single(LOADINT1) => {
                    let local = self.push_with_hint(TypeHint::Int);
                    // Load the immediate for 1
                    function.line(format!("let {} = {};", local, 1));
                    function.line("ctx.ip += 1;");
                }
                Single(LOADINT2) => {
                    let local = self.push_with_hint(TypeHint::Int);
                    // Load the immediate for 2
                    function.line(format!("let {} = {};", local, 2));
                    function.line("ctx.ip += 1;");
                }
                Double(CALLGLOBAL, n) => {
                    // println!("Stack: {:?}", self.stack);

                    let args = self
                        .stack
                        .split_off(self.stack.len() - n)
                        .into_iter()
                        .map(|x| x.to_string() + ".into(), ")
                        .collect::<String>();

                    if index == last - 1 {
                        function.line(format!(
                            "opcode_to_ssa_handler!(CALLGLOBAL, Tail)(ctx, &mut [{}])?;",
                            args
                        ));
                    } else {
                        let local = self.push();

                        function.line(format!(
                            "let {} = opcode_to_ssa_handler!(CALLGLOBAL)(ctx, &mut [{}])?;",
                            local, args
                        ));
                    }
                }
                Double(READLOCAL, n) => {
                    if self.local_offset.is_none()
                        || self.local_offset.is_some() && self.local_offset.unwrap() > *n
                    {
                        let local = self.push();
                        function.line(format!(
                            "let {} = {}(ctx)?;",
                            local,
                            op_code_to_handler(*op)
                        ));
                    } else {
                        let local = self.push();
                        let var = self.stack.get(*n).unwrap();
                        function.line(format!("let {} = {}.clone();", local, var));
                    }
                }
                Single(READLOCAL0) => {
                    // If we're dealing with a let var, then we can only specialize it _iff_ the
                    // value we're reading exceeds the current function arguments.
                    // For instance, if we have a function like this:
                    // (lambda (x y z) (let ((foo 10)) (+ x y z foo))
                    // Then foo should be something like READLOCAL(4)
                    // In this case, we can safely snag it from the Rust stack
                    // Otherwise, we need to read it from the steel stack.
                    if self.local_offset.is_none()
                        || self.local_offset.is_some() && self.local_offset.unwrap() > 0
                    {
                        let local = self.push();
                        function.line(format!(
                            "let {} = {}(ctx)?;",
                            local,
                            op_code_to_handler(*op)
                        ));
                    } else {
                        let local = self.push();
                        let var = self.stack.get(0).unwrap();
                        function.line(format!("let {} = {}.clone();", local, var));
                    }
                }
                Single(READLOCAL1) => {
                    if self.local_offset.is_none()
                        || self.local_offset.is_some() && self.local_offset.unwrap() > 1
                    {
                        let local = self.push();
                        function.line(format!(
                            "let {} = {}(ctx)?;",
                            local,
                            op_code_to_handler(*op)
                        ));
                    } else {
                        let local = self.push();
                        let var = self.stack.get(1).unwrap();
                        function.line(format!("let {} = {}.clone();", local, var));
                    }
                }
                Single(READLOCAL2) => {
                    if self.local_offset.is_none()
                        || self.local_offset.is_some() && self.local_offset.unwrap() > 2
                    {
                        let local = self.push();
                        function.line(format!(
                            "let {} = {}(ctx)?;",
                            local,
                            op_code_to_handler(*op)
                        ));
                    } else {
                        let local = self.push();
                        let var = self.stack.get(2).unwrap();
                        function.line(format!("let {} = {}.clone();", local, var));
                    }
                }
                Single(READLOCAL3) => {
                    if self.local_offset.is_none()
                        || self.local_offset.is_some() && self.local_offset.unwrap() > 3
                    {
                        let local = self.push();
                        function.line(format!(
                            "let {} = {}(ctx)?;",
                            local,
                            op_code_to_handler(*op)
                        ));
                    } else {
                        let local = self.push();
                        let var = self.stack.get(3).unwrap();
                        function.line(format!("let {} = {}.clone();", local, var));
                    }
                }
                Double(MOVEREADLOCAL, n) => {
                    if self.local_offset.is_none()
                        || self.local_offset.is_some() && self.local_offset.unwrap() > *n
                    {
                        let local = self.push();
                        function.line(format!(
                            "let {} = {}(ctx)?;",
                            local,
                            op_code_to_handler(*op)
                        ));
                    } else {
                        let local = self.push();
                        let var = self.stack.get(*n).unwrap();
                        function.line(format!("let {} = {};", local, var));
                    }
                }
                Single(MOVEREADLOCAL0) => {
                    // If we're dealing with a let var, then we can only specialize it _iff_ the
                    // value we're reading exceeds the current function arguments.
                    // For instance, if we have a function like this:
                    // (lambda (x y z) (let ((foo 10)) (+ x y z foo))
                    // Then foo should be something like READLOCAL(4)
                    // In this case, we can safely snag it from the Rust stack
                    // Otherwise, we need to read it from the steel stack.
                    if self.local_offset.is_none()
                        || self.local_offset.is_some() && self.local_offset.unwrap() > 0
                    {
                        let local = self.push();
                        function.line(format!(
                            "let {} = {}(ctx)?;",
                            local,
                            op_code_to_handler(*op)
                        ));
                    } else {
                        let local = self.push();
                        let var = self.stack.get(0).unwrap();
                        function.line(format!("let {} = {};", local, var));
                    }
                }
                Single(MOVEREADLOCAL1) => {
                    if self.local_offset.is_none()
                        || self.local_offset.is_some() && self.local_offset.unwrap() > 1
                    {
                        let local = self.push();
                        function.line(format!(
                            "let {} = {}(ctx)?;",
                            local,
                            op_code_to_handler(*op)
                        ));
                    } else {
                        let local = self.push();
                        let var = self.stack.get(1).unwrap();
                        function.line(format!("let {} = {};", local, var));
                    }
                }
                Single(MOVEREADLOCAL2) => {
                    if self.local_offset.is_none()
                        || self.local_offset.is_some() && self.local_offset.unwrap() > 2
                    {
                        let local = self.push();
                        function.line(format!(
                            "let {} = {}(ctx)?;",
                            local,
                            op_code_to_handler(*op)
                        ));
                    } else {
                        let local = self.push();
                        let var = self.stack.get(2).unwrap();
                        function.line(format!("let {} = {};", local, var));
                    }
                }
                Single(MOVEREADLOCAL3) => {
                    if self.local_offset.is_none()
                        || self.local_offset.is_some() && self.local_offset.unwrap() > 3
                    {
                        let local = self.push();
                        function.line(format!(
                            "let {} = {}(ctx)?;",
                            local,
                            op_code_to_handler(*op)
                        ));
                    } else {
                        let local = self.push();
                        let var = self.stack.get(3).unwrap();
                        function.line(format!("let {} = {};", local, var));
                    }
                }
                Single(PUSH | READCAPTURED | TAILCALL | CALLGLOBAL) => {
                    let local = self.push();
                    function.line(format!(
                        "let {} = {}(ctx)?;",
                        local,
                        op_code_to_handler(*op)
                    ));
                }
                // Single(PUSH) => {
                //     // Consider embedding some type hints on these for even more specialization if possible
                //     let local = self.push();
                //     function.line(format!("{} = {}(ctx)?;", local, op_code_to_handler(*op)));
                // }
                // Single(READCAPTURED) => {
                //     let local = self.push();
                //     function.line(format!("{} = {}(ctx)?;", local, op_code_to_handler(*op)));
                // }
                Single(IF) => {
                    // (if <test> <then> <else>)
                    let test_condition = self.pop();

                    // If we're dealing with an int, just unbox it directly
                    match test_condition.type_hint {
                        TypeHint::Bool => {
                            function.line(format!(
                                "if_to_ssa_handler!(IF, Bool)(ctx, {});",
                                test_condition
                            ));
                        }
                        _ => {
                            function.line(format!(
                                "if_to_ssa_handler!(IF)(ctx, {}.into());",
                                test_condition
                            ));
                        }
                    }
                }
                Double(LTE | EQUAL, 2) => {
                    let right = self.pop();
                    let left = self.pop();

                    function.line("ctx.ip += 2;");

                    match (left.type_hint, right.type_hint) {
                        (a, b) if a == b => {
                            let local = self.push_with_hint(TypeHint::Bool);

                            function.line(format!("let {} = {} == {};", local, left, right));
                            function.line("ctx.ip += 1;");
                        }
                        (TypeHint::Int, TypeHint::Int) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, Int, Int)", op);

                            let local = self.push_with_hint(TypeHint::Bool);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::Int, TypeHint::Float) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, Int, Float)", op);

                            let local = self.push_with_hint(TypeHint::Bool);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::Int, TypeHint::None) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, Int, None)", op);

                            let local = self.push_with_hint(TypeHint::Bool);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::Float, TypeHint::Int) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, Float, Int)", op);

                            let local = self.push_with_hint(TypeHint::Bool);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::Float, TypeHint::Float) => {
                            // Delegate to the binary handler to return an int
                            let call =
                                format!("binop_opcode_to_ssa_handler!({}, Float, Float)", op);

                            let local = self.push_with_hint(TypeHint::Bool);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::Float, TypeHint::None) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, Float, None)", op);

                            // Probably needs to be promoted to a float in this case
                            let local = self.push_with_hint(TypeHint::Bool);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::None, TypeHint::Int) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, None, Int)", op);

                            // Probably needs to be promoted to a float in this case
                            let local = self.push_with_hint(TypeHint::Bool);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::None, TypeHint::Float) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, None, Int)", op);

                            // Probably needs to be promoted to a float in this case
                            let local = self.push_with_hint(TypeHint::Bool);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (_, _) => todo!(),
                    }
                }

                // TODO: Need to handle the actual op code as well
                // READLOCAL0, LOADINT2, LTE, IF
                Double(ADD | MUL | SUB | DIV, 2) => {
                    let right = self.pop();
                    let left = self.pop();

                    function.line("ctx.ip += 2;");

                    match (left.type_hint, right.type_hint) {
                        (TypeHint::Int, TypeHint::Int) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, Int, Int)", op);

                            let local = self.push_with_hint(TypeHint::Int);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::Int, TypeHint::Float) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, Int, Float)", op);

                            let local = self.push_with_hint(TypeHint::Float);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::Int, TypeHint::None) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, Int, None)", op);

                            let local = self.push_with_hint(TypeHint::None);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::Float, TypeHint::Int) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, Float, Int)", op);

                            let local = self.push_with_hint(TypeHint::Float);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::Float, TypeHint::Float) => {
                            // Delegate to the binary handler to return an int
                            let call =
                                format!("binop_opcode_to_ssa_handler!({}, Float, Float)", op);

                            let local = self.push_with_hint(TypeHint::Float);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::Float, TypeHint::None) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, Float, None)", op);

                            // Probably needs to be promoted to a float in this case
                            let local = self.push_with_hint(TypeHint::Float);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::None, TypeHint::Int) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, None, Int)", op);

                            // Probably needs to be promoted to a float in this case
                            let local = self.push_with_hint(TypeHint::None);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (TypeHint::None, TypeHint::Float) => {
                            // Delegate to the binary handler to return an int
                            let call = format!("binop_opcode_to_ssa_handler!({}, None, Float)", op);

                            // Probably needs to be promoted to a float in this case
                            let local = self.push_with_hint(TypeHint::Float);

                            push_binop(&mut function, local, call, left, right);
                        }
                        (_, _) => todo!(),
                    }
                }
                _ => {
                    todo!()
                }
            }
        }

        // For whatever is left, push on to the SteelThread stack
        for value in &self.stack {
            match value.type_hint {
                TypeHint::Int | TypeHint::Bool | TypeHint::Float => {
                    function.line(format!("ctx.stack.push({}.into());", value))
                }
                // It is already confirmed to be... something thats non primitive.
                _ => function.line(format!("ctx.stack.push({});", value)),
            };
        }

        function.line("Ok(())");
        // scope.push_fn(function);

        // scope.to_string()

        function
    }
}

fn push_binop(
    function: &mut Function,
    local: LocalVariable,
    call: String,
    left: LocalVariable,
    right: LocalVariable,
) {
    function.line(format!(
        "let {} = {}",
        local,
        Call::new(
            call.into(),
            // op_code_to_handler(*op).into(),
            vec![
                "ctx".into(),
                left.to_string().into(),
                right.to_string().into()
            ],
        )
        .to_string(),
    ));
}

struct Call<'a> {
    name: Cow<'a, str>,
    args: Vec<Cow<'a, str>>,
}

impl<'a> Call<'a> {
    pub fn new(name: Cow<'a, str>, args: Vec<Cow<'a, str>>) -> Self {
        Self {
            name: name.into(),
            args,
        }
    }
}

impl<'a> std::fmt::Display for Call<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        write!(f, "(")?;
        for arg in &self.args {
            write!(f, "{}, ", arg)?;
        }
        write!(f, ")?;")
    }
}

// fn call_function(name: &str, args: &[&str]) -> String {
//     // format!("{}({})")

//     let mut builder = name.to_string() + "(";

// }

// READCAPTURED, TAILCALL

impl Pattern {
    pub fn from_opcodes(op_codes: &[(OpCode, usize)]) -> Vec<Pattern> {
        use OpCode::*;

        let mut patterns: Vec<Pattern> = Vec::new();
        let mut iter = op_codes.iter();

        while let Some(op) = iter.next() {
            match op {
                (
                    LOADINT0 | LOADINT1 | LOADINT2 | READLOCAL0 | READLOCAL1 | READLOCAL2
                    | READLOCAL3 | MOVEREADLOCAL0 | MOVEREADLOCAL1 | MOVEREADLOCAL2
                    | MOVEREADLOCAL3 | IF | PUSH | READCAPTURED | TAILCALL,
                    _,
                ) => patterns.push(Pattern::Single(op.0)),
                (READLOCAL | MOVEREADLOCAL, n) => patterns.push(Pattern::Double(op.0, *n)),
                (ADD | SUB | MUL | DIV | EQUAL | LTE, 2) => patterns.push(Pattern::Double(op.0, 2)),
                (BEGINSCOPE, n) => patterns.push(Pattern::Double(op.0, *n)),
                (CALLGLOBAL, n) => {
                    // let arity = iter.next().unwrap();
                    patterns.push(Pattern::Double(CALLGLOBAL, *n))
                }
                _ => {
                    continue;
                }
            }
        }

        return patterns;
    }
}

// struct SuperInstructionMap {
//     map: std::collections::HashMap<Vec<Pattern>, for<'r> fn (&'r mut VmCore<'_>, usize) -> Result<()>>
// }

pub fn generate_opcode_map(patterns: Vec<Vec<(OpCode, usize)>>) -> String {
    use OpCode::*;

    let mut global_scope = Scope::new();

    let mut generate = codegen::Function::new("generate_dynamic_op_codes");
    generate.ret(codegen::Type::new("SuperInstructionMap"));

    generate.line("use OpCode::*;");
    generate.line("use steel_gen::Pattern::*;");

    generate.line("let mut map = SuperInstructionMap::new();");

    let mut converter = StackToSSAConverter::new();

    for pattern in patterns {
        let pattern = Pattern::from_opcodes(&pattern);
        let generated_name = pattern.iter().map(|x| format!("{}", x)).collect::<String>();
        let generated_function = converter.process_sequence(&pattern);

        let mut scope = Scope::new();

        scope.push_fn(generated_function);

        generate.line(scope.to_string());
        generate.line(format!(
            "map.insert(vec!{:?}, {});",
            pattern, generated_name
        ));

        converter.reset();

        // let block = Block::new(before)

        // generate.push_block(block)
    }

    // Return the map now
    generate.line("map");

    // This gives me the interface to the super instruction stuff
    let top_level_definition = r#"

    pub(crate) struct SuperInstructionMap {
        map: std::collections::HashMap<Vec<steel_gen::Pattern>, for<'r> fn (&'r mut VmCore<'_>, usize) -> Result<()>>
    }

    impl SuperInstructionMap {
        pub(crate) fn new() -> Self {
            Self { map: std::collections::HashMap::new() }
        }

        pub(crate) fn insert(&mut self, pattern: Vec<steel_gen::Pattern>, func: for<'r> fn (&'r mut VmCore<'_>, usize) -> Result<()>) {
            self.map.insert(pattern, func);
        }

        pub(crate) fn get(&self, op_codes: &[(OpCode, usize)]) -> Option<for<'r> fn (&'r mut VmCore<'_>, usize) -> Result<()>> {
            let pattern = steel_gen::Pattern::from_opcodes(&op_codes);
            self.map.get(&pattern).copied()
        }
    }
    
    lazy_static! {
        pub(crate) static ref DYNAMIC_SUPER_PATTERNS: SuperInstructionMap = generate_dynamic_op_codes();
    }
    
    "#;

    global_scope.push_fn(generate);

    format!("{}\n{}", top_level_definition, global_scope.to_string())
}

#[test]
fn test() {
    // let op_codes = vec![
    //     Pattern::Double(OpCode::BEGINSCOPE, 0),
    //     Pattern::Single(OpCode::LOADINT0),
    //     // Pattern::Single(OpCode::BEGINSCOPE),
    //     Pattern::Single(OpCode::LOADINT1),
    //     Pattern::Single(OpCode::LOADINT1),
    //     Pattern::Single(OpCode::LOADINT1),
    //     Pattern::Single(OpCode::LOADINT1),
    //     Pattern::Single(OpCode::LOADINT1),
    //     Pattern::Single(OpCode::READLOCAL0),
    //     Pattern::Single(OpCode::READLOCAL1),
    //     Pattern::Pair(OpCode::CALLGLOBAL, OpCode::Arity, 6),
    //     // Pattern::Single(OpCode::BEGINSCOPE), // Pattern::Double(OpCode::ADD, 2),
    //     Pattern::Single(OpCode::LOADINT2),
    //     Pattern::Double(OpCode::EQUAL, 2),
    //     Pattern::Single(OpCode::IF),
    // ];

    use OpCode::*;

    let op_codes = vec![
        (MOVEREADLOCAL0, 0),
        (LOADINT2, 225),
        (SUB, 2),
        (CALLGLOBAL, 1),
    ];

    let op_codes = Pattern::from_opcodes(&op_codes);

    println!("{:#?}", op_codes);

    let mut stack_to_ssa = StackToSSAConverter::new();

    let result = stack_to_ssa.process_sequence(&op_codes);

    let mut scope = Scope::new();

    scope.push_fn(result);

    println!("{}", scope.to_string());

    // println!("{}", ctx_signature().to_string());
}

#[test]
fn test_generation() {
    use OpCode::*;

    // TODO: Come up with better way for this to make it in
    let patterns: Vec<Vec<(OpCode, usize)>> = vec![vec![
        (MOVEREADLOCAL0, 0),
        (LOADINT2, 225),
        (SUB, 2),
        (CALLGLOBAL, 1),
    ]];

    println!("{}", generate_opcode_map(patterns));
}