use super::{
    builtin::BuiltInModule,
    dylib::DylibContainers,
    primitives::{register_builtin_modules, register_builtin_modules_without_io, CONSTANTS},
    vm::SteelThread,
};
use crate::{
    compiler::{
        compiler::Compiler,
        modules::CompiledModule,
        program::{Executable, RawProgramWithSymbols},
    },
    parser::ast::ExprKind,
    parser::{
        kernel::{fresh_kernel_image, Kernel},
        parser::{ParseError, Parser, Sources},
    },
    rerrs::back_trace,
    rvals::{FromSteelVal, IntoSteelVal, Result, SteelVal},
    stop, throw, SteelErr,
};
use std::{collections::HashMap, path::PathBuf, rc::Rc};

use im_rc::HashMap as ImmutableHashMap;
use itertools::Itertools;

#[derive(Clone)]
pub struct Engine {
    virtual_machine: SteelThread,
    compiler: Compiler,
    constants: Option<ImmutableHashMap<String, SteelVal>>,
    modules: ImmutableHashMap<Rc<str>, BuiltInModule>,
    sources: Sources,
    dylibs: DylibContainers,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Function to access a kernel level execution environment
    /// Has access to primitives and syntax rules, but will not defer to a child
    /// kernel in the compiler
    pub(crate) fn new_kernel() -> Self {
        log::info!(target:"kernel", "Instantiating a new kernel");

        let mut vm = Engine {
            virtual_machine: SteelThread::new(),
            compiler: Compiler::default(),
            constants: None,
            modules: ImmutableHashMap::new(),
            sources: Sources::new(),
            dylibs: DylibContainers::new(),
        };

        register_builtin_modules(&mut vm);

        vm.compile_and_run_raw_program(crate::steel_vm::primitives::ALL_MODULES)
            .unwrap();

        log::info!(target:"kernel", "Registered modules in the kernel!");

        // embed_primitives(&mut vm);

        let core_libraries = [
            crate::stdlib::PRELUDE,
            crate::stdlib::CONTRACTS,
            crate::stdlib::DISPLAY,
        ];

        for core in core_libraries.into_iter() {
            vm.compile_and_run_raw_program(core).unwrap();
        }

        vm.dylibs.load_modules();

        let modules = vm.dylibs.modules().collect::<Vec<_>>();

        for module in modules {
            vm.register_module(module);
        }

        vm
    }

    /// Instantiates a raw engine instance. Includes no primitives or prelude.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new_raw();
    /// assert!(vm.run("(+ 1 2 3").is_err()); // + is a free identifier
    /// ```
    pub fn new_raw() -> Self {
        Engine {
            virtual_machine: SteelThread::new(),
            compiler: Compiler::default_with_kernel(),
            constants: None,
            modules: ImmutableHashMap::new(),
            sources: Sources::new(),
            dylibs: DylibContainers::new(),
        }
    }

    /// Instantiates a new engine instance with all primitive functions enabled.
    /// This excludes the prelude and contract files.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new_base();
    /// // map is found in the prelude, so this will fail
    /// assert!(vm.run(r#"(map (lambda (x) 10) (list 1 2 3 4 5))"#).is_err());
    /// ```
    #[inline]
    pub fn new_base() -> Self {
        let mut vm = Engine::new_raw();
        // Embed any primitives that we want to use

        register_builtin_modules(&mut vm);

        vm.compile_and_run_raw_program(crate::steel_vm::primitives::ALL_MODULES)
            .unwrap();

        vm.dylibs.load_modules();

        let modules = vm.dylibs.modules().collect::<Vec<_>>();

        for module in modules {
            vm.register_module(module);
        }

        // vm.dylibs.load_modules(&mut vm);

        vm
    }

    pub fn with_contracts(&mut self, contracts: bool) -> &mut Self {
        self.virtual_machine.with_contracts(contracts);
        self
    }

