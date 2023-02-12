#![allow(unused)]

use crate::{
    compiler::{
        // code_generator::{convert_call_globals, CodeGenerator},
        constants::ConstantMap,
        map::SymbolMap,
        passes::{
            analysis::SemanticAnalysis, begin::flatten_begins_and_expand_defines,
            reader::MultipleArityFunctions, shadow::RenameShadowedVariables,
        },
    },
    parser::{ast::AstTools, expand_visitor::expand_kernel, kernel::Kernel},
    steel_vm::builtin::BuiltInModule,
    // values::structs::StructBuilders,
};
use crate::{
    core::{instructions::Instruction, opcode::OpCode},
    parser::parser::Sources,
};

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use std::{iter::Iterator, rc::Rc};

// TODO: Replace the usages of hashmap with this directly
use fxhash::FxHashMap;

use crate::rvals::{Result, SteelVal};

use crate::parser::ast::ExprKind;
use crate::parser::expander::SteelMacro;
use crate::parser::parser::SyntaxObject;
use crate::parser::parser::{ParseError, Parser};
use crate::parser::tokens::TokenType;

// use crate::core::instructions::{densify, DenseInstruction};

use crate::stop;

use log::{debug, log_enabled};

use crate::steel_vm::const_evaluation::ConstantEvaluatorManager;

use super::{
    modules::{CompiledModule, ModuleManager},
    passes::analysis::Analysis,
    program::RawProgramWithSymbols,
};

use im_rc::HashMap as ImmutableHashMap;

use std::time::Instant;

// use itertools::Itertools;

#[derive(Default)]
pub struct DebruijnIndicesInterner {
    flat_defines: HashSet<String>,
    second_pass_defines: HashSet<String>,
}

