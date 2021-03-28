use std::convert::TryFrom;

use super::{
    constants::{ConstantMap, ConstantTable},
    map::SymbolMap,
};
use crate::{
    core::{instructions::Instruction, opcode::OpCode},
    parser::{ast::Atom, parser::SyntaxObject, span_visitor::get_span, tokens::TokenType},
    values::structs::SteelStruct,
};

use crate::parser::ast::ExprKind;
use crate::parser::visitors::VisitorMut;

use crate::rerrs::{ErrorKind, SteelErr};
use crate::rvals::{Result, SteelVal};
use crate::stop;

use log::info;

// use super::codegen::{check_and_transform_mutual_recursion, transform_tail_call};

pub struct CodeGenerator<'a> {
    instructions: Vec<Instruction>,
    constant_map: &'a mut ConstantMap,
    defining_context: Option<String>,
    symbol_map: &'a mut SymbolMap,
    depth: u32,
    locals: Vec<String>,
}

impl<'a> CodeGenerator<'a> {
    pub fn new(constant_map: &'a mut ConstantMap, symbol_map: &'a mut SymbolMap) -> Self {
        CodeGenerator {
            instructions: Vec::new(),
            constant_map,
            defining_context: None,
            symbol_map,
            depth: 0,
            locals: Vec::new(),
        }
    }

    pub fn new_from_body_instructions(
        constant_map: &'a mut ConstantMap,
        symbol_map: &'a mut SymbolMap,
        instructions: Vec<Instruction>,
        depth: u32,
        locals: Vec<String>,
    ) -> Self {
        CodeGenerator {
            instructions,
            constant_map,
            defining_context: None,
            symbol_map,
            depth,
            locals,
        }
    }

    pub fn compile(mut self, expr: &ExprKind) -> Result<Vec<Instruction>> {
        self.visit(expr)?;
        Ok(self.instructions)
    }

    #[inline]
    fn push(&mut self, instr: Instruction) {
        self.instructions.push(instr);
    }

    #[inline]
    fn len(&self) -> usize {
        self.instructions.len()
    }
}

impl<'a> VisitorMut for CodeGenerator<'a> {
    type Output = Result<()>;

    fn visit_if(&mut self, f: &crate::parser::ast::If) -> Self::Output {
        // load in the test condition
        self.visit(&f.test_expr)?;
        // push in if
        self.push(Instruction::new_if(self.instructions.len() + 2));
        // save spot of jump instruction, fill in after
        let idx = self.len();
        self.push(Instruction::new_jmp(0)); // dummy value

        // emit instructions for then
        self.visit(&f.then_expr)?;
        self.push(Instruction::new_jmp(0));
        let false_start = self.len();

        // emit instructions for else expression
        self.visit(&f.else_expr)?;
        let j3 = self.len(); // first instruction after else

        // set index of jump instruction
        if let Some(elem) = self.instructions.get_mut(idx) {
            (*elem).payload_size = false_start;
        } else {
            stop!(Generic => "out of bounds jump");
        }

        if let Some(elem) = self.instructions.get_mut(false_start - 1) {
            (*elem).payload_size = j3;
        } else {
            stop!(Generic => "out of bounds jump");
        }

        Ok(())
    }

    fn visit_define(&mut self, define: &crate::parser::ast::Define) -> Self::Output {
        // todo!()

        let sidx = self.len();
        self.push(Instruction::new_sdef());

        if let ExprKind::Atom(name) = &define.name {
            let defining_context = if let TokenType::Identifier(ident) = &name.syn.ty {
                if let Some(x) = self.instructions.get_mut(sidx) {
                    x.contents = Some(name.syn.clone());
                }
                Some(ident.clone())
            } else {
                None
            };

            // Set this for tail call optimization ease
            self.defining_context = defining_context;

            self.visit(&define.body)?;
            self.push(Instruction::new_pop());
            let defn_body_size = self.len() - sidx;
            self.push(Instruction::new_edef());

            if let Some(elem) = self.instructions.get_mut(sidx) {
                (*elem).payload_size = defn_body_size;
            } else {
                stop!(Generic => "out of bounds closure len");
            }

            // TODO pick up from here
            if self.depth == 0 {
                println!("binding global: {}", name);
                self.push(Instruction::new_bind(name.syn.clone()));
            } else {
                println!("binding local: {}", name);
                // let ident = &self.defining_context;

                // self.locals.push(ident.clone().unwrap());
                // // Throw in a dummy value for where voids are going to be
                // self.locals.push("#####".to_string()); // TODO get rid of this dummy value

                // let binding_index = self.locals.len() - 2;

                // println!("Binding it to index: {}", binding_index);

                // Do late bound for locals as well
                self.push(Instruction::new_bind_local(0, name.syn.clone()));
            }

            self.push(Instruction::new_void());

            // Clean up the defining context state
            self.defining_context = None;
        } else {
            panic!(
                "Complex defines not supported in bytecode generation: {}",
                (define.name).to_string()
            )
        }

        Ok(())
    }