    #[inline]
    pub fn new_sandboxed() -> Self {
        let mut vm = Engine::new_raw();

        register_builtin_modules_without_io(&mut vm);

        vm.compile_and_run_raw_program(crate::steel_vm::primitives::SANDBOXED_MODULES)
            .unwrap();

        let core_libraries = [
            crate::stdlib::PRELUDE,
            crate::stdlib::CONTRACTS,
            crate::stdlib::DISPLAY,
        ];

        for core in core_libraries.into_iter() {
            vm.compile_and_run_raw_program(core).unwrap();
        }

        vm
    }

    pub fn call_printing_method_in_context(&mut self, argument: SteelVal) -> Result<SteelVal> {
        let function = self.extract_value("println")?;
        self.call_function_with_args(function, vec![argument])
    }

    pub(crate) fn call_function_with_args(
        &mut self,
        function: SteelVal,
        arguments: Vec<SteelVal>,
    ) -> Result<SteelVal> {
        self.virtual_machine
            .call_function(self.compiler.constant_map.clone(), function, arguments)
    }

    pub fn run(&mut self, input: &str) -> Result<Vec<SteelVal>> {
        self.compile_and_run_raw_program(input)
    }

    /// Instantiates a new engine instance with all the primitive functions enabled.
    /// This is the most general engine entry point, and includes both the contract and
    /// prelude files in the root.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new();
    /// vm.run(r#"(+ 1 2 3)"#).unwrap();
    /// ```
    pub fn new() -> Self {
        let mut engine = fresh_kernel_image();

        engine.compiler.kernel = Some(Kernel::new());

        engine

        // let mut vm = Engine::new_base();

        // let core_libraries = [
        //     crate::stdlib::PRELUDE,
        //     crate::stdlib::DISPLAY,
        //     crate::stdlib::CONTRACTS,
        // ];

        // for core in core_libraries.into_iter() {
        //     vm.compile_and_run_raw_program(core).unwrap();
        // }

        // vm
    }

    /// Consumes the current `Engine` and emits a new `Engine` with the prelude added
    /// to the environment. The prelude won't work unless the primitives are also enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new_base().with_prelude().unwrap();
    /// vm.run("(+ 1 2 3)").unwrap();
    /// ```
    pub fn with_prelude(mut self) -> Result<Self> {
        let core_libraries = &[
            crate::stdlib::PRELUDE,
            crate::stdlib::DISPLAY,
            crate::stdlib::CONTRACTS,
        ];

        for core in core_libraries {
            self.compile_and_run_raw_program(core)?;
        }

        Ok(self)
    }

    /// Registers the prelude to the environment of the given Engine.
    /// The prelude won't work unless the primitives are also enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new_base();
    /// vm.register_prelude().unwrap();
    /// vm.run("(+ 1 2 3)").unwrap();
    /// ```
    pub fn register_prelude(&mut self) -> Result<&mut Self> {
        let core_libraries = &[
            crate::stdlib::PRELUDE,
            crate::stdlib::DISPLAY,
            crate::stdlib::CONTRACTS,
        ];

        for core in core_libraries {
            self.compile_and_run_raw_program(core)?;
        }

        Ok(self)
    }

    // Registers the given module into the virtual machine
    pub fn register_module(&mut self, module: BuiltInModule) -> &mut Self {
        // Add the module to the map
        self.modules.insert(Rc::clone(&module.name), module.clone());
        // Register the actual module itself as a value to make the virtual machine capable of reading from it
        self.register_value(
            module.unreadable_name().as_str(),
            module.into_steelval().unwrap(),
        );

        self
    }

    // /// Emits a program with path information embedded for error messaging.
    // pub fn emit_program_with_path(&mut self, expr: &str, path: PathBuf) -> Result<Program> {
    //     let constants = self.constants();
    //     self.compiler.compile_program(expr, Some(path), constants)
    // }

    /// Emits a program for a given `expr` directly without providing any error messaging for the path.
    // pub fn emit_program(&mut self, expr: &str) -> Result<Program> {
    //     let constants = self.constants();
    //     self.compiler.compile_program(expr, None, constants)
    // }