impl DebruijnIndicesInterner {
    pub fn collect_first_pass_defines(
        &mut self,
        instructions: &mut [Instruction],
        symbol_map: &mut SymbolMap,
    ) -> Result<()> {
        for i in 2..instructions.len() {
            match (&instructions[i], &instructions[i - 1], &instructions[i - 2]) {
                (
                    Instruction {
                        op_code: OpCode::BIND,
                        contents:
                            Some(SyntaxObject {
                                ty: TokenType::Identifier(s),
                                ..
                            }),
                        ..
                    },
                    Instruction {
                        op_code: OpCode::EDEF,
                        ..
                    },
                    Instruction {
                        op_code: OpCode::ECLOSURE,
                        ..
                    },
                ) => {
                    let idx = symbol_map.get_or_add(s);
                    self.flat_defines.insert(s.to_owned());

                    if let Some(x) = instructions.get_mut(i) {
                        x.payload_size = idx;
                    }
                }
                (
                    Instruction {
                        op_code: OpCode::BIND,
                        contents:
                            Some(SyntaxObject {
                                ty: TokenType::Identifier(s),
                                ..
                            }),
                        ..
                    },
                    ..,
                ) => {
                    let idx = symbol_map.get_or_add(s);
                    self.flat_defines.insert(s.to_owned());

                    if let Some(x) = instructions.get_mut(i) {
                        x.payload_size = idx;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub fn collect_second_pass_defines(
        &mut self,
        instructions: &mut [Instruction],
        symbol_map: &mut SymbolMap,
    ) -> Result<()> {
        // let mut second_pass_defines: HashSet<String> = HashSet::new();

        let mut depth = 0;

        // name mangle
        // Replace all identifiers with indices
        for i in 0..instructions.len() {
            match &instructions[i] {
                Instruction {
                    op_code: OpCode::SCLOSURE | OpCode::NEWSCLOSURE | OpCode::PUREFUNC,
                    ..
                } => {
                    depth += 1;
                }
                Instruction {
                    op_code: OpCode::ECLOSURE,
                    ..
                } => {
                    depth -= 1;
                }
                Instruction {
                    op_code: OpCode::BIND,
                    contents:
                        Some(SyntaxObject {
                            ty: TokenType::Identifier(s),
                            ..
                        }),
                    ..
                } => {
                    // Keep track of where the defines actually are in the process
                    self.second_pass_defines.insert(s.to_owned());
                }
                Instruction {
                    op_code: OpCode::PUSH,
                    contents:
                        Some(SyntaxObject {
                            ty: TokenType::Identifier(s),
                            span,
                            ..
                        }),
                    ..
                }
                | Instruction {
                    op_code: OpCode::SET,
                    contents:
                        Some(SyntaxObject {
                            ty: TokenType::Identifier(s),
                            span,
                            ..
                        }),
                    ..
                } => {
                    if self.flat_defines.get(s).is_some()
                        && self.second_pass_defines.get(s).is_none()
                        && depth == 0
                    {
                        let message =
                            format!("Cannot reference an identifier before its definition: {s}");
                        stop!(FreeIdentifier => message; *span);
                    }

                    let idx = symbol_map.get(s).map_err(|e| e.set_span(*span))?;

                    // TODO commenting this for now
                    if let Some(x) = instructions.get_mut(i) {
                        x.payload_size = idx;
                        x.constant = false;
                    }
                }
                Instruction {
                    op_code: OpCode::CALLGLOBAL,
                    contents:
                        Some(SyntaxObject {
                            ty: TokenType::Identifier(s),
                            span,
                            ..
                        }),
                    ..
                }
                | Instruction {
                    op_code: OpCode::CALLGLOBALTAIL,
                    contents:
                        Some(SyntaxObject {
                            ty: TokenType::Identifier(s),
                            span,
                            ..
                        }),
                    ..
                } => {
                    if self.flat_defines.get(s).is_some()
                        && self.second_pass_defines.get(s).is_none()
                        && depth == 0
                    {
                        let message =
                            format!("Cannot reference an identifier before its definition: {s}");
                        stop!(FreeIdentifier => message; *span);
                    }

                    let idx = symbol_map.get(s).map_err(|e| e.set_span(*span))?;

                    // TODO commenting this for now
                    if let Some(x) = instructions.get_mut(i + 1) {
                        x.payload_size = idx;
                        x.constant = false;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

// TODO this needs to take into account if they are functions or not before adding them
// don't just blindly do all global defines first - need to do them in order correctly
// pub fn replace_defines_with_debruijn_indices(
//     instructions: &mut [Instruction],
//     symbol_map: &mut SymbolMap,
// ) -> Result<()> {
//     let mut flat_defines: HashSet<String> = HashSet::new();

//     for i in 2..instructions.len() {
//         match (&instructions[i], &instructions[i - 1], &instructions[i - 2]) {
//             (
//                 Instruction {
//                     op_code: OpCode::BIND,
//                     contents:
//                         Some(SyntaxObject {
//                             ty: TokenType::Identifier(s),
//                             ..
//                         }),
//                     ..
//                 },
//                 Instruction {
//                     op_code: OpCode::EDEF,
//                     ..
//                 },
//                 Instruction {
//                     op_code: OpCode::ECLOSURE,
//                     ..
//                 },
//             ) => {
//                 let idx = symbol_map.get_or_add(s);
//                 flat_defines.insert(s.to_owned());

//                 if let Some(x) = instructions.get_mut(i) {
//                     x.payload_size = idx;
//                 }
//             }
//             (
//                 Instruction {
//                     op_code: OpCode::BIND,
//                     contents:
//                         Some(SyntaxObject {
//                             ty: TokenType::Identifier(s),
//                             ..
//                         }),
//                     ..
//                 },
//                 ..,
//             ) => {
//                 let idx = symbol_map.get_or_add(s);
//                 flat_defines.insert(s.to_owned());

//                 if let Some(x) = instructions.get_mut(i) {
//                     x.payload_size = idx;
//                 }
//             }
//             _ => {}
//         }
//     }

//     let mut second_pass_defines: HashSet<String> = HashSet::new();

//     let mut depth = 0;

//     // name mangle
//     // Replace all identifiers with indices
//     for i in 0..instructions.len() {
//         match &instructions[i] {
//             Instruction {
//                 op_code: OpCode::SCLOSURE | OpCode::NEWSCLOSURE | OpCode::PUREFUNC,
//                 ..
//             } => {
//                 depth += 1;
//             }
//             Instruction {
//                 op_code: OpCode::ECLOSURE,
//                 ..
//             } => {
//                 depth -= 1;
//             }
//             Instruction {
//                 op_code: OpCode::BIND,
//                 contents:
//                     Some(SyntaxObject {
//                         ty: TokenType::Identifier(s),
//                         ..
//                     }),
//                 ..
//             } => {
//                 // Keep track of where the defines actually are in the process
//                 second_pass_defines.insert(s.to_owned());
//             }
//             Instruction {
//                 op_code: OpCode::PUSH,
//                 contents:
//                     Some(SyntaxObject {
//                         ty: TokenType::Identifier(s),
//                         span,
//                         ..
//                     }),
//                 ..
//             }
//             | Instruction {
//                 op_code: OpCode::CALLGLOBAL,
//                 contents:
//                     Some(SyntaxObject {
//                         ty: TokenType::Identifier(s),
//                         span,
//                         ..
//                     }),
//                 ..
//             }
//             | Instruction {
//                 op_code: OpCode::CALLGLOBALTAIL,
//                 contents:
//                     Some(SyntaxObject {
//                         ty: TokenType::Identifier(s),
//                         span,
//                         ..
//                     }),
//                 ..
//             }
//             | Instruction {
//                 op_code: OpCode::SET,
//                 contents:
//                     Some(SyntaxObject {
//                         ty: TokenType::Identifier(s),
//                         span,
//                         ..
//                     }),
//                 ..
//             } => {
//                 if flat_defines.get(s).is_some() {
//                     if second_pass_defines.get(s).is_none() && depth == 0 {
//                         let message = format!(
//                             "Cannot reference an identifier before its definition: {}",
//                             s
//                         );
//                         stop!(FreeIdentifier => message; *span);
//                     }
//                 }

//                 let idx = symbol_map.get(s).map_err(|e| e.set_span(*span))?;

//                 // TODO commenting this for now
//                 if let Some(x) = instructions.get_mut(i) {
//                     x.payload_size = idx;
//                     x.constant = false;
//                 }
//             }
//             _ => {}
//         }
//     }

//     Ok(())
// }

// Adds a flag to the pop value in order to save the heap to the global heap
// I should really come up with a better name but for now we'll leave it
// fn inject_heap_save_to_pop(instructions: &mut [Instruction]) {
//     match instructions {
//         [.., Instruction {
//             op_code: OpCode::EDEF,
//             ..
//         }, Instruction {
//             op_code: OpCode::BIND,
//             ..
//         }, Instruction {
//             op_code: OpCode::VOID,
//             ..
//         }, Instruction {
//             op_code: OpCode::POP,
//             payload_size: x,
//             ..
//         }] => {
//             *x = 1;
//         }
//         _ => {}
//     }
// }

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum OptLevel {
    Zero = 0,
    One,
    Two,
    Three,
}

#[derive(Clone)]
pub struct Compiler {
    pub(crate) symbol_map: SymbolMap,
    pub(crate) constant_map: ConstantMap,
    pub(crate) macro_env: HashMap<String, SteelMacro>,
    module_manager: ModuleManager,
    opt_level: OptLevel,
    pub(crate) kernel: Option<Kernel>,
}

impl Default for Compiler {
    fn default() -> Self {
        Compiler::new(
            SymbolMap::new(),
            ConstantMap::new(),
            HashMap::new(),
            ModuleManager::default(),
        )
    }
}

impl Compiler {
    fn new(
        symbol_map: SymbolMap,
        constant_map: ConstantMap,
        macro_env: HashMap<String, SteelMacro>,
        module_manager: ModuleManager,
    ) -> Compiler {
        Compiler {
            symbol_map,
            constant_map,
            macro_env,
            module_manager,
            opt_level: OptLevel::Three,
            kernel: None,
        }
    }

    fn new_with_kernel(
        symbol_map: SymbolMap,
        constant_map: ConstantMap,
        macro_env: HashMap<String, SteelMacro>,
        module_manager: ModuleManager,
        kernel: Kernel,
    ) -> Compiler {
        Compiler {
            symbol_map,
            constant_map,
            macro_env,
            module_manager,
            opt_level: OptLevel::Three,
            kernel: Some(kernel),
        }
    }

    pub(crate) fn default_from_kernel(kernel: Kernel) -> Compiler {
        Compiler::new_with_kernel(
            SymbolMap::new(),
            ConstantMap::new(),
            HashMap::new(),
            ModuleManager::default(),
            kernel,
        )
    }

    pub fn default_with_kernel() -> Compiler {
        Compiler::new_with_kernel(
            SymbolMap::new(),
            ConstantMap::new(),
            HashMap::new(),
            ModuleManager::default(),
            Kernel::new(),
        )
    }

    /// Registers a name in the underlying symbol map and returns the idx that it maps to
    pub fn register(&mut self, name: &str) -> usize {
        self.symbol_map.get_or_add(name)
    }

    /// Get the index associated with a name in the underlying symbol map
    /// If the name hasn't been registered, this will return `None`
    pub fn get_idx(&self, name: &str) -> Option<usize> {
        self.symbol_map.get(name).ok()
    }

    pub fn compile_executable_from_expressions(
        &mut self,
        exprs: Vec<ExprKind>,
        builtin_modules: ImmutableHashMap<Rc<str>, BuiltInModule>,
        constants: ImmutableHashMap<String, SteelVal>,
        sources: &mut Sources,
    ) -> Result<RawProgramWithSymbols> {
        self.compile_raw_program(exprs, constants, builtin_modules, None, sources)
    }

    pub fn compile_executable(
        &mut self,
        expr_str: &str,
        path: Option<PathBuf>,
        constants: ImmutableHashMap<String, SteelVal>,
        builtin_modules: ImmutableHashMap<std::rc::Rc<str>, BuiltInModule>,
        sources: &mut Sources,
    ) -> Result<RawProgramWithSymbols> {
        let mut intern = HashMap::new();

        #[cfg(feature = "profiling")]
        let now = Instant::now();

        let id = sources.add_source(expr_str.to_string(), path.clone());

        // Could fail here
        let parsed: std::result::Result<Vec<ExprKind>, ParseError> = if let Some(p) = &path {
            Parser::new_from_source(expr_str, &mut intern, p.clone(), Some(id)).collect()
        } else {
            Parser::new(expr_str, &mut intern, Some(id)).collect()
        };

        #[cfg(feature = "profiling")]
        if log_enabled!(target: "pipeline_time", log::Level::Debug) {
            debug!(target: "pipeline_time", "Parsing Time: {:?}", now.elapsed());
        }

        let parsed = parsed?;

        // TODO fix this hack
        self.compile_raw_program(parsed, constants, builtin_modules, path, sources)
    }

    // TODO: Add a flag/function for parsing comments as well
    // Move the body of this function into the other one, so that way we have proper
    pub fn emit_expanded_ast(
        &mut self,
        expr_str: &str,
        constants: ImmutableHashMap<String, SteelVal>,
        path: Option<PathBuf>,
        sources: &mut Sources,
        builtin_modules: ImmutableHashMap<Rc<str>, BuiltInModule>,
    ) -> Result<Vec<ExprKind>> {
        let mut intern = HashMap::new();

        let id = sources.add_source(expr_str.to_string(), path.clone());

        // Could fail here
        let parsed: std::result::Result<Vec<ExprKind>, ParseError> =
            Parser::new(expr_str, &mut intern, Some(id)).collect();

        let parsed = parsed?;

        let mut expanded_statements =
            self.expand_expressions(parsed, path, sources, builtin_modules.clone())?;

        if log_enabled!(log::Level::Debug) {
            debug!(
                "Generating instructions for the expression: {:?}",
                expanded_statements
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
            );
        }

        expanded_statements = expanded_statements
            .into_iter()
            .map(|x| expand_kernel(x, self.kernel.as_mut(), builtin_modules.clone()))
            .collect::<Result<Vec<_>>>()?;

        let mut expanded_statements =
            self.apply_const_evaluation(constants, expanded_statements)?;

        RenameShadowedVariables::rename_shadowed_vars(&mut expanded_statements);

        let mut analysis = Analysis::from_exprs(&expanded_statements);
        analysis.populate_captures(&expanded_statements);

        let mut semantic = SemanticAnalysis::from_analysis(&mut expanded_statements, analysis);

        // This is definitely broken still
        semantic
            // .replace_anonymous_function_calls_with_plain_lets();
            .lift_pure_local_functions();
        // .lift_all_local_functions();

        debug!("About to expand defines");
        let mut expanded_statements = flatten_begins_and_expand_defines(expanded_statements);

        let mut analysis = Analysis::from_exprs(&expanded_statements);
        analysis.populate_captures(&expanded_statements);

        let mut semantic = SemanticAnalysis::from_analysis(&mut expanded_statements, analysis);
        semantic.refresh_variables();

        semantic.flatten_anonymous_functions();

        semantic.refresh_variables();

        if log_enabled!(log::Level::Debug) {
            debug!(
                "Successfully expanded defines: {:?}",
                expanded_statements
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
            );
        }

        // TODO - make sure I want to keep this
        let expanded_statements =
            MultipleArityFunctions::expand_multiple_arity_functions(expanded_statements);

        Ok(expanded_statements)
    }

    pub fn compile_module(
        &mut self,
        path: PathBuf,
        sources: &mut Sources,
        builtin_modules: ImmutableHashMap<Rc<str>, BuiltInModule>,
    ) -> Result<()> {
        self.module_manager.add_module(
            path,
            &mut self.macro_env,
            &mut self.kernel,
            sources,
            builtin_modules,
        )
    }

    pub fn modules(&self) -> &HashMap<PathBuf, CompiledModule> {
        self.module_manager.modules()
    }

    pub fn expand_expressions(
        &mut self,
        exprs: Vec<ExprKind>,
        path: Option<PathBuf>,
        sources: &mut Sources,
        builtin_modules: ImmutableHashMap<Rc<str>, BuiltInModule>,
    ) -> Result<Vec<ExprKind>> {
        #[cfg(feature = "modules")]
        return self.module_manager.compile_main(
            &mut self.macro_env,
            &mut self.kernel,
            sources,
            exprs,
            path,
            builtin_modules,
        );

        #[cfg(not(feature = "modules"))]
        self.module_manager
            .expand_expressions(&mut self.macro_env, exprs)
    }

    fn generate_instructions_for_executable(
        &mut self,
        expanded_statements: Vec<ExprKind>,
    ) -> Result<Vec<Vec<Instruction>>> {
        let mut results = Vec::new();
        let mut instruction_buffer = Vec::new();
        let mut index_buffer = Vec::new();

        let analysis = {
            let mut analysis = Analysis::from_exprs(&expanded_statements);
            analysis.populate_captures(&expanded_statements);
            analysis.populate_captures(&expanded_statements);
            analysis
        };

        // expanded_statements.pretty_print();

        for expr in expanded_statements {
            let mut instructions =
                super::code_gen::CodeGenerator::new(&mut self.constant_map, &analysis)
                    .top_level_compile(&expr)?;

            // TODO: I don't think this needs to be here anymore
            // inject_heap_save_to_pop(&mut instructions);
            index_buffer.push(instructions.len());
            instruction_buffer.append(&mut instructions);
        }

        for idx in index_buffer {
            let extracted: Vec<Instruction> = instruction_buffer.drain(0..idx).collect();
            results.push(extracted);
        }

        Ok(results)
    }

    // TODO
    // figure out how the symbols will work so that a raw program with symbols
    // can be later pulled in and symbols can be interned correctly
    fn compile_raw_program(
        &mut self,
        exprs: Vec<ExprKind>,
        constants: ImmutableHashMap<String, SteelVal>,
        builtin_modules: ImmutableHashMap<Rc<str>, BuiltInModule>,
        path: Option<PathBuf>,
        sources: &mut Sources,
    ) -> Result<RawProgramWithSymbols> {
        let mut expanded_statements =
            self.expand_expressions(exprs, path, sources, builtin_modules.clone())?;

        if log_enabled!(log::Level::Debug) {
            debug!(
                "Generating instructions for the expression: {:?}",
                expanded_statements
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
            );
        }

        expanded_statements = expanded_statements
            .into_iter()
            .map(|x| expand_kernel(x, self.kernel.as_mut(), builtin_modules.clone()))
            .collect::<Result<Vec<_>>>()?;

        let mut expanded_statements =
            self.apply_const_evaluation(constants, expanded_statements)?;

        RenameShadowedVariables::rename_shadowed_vars(&mut expanded_statements);

        let mut analysis = Analysis::from_exprs(&expanded_statements);
        analysis.populate_captures(&expanded_statements);

        let mut semantic = SemanticAnalysis::from_analysis(&mut expanded_statements, analysis);

        // This is definitely broken still
        semantic
            // .replace_anonymous_function_calls_with_plain_lets();
            .lift_pure_local_functions();
        // .lift_all_local_functions();

        debug!("About to expand defines");
        let mut expanded_statements = flatten_begins_and_expand_defines(expanded_statements);

        let mut analysis = Analysis::from_exprs(&expanded_statements);
        analysis.populate_captures(&expanded_statements);

        let mut semantic = SemanticAnalysis::from_analysis(&mut expanded_statements, analysis);
        semantic.refresh_variables();

        semantic.flatten_anonymous_functions();

        semantic.refresh_variables();

        if log_enabled!(log::Level::Debug) {
            debug!(
                "Successfully expanded defines: {:?}",
                expanded_statements
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
            );
        }

        // TODO - make sure I want to keep this
        let expanded_statements =
            MultipleArityFunctions::expand_multiple_arity_functions(expanded_statements);

        let instructions = self.generate_instructions_for_executable(expanded_statements)?;

        let mut raw_program = RawProgramWithSymbols::new(
            instructions,
            self.constant_map.clone(),
            "0.1.0".to_string(),
        );

        // Make sure to apply the peephole optimizations
        raw_program.apply_optimizations();

        Ok(raw_program)
    }

    fn apply_const_evaluation(
        &mut self,
        constants: ImmutableHashMap<String, SteelVal>,
        mut expanded_statements: Vec<ExprKind>,
    ) -> Result<Vec<ExprKind>> {
        #[cfg(feature = "profiling")]
        let opt_time = Instant::now();

        match self.opt_level {
            // TODO
            // Cut this off at 10 iterations no matter what
            OptLevel::Three => {
                for _ in 0..10 {
                    let mut manager =
                        ConstantEvaluatorManager::new(constants.clone(), self.opt_level);
                    expanded_statements = manager.run(expanded_statements)?;

                    if !manager.changed {
                        break;
                    }
                }
            }
            OptLevel::Two => {
                expanded_statements = ConstantEvaluatorManager::new(constants, self.opt_level)
                    .run(expanded_statements)?;
            }
            _ => {}
        }

        #[cfg(feature = "profiling")]
        if log_enabled!(target: "pipeline_time", log::Level::Debug) {
            debug!(
                target: "pipeline_time",
                "Const Evaluation Time: {:?}",
                opt_time.elapsed()
            );
        };

        Ok(expanded_statements)
    }
}