    fn visit_lambda_function(
        &mut self,
        lambda_function: &crate::parser::ast::LambdaFunction,
    ) -> Self::Output {
        // todo!()

        let idx = self.len();
        self.push(Instruction::new_sclosure());
        self.push(Instruction::new_ndef(0)); // Default with 0 for now

        let mut body_instructions = Vec::new();
        let arity;

        let l = &lambda_function.args;

        arity = l.len();
        // let rev_iter = l.iter().rev();
        let rev_iter = l.iter();
        for symbol in rev_iter {
            if let ExprKind::Atom(atom) = symbol {
                match &atom.syn {
                    SyntaxObject {
                        ty: TokenType::Identifier(i),
                        ..
                    } => {
                        self.locals.push(i.clone());
                        // println!("Validating the identifiers in the arguments");
                        // body_instructions.push(Instruction::new_bind(atom.syn.clone()));
                    }
                    SyntaxObject {
                        ty: _, span: sp, ..
                    } => {
                        stop!(Generic => "lambda function requires list of identifiers"; *sp);
                    }
                }
            } else {
                // stop!(Generic => "lambda function requires list of identifiers"; symbol.span());
                // TODO come back add the span
                stop!(Generic => "lambda function requires list of identifiers");
            }
        }

        fn collect_defines_from_scope(locals: &mut Vec<String>, expr: &ExprKind) {
            // Collect defines for body here
            if let ExprKind::Begin(b) = expr {
                for expr in &b.exprs {
                    match expr {
                        ExprKind::Define(d) => {
                            if let ExprKind::Atom(name) = &d.name {
                                if let TokenType::Identifier(ident) = &name.syn.ty {
                                    locals.push(ident.clone());
                                    // TODO insert dummy value for offset calculation
                                    locals.push("#####".to_string());
                                } else {
                                    panic!("define requires an identifier")
                                }
                            }
                        }
                        ExprKind::Begin(b) => {
                            for expr in &b.exprs {
                                collect_defines_from_scope(locals, &expr);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        collect_defines_from_scope(&mut self.locals, &lambda_function.body);

        // go ahead and statically calculate all of the variables that this closure needs to capture
        // only do so if those variables cannot be accessed locally
        // if the variables can be accessed locally, leave them the same
        // if the variables cannot be accessed locally, give the stack offset where it might exist
        // if it doesn't exist at a stack offset, then the value exists somewhere in a captured environment.
        // The upvalues themselves need to be pointed upwards in some capacity, and these also

        // make recursive call with "fresh" vector so that offsets are correct
        body_instructions = CodeGenerator::new_from_body_instructions(
            &mut self.constant_map,
            &mut self.symbol_map,
            body_instructions,
            self.depth + 1, // pass through the depth
            self.locals.clone(),
        ) // pass through the locals here
        .compile(&lambda_function.body)?;

        body_instructions.push(Instruction::new_pop());
        if let Some(ctx) = &self.defining_context {
            transform_tail_call(&mut body_instructions, ctx);
            let b = check_and_transform_mutual_recursion(&mut body_instructions);
            if b {
                info!("Transformed mutual recursion for: {}", ctx);
            }
        }

        self.instructions.append(&mut body_instructions);
        let closure_body_size = self.len() - idx;
        self.push(Instruction::new_eclosure(arity));

        if let Some(elem) = self.instructions.get_mut(idx) {
            (*elem).payload_size = closure_body_size;
        } else {
            stop!(Generic => "out of bounds closure len");
        }

        Ok(())
    }

    fn visit_begin(&mut self, begin: &crate::parser::ast::Begin) -> Self::Output {
        if begin.exprs.is_empty() {
            self.push(Instruction::new_void());
            return Ok(());
        }

        for expr in &begin.exprs {
            self.visit(expr)?;
        }
        Ok(())
    }

    fn visit_return(&mut self, r: &crate::parser::ast::Return) -> Self::Output {
        self.visit(&r.expr)?;
        // pop is equivalent to the last instruction in the function
        self.push(Instruction::new_pop());
        Ok(())
    }

    fn visit_apply(&mut self, apply: &crate::parser::ast::Apply) -> Self::Output {
        // todo!()
        self.visit(&apply.func)?;
        self.visit(&apply.list)?;
        self.push(Instruction::new_apply(apply.location.clone()));
        Ok(())
    }

    fn visit_panic(&mut self, p: &crate::parser::ast::Panic) -> Self::Output {
        // todo!()
        self.visit(&p.message)?;
        self.push(Instruction::new_panic(p.location.clone()));
        Ok(())
    }

    fn visit_transduce(&mut self, transduce: &crate::parser::ast::Transduce) -> Self::Output {
        self.visit(&transduce.transducer)?;
        self.visit(&transduce.func)?;
        self.visit(&transduce.initial_value)?;
        self.visit(&transduce.iterable)?;
        self.push(Instruction::new_transduce());
        Ok(())
    }

    fn visit_read(&mut self, read: &crate::parser::ast::Read) -> Self::Output {
        self.visit(&read.expr)?;
        self.push(Instruction::new_read());
        Ok(())
    }

    fn visit_execute(&mut self, execute: &crate::parser::ast::Execute) -> Self::Output {
        self.visit(&execute.transducer)?;
        self.visit(&execute.collection)?;

        if let Some(output_type) = &execute.output_type {
            self.visit(output_type)?;
            self.push(Instruction::new_collect_to());
        } else {
            self.push(Instruction::new_collect());
        }
        Ok(())
    }

    fn visit_quote(&mut self, quote: &crate::parser::ast::Quote) -> Self::Output {
        let converted = SteelVal::try_from(quote.expr.clone())?;
        let idx = self.constant_map.add_or_get(converted);
        self.push(Instruction::new_push_const(idx));

        Ok(())
    }

    fn visit_struct(&mut self, s: &crate::parser::ast::Struct) -> Self::Output {
        let builder = SteelStruct::generate_from_ast(&s)?;

        // Add the eventual function names to the symbol map
        let indices = self.symbol_map.insert_struct_function_names(&builder);

        // Get the value we're going to add to the constant map for eventual use
        // Throw the bindings in as well
        let constant_values = builder.to_constant_val(indices);
        let idx = self.constant_map.add_or_get(constant_values);

        // Inside some nested scope, so these don't need anything more than the instruction
        self.push(Instruction::new_struct(idx));

        Ok(())
    }

    fn visit_macro(&mut self, m: &crate::parser::ast::Macro) -> Self::Output {
        stop!(BadSyntax => "unexpected macro definition"; m.location.span)
    }

    fn visit_eval(&mut self, e: &crate::parser::ast::Eval) -> Self::Output {
        self.visit(&e.expr)?;
        self.push(Instruction::new_eval());
        Ok(())
    }

    fn visit_atom(&mut self, a: &crate::parser::ast::Atom) -> Self::Output {
        // println!("visiting atom: {}", a);

        let ident = if let SyntaxObject {
            ty: TokenType::Identifier(i),
            ..
        } = &a.syn
        {
            i
        } else {
            // println!("pushing constant");

            let value = eval_atom(&a.syn)?;
            let idx = self.constant_map.add_or_get(value);
            self.push(Instruction::new(
                OpCode::PUSHCONST,
                idx,
                a.syn.clone(),
                true,
            ));
            return Ok(());
        };

        if let Some(idx) = self.locals.iter().position(|x| x == ident) {
            // println!("pushing local");
            self.push(Instruction::new_local(idx, a.syn.clone()))
        } else {
            // println!("pushing global");
            self.push(Instruction::new(OpCode::PUSH, 0, a.syn.clone(), true));
        }

        // if self.locals.contains()

        Ok(())
    }

    fn visit_list(&mut self, l: &crate::parser::ast::List) -> Self::Output {
        // dbg!(l);

        let pop_len = l.args[1..].len();

        // emit instructions for the args
        for expr in &l.args[1..] {
            self.visit(expr)?;
        }

        // emit instructions for the func
        self.visit(&l.args[0])?;

        if let ExprKind::Atom(Atom { syn: s }) = &l.args[0] {
            self.push(Instruction::new_func(pop_len, s.clone()));
        } else {
            // TODO check span information here by coalescing the entire list
            self.push(Instruction::new_func(
                pop_len,
                SyntaxObject::new(
                    TokenType::Identifier("lambda".to_string()),
                    get_span(&l.args[0]),
                ),
            ));
        }

        Ok(())
    }

    fn visit_syntax_rules(&mut self, l: &crate::parser::ast::SyntaxRules) -> Self::Output {
        stop!(BadSyntax => "unexpected syntax rules"; l.location.span)
    }

    fn visit_set(&mut self, s: &crate::parser::ast::Set) -> Self::Output {
        self.visit(&s.expr)?;
        if let ExprKind::Atom(Atom { syn: s }) = &s.variable {
            self.push(Instruction::new(OpCode::SET, 0, s.clone(), false));
        } else {
            stop!(BadSyntax => "set! takes an identifier")
        }
        Ok(())
    }

    fn visit_require(&mut self, r: &crate::parser::ast::Require) -> Self::Output {
        stop!(BadSyntax => "unexpected require statement in code gen"; r.location.span)
    }

    // There may need to be more magic here
    // but for now, explore how the VM can handle this wth holding
    // the continuation as a value
    fn visit_callcc(&mut self, cc: &crate::parser::ast::CallCC) -> Self::Output {
        self.visit(&cc.expr)?;
        self.push(Instruction::new_call_cc());
        // self.push(Instruction::new_pop());
        Ok(())
    }
}

fn transform_tail_call(instructions: &mut Vec<Instruction>, defining_context: &str) -> bool {
    let last_idx = instructions.len() - 1;

    let mut indices = vec![last_idx];

    let mut transformed = false;

    for (idx, instruction) in instructions.iter().enumerate() {
        if instruction.op_code == OpCode::JMP && instruction.payload_size == last_idx {
            indices.push(idx);
        }
    }

    for index in &indices {
        if *index < 2 {
            continue;
        }
        let prev_instruction = instructions.get(index - 1);
        let prev_func_push = instructions.get(index - 2);

        match (prev_instruction, prev_func_push) {
            (
                Some(Instruction {
                    op_code: OpCode::FUNC,
                    ..
                }),
                Some(Instruction {
                    op_code: OpCode::PUSH,
                    contents:
                        Some(SyntaxObject {
                            ty: TokenType::Identifier(s),
                            ..
                        }),
                    ..
                }),
            ) => {
                if s == defining_context {
                    let new_jmp = Instruction::new_jmp(0);
                    // inject tail call jump
                    instructions[index - 2] = new_jmp;
                    instructions[index - 1] = Instruction::new_pass();
                    transformed = true;

                    info!("Tail call optimization performed for: {}", defining_context);
                    println!("Tail call optimization perfored for: {}", defining_context);
                }
            }
            _ => {}
        }
    }

    transformed
}

// Note, this should be called AFTER `transform_tail_call`
fn check_and_transform_mutual_recursion(instructions: &mut [Instruction]) -> bool {
    let last_idx = instructions.len() - 1;

    // could panic
    let mut indices = vec![last_idx];

    let mut transformed = false;

    for (idx, instruction) in instructions.iter().enumerate() {
        if instruction.op_code == OpCode::JMP && instruction.payload_size == last_idx {
            indices.push(idx);
        }
    }

    for index in &indices {
        if *index < 2 {
            continue;
        }
        let prev_instruction = instructions.get(index - 1);
        let prev_func_push = instructions.get(index - 2);

        match (prev_instruction, prev_func_push) {
            (
                Some(Instruction {
                    op_code: OpCode::FUNC,
                    ..
                }),
                Some(Instruction {
                    op_code: OpCode::PUSH,
                    contents:
                        Some(SyntaxObject {
                            ty: TokenType::Identifier(_s),
                            ..
                        }),
                    ..
                }),
            ) => {
                if let Some(x) = instructions.get_mut(index - 1) {
                    x.op_code = OpCode::TAILCALL;
                    transformed = true;
                }
            }
            _ => {}
        }
    }

    transformed
}

// fn extract_constants<CT: ConstantTable>(
//     instructions: &mut [Instruction],
//     constants: &mut CT,
// ) -> Result<()> {
//     for i in 0..instructions.len() {
//         let inst = &instructions[i];
//         if let OpCode::PUSH = inst.op_code {
//             // let idx = constants.len();
//             if inst.constant {
//                 let value = eval_atom(&inst.contents.as_ref().unwrap())?;
//                 let idx = constants.add_or_get(value);
//                 // constants.push(eval_atom(&inst.contents.as_ref().unwrap())?);
//                 if let Some(x) = instructions.get_mut(i) {
//                     x.op_code = OpCode::PUSHCONST;
//                     x.payload_size = idx;
//                     x.contents = None;
//                 }
//             }
//         }
//     }

//     Ok(())
// }

/// evaluates an atom expression in given environment
fn eval_atom(t: &SyntaxObject) -> Result<SteelVal> {
    match &t.ty {
        TokenType::BooleanLiteral(b) => Ok((*b).into()),
        // TokenType::Identifier(s) => env.borrow().lookup(&s),
        TokenType::NumberLiteral(n) => Ok(SteelVal::NumV(*n)),
        TokenType::StringLiteral(s) => Ok(SteelVal::StringV(s.clone().into())),
        TokenType::CharacterLiteral(c) => Ok(SteelVal::CharV(*c)),
        TokenType::IntegerLiteral(n) => Ok(SteelVal::IntV(*n)),
        what => {
            println!("getting here in the eval_atom");
            stop!(UnexpectedToken => what; t.span)
        }
    }
}