    pub fn emit_raw_program_no_path(&mut self, expr: &str) -> Result<RawProgramWithSymbols> {
        let constants = self.constants();
        self.compiler.compile_executable(
            expr,
            None,
            constants,
            self.modules.clone(),
            &mut self.sources,
        )
    }

    pub fn emit_raw_program(&mut self, expr: &str, path: PathBuf) -> Result<RawProgramWithSymbols> {
        let constants = self.constants();
        self.compiler.compile_executable(
            expr,
            Some(path),
            constants,
            self.modules.clone(),
            &mut self.sources,
        )
    }

    pub fn debug_print_build(
        &mut self,
        name: String,
        program: RawProgramWithSymbols,
    ) -> Result<()> {
        program.debug_build(name, &mut self.compiler.symbol_map)
    }

    // Attempts to disassemble the given expression into a series of bytecode dumps
    // pub fn disassemble(&mut self, expr: &str) -> Result<String> {
    //     let constants = self.constants();
    //     self.compiler
    //         .emit_debug_instructions(expr, constants)
    //         .map(|x| {
    //             x.into_iter()
    //                 .map(|i| crate::core::instructions::disassemble(&i))
    //                 .join("\n\n")
    //         })
    // }

    // pub fn execute_without_callbacks(
    //     &mut self,
    //     bytecode: Rc<[DenseInstruction]>,
    //     constant_map: &ConstantMap,
    // ) -> Result<SteelVal> {
    //     self.virtual_machine
    //         .execute::<DoNotUseCallback>(bytecode, constant_map, &[])
    // }

    /// Execute bytecode with a constant map directly.
    // pub fn execute(
    //     &mut self,
    //     bytecode: Rc<[DenseInstruction]>,
    //     constant_map: ConstantMap,
    // ) -> Result<SteelVal> {
    //     self.virtual_machine
    //         .execute(bytecode, constant_map, Rc::from([]))
    // }

    /// Emit the bytecode directly, with a path provided.
    // pub fn emit_instructions_with_path(
    //     &mut self,
    //     exprs: &str,
    //     path: PathBuf,
    // ) -> Result<Vec<Vec<DenseInstruction>>> {
    //     let constants = self.constants();
    //     self.compiler
    //         .emit_instructions(exprs, Some(path), constants)
    // }

    // /// Emit instructions directly, without a path for error messaging.
    // pub fn emit_instructions(&mut self, exprs: &str) -> Result<Vec<Vec<DenseInstruction>>> {
    //     let constants = self.constants();
    //     self.compiler.emit_instructions(exprs, None, constants)
    // }

    /// Execute a program directly, returns a vector of `SteelVal`s corresponding to each expr in the `Program`.
    // pub fn execute_program(&mut self, program: Program) -> Result<Vec<SteelVal>> {
    //     self.virtual_machine
    //         .execute_program::<UseCallback, ApplyContract>(program)
    // }

    // TODO -> clean up this API a lot
    pub fn compile_and_run_raw_program_with_path(
        &mut self,
        exprs: &str,
        path: PathBuf,
    ) -> Result<Vec<SteelVal>> {
        let constants = self.constants();
        let program = self.compiler.compile_executable(
            exprs,
            Some(path),
            constants,
            self.modules.clone(),
            &mut self.sources,
        )?;

        // program.profile_instructions();

        self.run_raw_program(program)
    }

    pub(crate) fn _run_raw_program_from_exprs(
        &mut self,
        exprs: Vec<ExprKind>,
    ) -> Result<Vec<SteelVal>> {
        let constants = self.constants();
        let program = self.compiler.compile_executable_from_expressions(
            exprs,
            self.modules.clone(),
            constants,
            &mut self.sources,
        )?;
        self.run_raw_program(program)
    }

    pub fn compile_and_run_raw_program(&mut self, exprs: &str) -> Result<Vec<SteelVal>> {
        let constants = self.constants();
        let program = self.compiler.compile_executable(
            exprs,
            None,
            constants,
            self.modules.clone(),
            &mut self.sources,
        )?;

        // program.profile_instructions();

        self.run_raw_program(program)
    }

    pub fn raw_program_to_executable(
        &mut self,
        program: RawProgramWithSymbols,
    ) -> Result<Executable> {
        let symbol_map_offset = self.compiler.symbol_map.len();

        let result = program.build("TestProgram".to_string(), &mut self.compiler.symbol_map);

        if result.is_err() {
            self.compiler.symbol_map.roll_back(symbol_map_offset);
        }

        result
    }

    pub fn run_raw_program(&mut self, program: RawProgramWithSymbols) -> Result<Vec<SteelVal>> {
        let executable = self.raw_program_to_executable(program)?;
        self.virtual_machine.run_executable(&executable)
    }

    pub fn run_executable(&mut self, executable: &Executable) -> Result<Vec<SteelVal>> {
        self.virtual_machine.run_executable(executable)
    }

    /// Directly emit the expanded ast
    pub fn emit_expanded_ast(
        &mut self,
        expr: &str,
        path: Option<PathBuf>,
    ) -> Result<Vec<ExprKind>> {
        let constants = self.constants();
        self.compiler.emit_expanded_ast(
            expr,
            constants,
            path,
            &mut self.sources,
            self.modules.clone(),
        )
    }

    /// Emit the unexpanded AST
    pub fn emit_ast_to_string(expr: &str) -> Result<String> {
        let mut intern = HashMap::new();
        let parsed: std::result::Result<Vec<ExprKind>, ParseError> =
            Parser::new(expr, &mut intern, None).collect();
        let parsed = parsed?;
        Ok(parsed.into_iter().map(|x| x.to_pretty(60)).join("\n\n"))
    }

    /// Emit the fully expanded AST
    pub fn emit_fully_expanded_ast_to_string(
        &mut self,
        expr: &str,
        path: Option<PathBuf>,
    ) -> Result<String> {
        let constants = self.constants();
        Ok(self
            .compiler
            .emit_expanded_ast(
                expr,
                constants,
                path,
                &mut self.sources,
                self.modules.clone(),
            )?
            .into_iter()
            .map(|x| x.to_pretty(60))
            .join("\n\n"))
    }

    /// Registers an external value of any type as long as it implements [`FromSteelVal`](crate::rvals::FromSteelVal) and
    /// [`IntoSteelVal`](crate::rvals::IntoSteelVal). This method does the coercion to embed the type into the `Engine`'s
    /// environment with the name `name`. This function can fail only if the conversion from `T` to [`SteelVal`](crate::rvals::SteelVal) fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new();
    /// let external_value = "hello-world".to_string();
    /// vm.register_external_value("hello-world", external_value).unwrap();
    /// vm.run("hello-world").unwrap(); // Will return the string
    /// ```
    pub fn register_external_value<T: FromSteelVal + IntoSteelVal>(
        &mut self,
        name: &str,
        value: T,
    ) -> Result<&mut Self> {
        let converted = value.into_steelval()?;
        Ok(self.register_value(name, converted))
    }

    /// Registers a [`SteelVal`](crate::rvals::SteelVal) under the name `name` in the `Engine`'s internal environment.
    ///
    /// # Examples
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// use steel::rvals::SteelVal;
    ///
    /// let mut vm = Engine::new();
    /// let external_value = SteelVal::StringV("hello-world".to_string().into());
    /// vm.register_value("hello-world", external_value);
    /// vm.run("hello-world").unwrap(); // Will return the string
    /// ```
    pub fn register_value(&mut self, name: &str, value: SteelVal) -> &mut Self {
        let idx = self.compiler.register(name);
        self.virtual_machine.insert_binding(idx, value);
        self
    }

    /// Registers multiple values at once
    pub fn register_values(
        &mut self,
        values: impl Iterator<Item = (String, SteelVal)>,
    ) -> &mut Self {
        for (name, value) in values {
            self.register_value(name.as_str(), value);
        }
        self
    }

    /// Registers a predicate for a given type. When embedding external values, it is convenient
    /// to be able to have a predicate to test if the given value is the specified type.
    /// In order to be registered, a type must implement [`FromSteelVal`](crate::rvals::FromSteelVal)
    /// and [`IntoSteelVal`](crate::rvals::IntoSteelVal)
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// use steel::steel_vm::register_fn::RegisterFn;
    /// fn foo() -> usize {
    ///    10
    /// }
    ///
    /// let mut vm = Engine::new();
    /// vm.register_fn("foo", foo);
    ///
    /// vm.run(r#"(foo)"#).unwrap(); // Returns vec![10]
    /// ```
    pub fn register_type<T: FromSteelVal + IntoSteelVal>(
        &mut self,
        predicate_name: &'static str,
    ) -> &mut Self {
        let f = move |args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => format!("{} expected 1 argument, got {}", predicate_name, args.len()));
            }

            assert!(args.len() == 1);

            Ok(SteelVal::BoolV(T::from_steelval(&args[0]).is_ok()))
        };

        self.register_value(
            predicate_name,
            SteelVal::BoxedFunction(Rc::new(Box::new(f))),
        )
    }

    // /// Registers a callback function. If registered, this callback will be called on every instruction
    // /// Allows for the introspection of the currently running process. The callback here takes as an argument the current instruction number.
    // ///
    // /// # Examples
    // ///
    // /// ```
    // /// # extern crate steel;
    // /// # use steel::steel_vm::engine::Engine;
    // /// let mut vm = Engine::new();
    // /// vm.on_progress(|count| {
    // ///     // parameter is 'usize' - number of instructions performed up to this point
    // ///     if count % 1000 == 0 {
    // ///         // print out a progress log every 1000 operations
    // ///         println!("Number of instructions up to this point: {}", count);
    // ///         // Returning false here would quit the evaluation of the function
    // ///         return true;
    // ///     }
    // ///     true
    // /// });
    // /// // This should end with "Number of instructions up to this point: 12000"
    // /// vm.run(
    // ///     r#"
    // ///     (define (loop x)
    // ///         (if (equal? x 1000)
    // ///             x
    // ///             (loop (+ x 1))))
    // ///     (loop 0)
    // /// "#,
    // /// )
    // /// .unwrap();
    // /// ```
    // pub fn on_progress<FN: Fn(usize) -> bool + 'static>(&mut self, _callback: FN) -> &mut Self {
    //     // self.virtual_machine.on_progress(callback);
    //     self
    // }

    /// Extracts a value with the given identifier `name` from the internal environment.
    /// If a script calculated some series of bound values, then it can be extracted this way.
    /// This will return the [`SteelVal`](crate::rvals::SteelVal), not the underlying data.
    /// To unwrap the value, use the [`extract`](crate::steel_vm::engine::Engine::extract) method and pass the type parameter.
    ///
    /// The function will return an error if the `name` is not currently bound in the `Engine`'s internal environment.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// use steel::rvals::SteelVal;
    /// let mut vm = Engine::new();
    /// vm.run("(define a 10)").unwrap();
    /// assert_eq!(vm.extract_value("a").unwrap(), SteelVal::IntV(10));
    /// ```
    pub fn extract_value(&self, name: &str) -> Result<SteelVal> {
        let idx = self.compiler.get_idx(name).ok_or_else(throw!(
            Generic => format!("free identifier: {name} - identifier given cannot be found in the global environment")
        ))?;

        self.virtual_machine.extract_value(idx)
            .ok_or_else(throw!(
                Generic => format!("free identifier: {name} - identifier given cannot be found in the global environment")
            ))
    }

    /// Extracts a value with the given identifier `name` from the internal environment, and attempts to coerce it to the
    /// given type. This will return an error if the `name` is not currently bound in the `Engine`'s internal environment, or
    /// if the type passed in does not match the value (and thus the coercion using [`FromSteelVal`](crate::rvals::FromSteelVal) fails)
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new();
    /// vm.run("(define a 10)").unwrap();
    /// assert_eq!(vm.extract::<usize>("a").unwrap(), 10);
    /// ```
    pub fn extract<T: FromSteelVal>(&self, name: &str) -> Result<T> {
        T::from_steelval(&self.extract_value(name)?)
    }

    pub fn raise_error(&self, error: SteelErr) {
        if let Some(span) = error.span() {
            if let Some(source_id) = span.source_id() {
                let file_name = self.sources.get_path(&source_id);

                if let Some(file_content) = self.sources.get(source_id) {
                    // Build stack trace if we have it:
                    if let Some(trace) = error.stack_trace() {
                        // TODO: Flatten recursive calls into the same stack trace
                        // and present the count
                        for dehydrated_context in trace.trace().iter().take(20) {
                            // Report a call stack with whatever we actually have,
                            if let Some(span) = dehydrated_context.span() {
                                if let Some(id) = span.source_id() {
                                    if let Some(source) = self.sources.get(id) {
                                        let trace_line_file_name = self.sources.get_path(&id);

                                        back_trace(
                                            trace_line_file_name
                                                .and_then(|x| x.to_str())
                                                .unwrap_or(""),
                                            source,
                                            *span,
                                        );

                                        // let slice = &source.as_str()[span.range()];

                                        // println!("{}", slice);

                                        // todo!()
                                    }
                                }
                            }

                            // source = self.sources.get(dehydrated_context.)
                        }
                    }

                    error.emit_result(
                        file_name.and_then(|x| x.to_str()).unwrap_or(""),
                        file_content,
                    );
                    return;
                }
            }
        }

        println!("Unable to locate source and span information for this error: {error}");
    }

    /// Execute a program given as the `expr`, and computes a `Vec<SteelVal>` corresponding to the output of each expression given.
    /// This method contains no path information used for error reporting, and simply runs the expression as is. Modules will be
    /// imported with the root directory as wherever the executable was started.
    /// Any parsing, compilation, or runtime error will be reflected here, ideally with span information as well. The error will not
    /// be reported automatically.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// use steel::rvals::SteelVal;
    /// let mut vm = Engine::new();
    /// let output = vm.run("(+ 1 2) (* 5 5) (- 10 5)").unwrap();
    /// assert_eq!(output, vec![SteelVal::IntV(3), SteelVal::IntV(25), SteelVal::IntV(5)]);
    /// ```
    // pub fn run(&mut self, expr: &str) -> Result<Vec<SteelVal>> {
    //     let constants = self.constants();
    //     let program = self.compiler.compile_program(expr, None, constants)?;
    //     self.virtual_machine.execute_program(program)
    // }

    /// Execute a program, however do not run any callbacks as registered with `on_progress`.
    // pub fn run_without_callbacks(&mut self, expr: &str) -> Result<Vec<SteelVal>> {
    //     let constants = self.constants();
    //     let program = self.compiler.compile_program(expr, None, constants)?;
    //     self.virtual_machine
    //         .execute_program::<DoNotUseCallback, ApplyContract>(program)
    // }

    // TODO: Come back to this
    /*
    // / Execute a program (as per [`run`](crate::steel_vm::engine::Engine::run)), however do not enforce any contracts. Any contracts that are added are not
    // / enforced.
    // /
    // / # Examples
    // /
    // / ```
    // / # extern crate steel;
    // / # use steel::steel_vm::engine::Engine;
    // / use steel::rvals::SteelVal;
    // / let mut vm = Engine::new();
    // / let output = vm.run_without_contracts(r#"
    // /        (define/contract (foo x)
    // /           (->/c integer? any/c)
    // /           "hello world")
    // /
    // /        (foo "bad-input")
    // / "#).unwrap();
    // / ```
    // pub fn run_without_contracts(&mut self, expr: &str) -> Result<Vec<SteelVal>> {
    //     let constants = self.constants();
    //     let program = self.compiler.compile_program(expr, None, constants)?;
    //     self.virtual_machine.execute_program::<UseCallback>(program)
    // }
     */

    /// Execute a program without invoking any callbacks, or enforcing any contract checking
    // pub fn run_without_callbacks_or_contracts(&mut self, expr: &str) -> Result<Vec<SteelVal>> {
    //     let constants = self.constants();
    //     let program = self.compiler.compile_program(expr, None, constants)?;
    //     self.virtual_machine
    //         .execute_program::<DoNotUseCallback, DoNotApplyContracts>(program)
    // }

    /// Similar to [`run`](crate::steel_vm::engine::Engine::run), however it includes path information
    /// for error reporting purposes.
    // pub fn run_with_path(&mut self, expr: &str, path: PathBuf) -> Result<Vec<SteelVal>> {
    //     let constants = self.constants();
    //     let program = self.compiler.compile_program(expr, Some(path), constants)?;
    //     self.virtual_machine.execute_program(program)
    // }

    // pub fn compile_and_run_raw_program(&mut self, expr: &str) -> Result<Vec<SteelVal>> {
    //     let constants = self.constants();
    //     let program = self.compiler.compile_program(expr, None, constants)?;
    //     self.virtual_machine.execute_program(program)
    // }

    // pub fn compile_and_run_raw_program(&mut self, expr: &str) -> Result<Vec<SteelVal>> {
    //     self.compile_and_run_raw_program(expr)
    // }

    // Read in the file from the given path and execute accordingly
    // Loads all the functions in from the given env
    // pub fn parse_and_execute_from_path<P: AsRef<Path>>(
    //     &mut self,
    //     path: P,
    // ) -> Result<Vec<SteelVal>> {
    //     let mut file = std::fs::File::open(path)?;
    //     let mut exprs = String::new();
    //     file.read_to_string(&mut exprs)?;
    //     self.compile_and_run_raw_program(exprs.as_str(), )
    // }

    // pub fn parse_and_execute_from_path<P: AsRef<Path>>(
    //     &mut self,
    //     path: P,
    // ) -> Result<Vec<SteelVal>> {
    //     let path_buf = PathBuf::from(path.as_ref());
    //     let mut file = std::fs::File::open(path)?;
    //     let mut exprs = String::new();
    //     file.read_to_string(&mut exprs)?;
    //     self.run_with_path(exprs.as_str(), path_buf)
    // }

    // TODO this does not take into account the issues with
    // people registering new functions that shadow the original one
    fn constants(&mut self) -> ImmutableHashMap<String, SteelVal> {
        if let Some(hm) = self.constants.clone() {
            if !hm.is_empty() {
                return hm;
            }
        }

        let mut hm = ImmutableHashMap::new();
        for constant in CONSTANTS {
            if let Ok(v) = self.extract_value(constant) {
                hm.insert(constant.to_string(), v);
            }
        }
        self.constants = Some(hm.clone());

        hm
    }

    pub fn add_module(&mut self, path: String) -> Result<()> {
        self.compiler
            .compile_module(path.into(), &mut self.sources, self.modules.clone())
    }

    pub fn modules(&self) -> &HashMap<PathBuf, CompiledModule> {
        self.compiler.modules()
    }
}

// #[cfg(test)]
// mod on_progress_tests {
//     use super::*;
//     use std::cell::Cell;
//     use std::rc::Rc;

//     // TODO: At the moment the on progress business is turned off

//     // #[test]
//     // fn count_every_thousand() {
//     //     let mut vm = Engine::new();

//     //     let external_count = Rc::new(Cell::new(0));
//     //     let embedded_count = Rc::clone(&external_count);

//     //     vm.on_progress(move |count| {
//     //         // parameter is 'usize' - number of instructions performed up to this point
//     //         if count % 1000 == 0 {
//     //             // print out a progress log every 1000 operations
//     //             println!("Number of instructions up to this point: {}", count);
//     //             embedded_count.set(embedded_count.get() + 1);

//     //             // Returning false here would quit the evaluation of the function
//     //             return true;
//     //         }
//     //         true
//     //     });

//     //     // This should end with "Number of instructions up to this point: 4000"
//     //     vm.run(
//     //         r#"
//     //         (define (loop x)
//     //             (if (equal? x 1000)
//     //                 x
//     //                 (loop (+ x 1))))
//     //         (displayln (loop 0))
//     //     "#,
//     //     )
//     //     .unwrap();

//     //     assert_eq!(external_count.get(), 4);
//     // }
// }
