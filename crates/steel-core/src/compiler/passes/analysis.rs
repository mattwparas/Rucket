use std::{
    collections::{hash_map, HashSet},
    hash::BuildHasherDefault,
};

// use itertools::Itertools;
use quickscope::ScopeMap;

use crate::{
    parser::{
        ast::{Atom, Define, ExprKind, LambdaFunction, Let, List, Quote},
        parser::{RawSyntaxObject, SyntaxObject, SyntaxObjectId},
        span::Span,
        tokens::TokenType,
    },
    throw, SteelErr,
};

use super::{VisitorMutControlFlow, VisitorMutRefUnit, VisitorMutUnitRef};

use fxhash::{FxHashMap, FxHasher};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IdentifierStatus {
    Global,
    Local,
    LocallyDefinedFunction,
    LetVar,
    Captured,
    Free,
    HeapAllocated,
}

// TODO: Make these not just plain public variables
#[derive(Debug, Clone)]
pub struct SemanticInformation {
    pub kind: IdentifierStatus,
    pub set_bang: bool,
    pub depth: usize,
    pub shadows: Option<SyntaxObjectId>,
    pub usage_count: usize,
    pub span: Span,
    // Referring to a local var definition
    pub refers_to: Option<SyntaxObjectId>,
    // If this is a top level define, what does this alias to?
    pub aliases_to: Option<SyntaxObjectId>,
    pub builtin: bool,
    pub last_usage: bool,
    pub stack_offset: Option<usize>,
    pub escapes: bool,
    // TODO: Move a bunch of these individual things into their own structs
    // something like Option<CaptureInformation>
    pub capture_index: Option<usize>,
    pub read_capture_offset: Option<usize>,
    pub captured_from_enclosing: bool,
    pub heap_offset: Option<usize>,
    pub read_heap_offset: Option<usize>,
}

impl SemanticInformation {
    pub fn new(kind: IdentifierStatus, depth: usize, span: Span) -> Self {
        Self {
            kind,
            set_bang: false,
            depth,
            shadows: None,
            usage_count: 0,
            span,
            refers_to: None,
            aliases_to: None,
            builtin: false,
            last_usage: false,
            stack_offset: None,
            escapes: false,
            capture_index: None,
            read_capture_offset: None,
            captured_from_enclosing: false,
            heap_offset: None,
            read_heap_offset: None,
        }
    }

    pub fn shadows(mut self, id: SyntaxObjectId) -> Self {
        self.shadows = Some(id);
        self
    }

    pub fn with_usage_count(mut self, count: usize) -> Self {
        self.usage_count = count;
        self
    }

    pub fn refers_to(mut self, id: SyntaxObjectId) -> Self {
        self.refers_to = Some(id);
        self
    }

    pub fn aliases_to(mut self, id: SyntaxObjectId) -> Self {
        self.aliases_to = Some(id);
        self
    }

    pub fn mark_builtin(&mut self) {
        self.builtin = true;
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.stack_offset = Some(offset);
        self
    }

    pub fn mark_escapes(&mut self) {
        self.escapes = true;
    }

    pub fn with_capture_index(mut self, offset: usize) -> Self {
        self.capture_index = Some(offset);
        self
    }

    pub fn with_read_capture_offset(mut self, offset: usize) -> Self {
        self.read_capture_offset = Some(offset);
        self
    }

    pub fn with_heap_offset(mut self, offset: usize) -> Self {
        self.heap_offset = Some(offset);
        self
    }

    pub fn with_read_heap_offset(mut self, offset: usize) -> Self {
        self.read_heap_offset = Some(offset);
        self
    }

    pub fn with_captured_from_enclosing(&mut self, captured_from_enclosing: bool) {
        self.captured_from_enclosing = captured_from_enclosing;
    }
}

#[derive(Debug, Clone)]
pub struct FunctionInformation {
    // Just a mapping of the vars to their scope info - holds which vars are being
    // captured by this function
    captured_vars: FxHashMap<String, ScopeInfo>,
    arguments: FxHashMap<String, ScopeInfo>,
    // Keeps a mapping of vars to their scope info, if the variable was mutated
    // if this variable was mutated and inevitably captured, we want to know
    // mutated_vars: HashMap<String, ScopeInfo>,
    // If this function is defined in the tail position / and or the alias to this function escapes,
    // then this should be marked as true
    pub escapes: bool,
    // If this function is bound to a variable, this is the id of that bound value
    pub aliases_to: Option<SyntaxObjectId>,

    // Depth the function definition occurs at
    pub depth: usize,
}

impl FunctionInformation {
    pub fn new(
        captured_vars: FxHashMap<String, ScopeInfo>,
        arguments: FxHashMap<String, ScopeInfo>,
    ) -> Self {
        Self {
            captured_vars,
            arguments,
            escapes: false,
            aliases_to: None,
            depth: 0,
        }
    }

    pub fn captured_vars(&self) -> &FxHashMap<String, ScopeInfo> {
        &self.captured_vars
    }

    pub fn arguments(&self) -> &FxHashMap<String, ScopeInfo> {
        &self.arguments
    }

    pub fn escapes(mut self, escapes: bool) -> Self {
        self.escapes = escapes;
        self
    }

    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum CallKind {
    Normal,
    TailCall,
    SelfTailCall(usize),
}

#[derive(Debug, Clone)]
pub struct CallSiteInformation {
    pub kind: CallKind,
    pub span: Span,
}

impl CallSiteInformation {
    pub fn new(kind: CallKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone)]
pub struct LetInformation {
    pub stack_offset: usize,
    pub function_context: Option<usize>,
    pub arguments: FxHashMap<String, ScopeInfo>,
}

impl LetInformation {
    pub fn new(
        stack_offset: usize,
        function_context: Option<usize>,
        arguments: FxHashMap<String, ScopeInfo>,
    ) -> Self {
        Self {
            stack_offset,
            function_context,
            arguments,
        }
    }
}

// Populate the metadata about individual
#[derive(Default, Debug, Clone)]
pub struct Analysis {
    // TODO: make these be specific IDs for semantic id, function id, and call info id
    pub(crate) info: FxHashMap<SyntaxObjectId, SemanticInformation>,
    pub(crate) function_info: FxHashMap<usize, FunctionInformation>,
    pub(crate) call_info: FxHashMap<usize, CallSiteInformation>,
    pub(crate) let_info: FxHashMap<usize, LetInformation>,
}

impl Analysis {
    pub fn from_exprs(exprs: &[ExprKind]) -> Self {
        let mut analysis = Analysis::default();
        analysis.run(exprs);
        analysis
    }

    pub fn populate_captures(&mut self, exprs: &[ExprKind]) {
        // Resolve all mutated and captured vars so that they're mutated after they've been captured
        let mutated_and_captured_vars = self
            .function_info
            .values()
            .flat_map(|x| x.captured_vars.values())
            .chain(self.let_info.values().flat_map(|x| x.arguments.values()))
            .filter(|x| x.captured && x.mutated)
            .map(|x| (x.id, x.clone()))
            .collect::<std::collections::HashMap<_, _>>();

        self.function_info
            .values_mut()
            .flat_map(|x| x.captured_vars.values_mut())
            .for_each(|x| {
                if mutated_and_captured_vars.get(&x.id).is_some() {
                    x.mutated = true;
                    x.captured = true;
                }
            });

        self.function_info
            .values_mut()
            .flat_map(|x| x.arguments.values_mut())
            .for_each(|x| {
                if mutated_and_captured_vars.get(&x.id).is_some() {
                    x.mutated = true;
                    x.captured = true;
                }
            });

        self.run(exprs);
    }

    pub fn resolve_alias(&self, mut id: SyntaxObjectId) -> Option<SyntaxObjectId> {
        while let Some(next) = self
            .info
            .get(&id)
            .and_then(|x| x.aliases_to)
            .and_then(|x| self.info.get(&x))
            .and_then(|x| x.refers_to)
        {
            id = next;
        }

        Some(id)
    }

    pub fn visit_top_level_define_function_without_body(
        &mut self,
        scope: &mut ScopeMap<String, ScopeInfo>,
        define: &crate::parser::ast::Define,
    ) {
        let name = define.name.atom_identifier().unwrap();

        let mut semantic_info = SemanticInformation::new(
            IdentifierStatus::Global,
            1,
            define.name.atom_syntax_object().unwrap().span,
        );

        if define.is_a_builtin_definition() {
            semantic_info.mark_builtin();
        }

        // If this variable name is already in scope, we should mark that this variable
        // shadows the previous id
        if let Some(shadowed_var) = scope.get(name) {
            semantic_info = semantic_info.shadows(shadowed_var.id)
        }

        log::info!("Defining global: {:?}", define.name);
        define_var(scope, define);

        self.insert(define.name.atom_syntax_object().unwrap(), semantic_info);
    }

    pub fn run(&mut self, exprs: &[ExprKind]) {
        let mut scope: ScopeMap<String, ScopeInfo> = ScopeMap::new();

        // TODO: Functions should be globally resolvable but top level identifiers cannot be used before they are defined
        // The way this is implemented right now doesn't respect that
        for expr in exprs.iter() {
            if let ExprKind::Define(define) = expr {
                if define.body.lambda_function().is_some() {
                    self.visit_top_level_define_function_without_body(&mut scope, define);
                }
            }
        }

        for expr in exprs {
            let mut pass = AnalysisPass::new(self, &mut scope);
            // pass.visit(expr);

            if let ExprKind::Define(define) = expr {
                if define.body.lambda_function().is_some() {
                    // Since we're at the top level, care should be taken to actually
                    // refer to the defining context correctly
                    pass.defining_context = define.name_id();
                    pass.defining_context_depth = 0;
                    // Continue with the rest of the body here
                    pass.visit(&define.body);
                    pass.defining_context = None;
                    // pass.defining_context_depth = 0;
                } else {
                    pass.visit_top_level_define_value_without_body(define);
                    pass.visit(&define.body);
                }
            } else {
                pass.visit(expr);
            }
        }

        log::info!("Global scope: {:?}", scope.iter_top().collect::<Vec<_>>());
    }

    pub fn get_function_info(&self, function: &LambdaFunction) -> Option<&FunctionInformation> {
        self.function_info.get(&function.syntax_object_id)
    }

    pub fn insert(&mut self, object: &SyntaxObject, metadata: SemanticInformation) {
        self.info.insert(object.syntax_object_id, metadata);
    }

    pub fn update_with(&mut self, object: &SyntaxObject, metadata: SemanticInformation) {
        let mut existing = self.info.get_mut(&object.syntax_object_id).unwrap();
        existing.kind = metadata.kind;
        existing.set_bang = existing.set_bang || metadata.set_bang;
        existing.shadows = metadata.shadows;
        existing.depth = metadata.depth;
        existing.usage_count = metadata.usage_count;
        existing.aliases_to = metadata.aliases_to;
        existing.refers_to = metadata.refers_to;
        existing.builtin = metadata.builtin;
        existing.captured_from_enclosing = metadata.captured_from_enclosing;
        existing.heap_offset = metadata.heap_offset;
        existing.read_heap_offset = metadata.read_heap_offset;
    }

    pub fn get(&self, object: &SyntaxObject) -> Option<&SemanticInformation> {
        self.info.get(&object.syntax_object_id)
    }

    pub fn get_mut(&mut self, id: &SyntaxObjectId) -> Option<&mut SemanticInformation> {
        self.info.get_mut(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeInfo {
    /// The ID of the variable, this is globally unique
    pub id: SyntaxObjectId,
    /// Whether or not this variable is captured by a scope
    /// TODO: This needs to actually just mark the depth at which variable was captured, or something to signify
    /// if a specific scope actually _uses_ the variable
    pub captured: bool,
    /// How many times has this variable been referenced
    pub usage_count: usize,
    /// Last touched by this ID
    pub last_used: Option<SyntaxObjectId>,
    /// Represents the position on the stack that this variable
    /// should live at during the execution of the program
    pub stack_offset: Option<usize>,
    /// Does this variable escape its scope? As in, does the value outlive the scope
    /// that it was defined in
    pub escapes: bool,
    /// If this is a captured var, the capture index
    pub capture_offset: Option<usize>,
    /// Was this var captured from the stack or from an enclosing function
    pub captured_from_enclosing: bool,
    /// Was this var mutated?
    pub mutated: bool,
    /// Heap offset
    pub heap_offset: Option<usize>,
    pub read_capture_offset: Option<usize>,
    pub read_heap_offset: Option<usize>,
    pub parent_heap_offset: Option<usize>,
    pub local_heap_offset: Option<usize>,
}

impl ScopeInfo {
    pub fn new(id: SyntaxObjectId) -> Self {
        Self {
            id,
            captured: false,
            usage_count: 0,
            last_used: None,
            stack_offset: None,
            escapes: false,
            capture_offset: None,
            captured_from_enclosing: false,
            mutated: false,
            heap_offset: None,
            read_capture_offset: None,
            read_heap_offset: None,
            parent_heap_offset: None,
            local_heap_offset: None,
        }
    }

    pub fn new_local(id: SyntaxObjectId, offset: usize) -> Self {
        Self {
            id,
            captured: false,
            usage_count: 0,
            last_used: None,
            stack_offset: Some(offset),
            escapes: false,
            capture_offset: None,
            captured_from_enclosing: false,
            mutated: false,
            heap_offset: None,
            read_capture_offset: None,
            read_heap_offset: None,
            parent_heap_offset: None,
            local_heap_offset: None,
        }
    }

    pub fn new_heap_allocated_var(
        id: SyntaxObjectId,
        stack_offset: usize,
        heap_offset: usize,
    ) -> Self {
        Self {
            id,
            captured: true,
            usage_count: 0,
            last_used: None,
            stack_offset: Some(stack_offset),
            escapes: false,
            capture_offset: None,
            captured_from_enclosing: false,
            mutated: true,
            heap_offset: Some(heap_offset),
            read_capture_offset: None,
            read_heap_offset: Some(heap_offset),
            parent_heap_offset: None,
            local_heap_offset: None,
        }
    }
}

struct AnalysisPass<'a> {
    info: &'a mut Analysis,
    scope: &'a mut ScopeMap<String, ScopeInfo>,
    captures: ScopeMap<String, ScopeInfo>,
    tail_call_eligible: bool,
    escape_analysis: bool,
    defining_context: Option<SyntaxObjectId>,
    // TODO: This should give us the depth (how many things we need to roll back)
    defining_context_depth: usize,
    stack_offset: usize,
    function_context: Option<usize>,
    contains_lambda_func: bool,
    vars_used: im_rc::HashSet<String>,
    total_vars_used: im_rc::HashSet<String>,
    ids_referenced_in_tail_position: HashSet<SyntaxObjectId>,
}

fn define_var(scope: &mut ScopeMap<String, ScopeInfo>, define: &crate::parser::ast::Define) {
    scope.define(
        define.name.atom_identifier().unwrap().to_string(),
        ScopeInfo::new(define.name.atom_syntax_object().unwrap().syntax_object_id),
    );
}

impl<'a> AnalysisPass<'a> {
    pub fn new(info: &'a mut Analysis, scope: &'a mut ScopeMap<String, ScopeInfo>) -> Self {
        AnalysisPass {
            info,
            scope,
            captures: ScopeMap::default(),
            tail_call_eligible: false,
            escape_analysis: false,
            defining_context: None,
            defining_context_depth: 0,
            stack_offset: 0,
            function_context: None,
            contains_lambda_func: false,
            vars_used: im_rc::HashSet::new(),
            total_vars_used: im_rc::HashSet::new(),
            ids_referenced_in_tail_position: HashSet::new(),
        }
    }
}

impl<'a> AnalysisPass<'a> {
    // TODO: This needs to be fixed with interning
    fn _get_possible_captures(&self, let_level_bindings: &[&str]) -> HashSet<String> {
        self.scope
            .iter()
            .filter(|x| !x.1.captured)
            .filter(|x| !let_level_bindings.contains(&x.0.as_str()))
            .map(|x| x.0.clone())
            .collect()
    }

    fn get_captured_vars(&self, let_level_bindings: &[&str]) -> FxHashMap<String, ScopeInfo> {
        self.scope
            .iter()
            .filter(|x| x.1.captured)
            .filter(|x| !let_level_bindings.contains(&x.0.as_str()))
            .map(|x| (x.0.clone(), x.1.clone()))
            .collect()
    }

    fn visit_top_level_define_value_without_body(&mut self, define: &crate::parser::ast::Define) {
        let name = define.name.atom_identifier().unwrap();

        let name_syntax_object = define.name.atom_syntax_object().unwrap();

        let mut semantic_info =
            SemanticInformation::new(IdentifierStatus::Global, 1, name_syntax_object.span);

        // If this variable name is already in scope, we should mark that this variable
        // shadows the previous id
        if let Some(shadowed_var) = self.scope.get(name) {
            semantic_info = semantic_info.shadows(shadowed_var.id)
        }

        if define.is_a_builtin_definition() {
            semantic_info.mark_builtin();
        }

        if let Some(aliases) = define.is_an_alias_definition() {
            log::info!(
                "Found definition that aliases - {} aliases {}: {:?} -> {:?}",
                define.name,
                define.body,
                name_syntax_object.syntax_object_id,
                define.body.atom_syntax_object().unwrap().syntax_object_id,
            );
            semantic_info = semantic_info.aliases_to(aliases);
        }

        log::info!("Defining global: {:?}", define.name);
        define_var(self.scope, define);

        self.info.insert(name_syntax_object, semantic_info);
    }

    // TODO: I really hate this identifier status if local nonsense
    fn visit_define_without_body(
        &mut self,
        define: &crate::parser::ast::Define,
        identifier_status_if_local: IdentifierStatus,
    ) {
        let name = define.name.atom_identifier().unwrap();

        let mut semantic_info = SemanticInformation::new(
            if self.scope.depth() == 1 {
                IdentifierStatus::Global
            } else {
                identifier_status_if_local
            },
            self.scope.depth(),
            define.name.atom_syntax_object().unwrap().span,
        );

        if define.is_a_builtin_definition() {
            semantic_info.mark_builtin();
        }

        // If this variable name is already in scope, we should mark that this variable
        // shadows the previous id
        if let Some(shadowed_var) = self.scope.get(name) {
            log::warn!("Redefining previous variable: {:?}", name);
            semantic_info = semantic_info.shadows(shadowed_var.id)
        }

        define_var(self.scope, define);

        self.info
            .insert(define.name.atom_syntax_object().unwrap(), semantic_info);
    }

    // Visit the function arguments, marking these as defining in our scope
    // and also defaulting them to be local identifiers. This way, in the event of a set!
    // we have something to refer to
    fn visit_func_args(&mut self, lambda_function: &LambdaFunction, depth: usize) {
        // let function_info = self
        //     .info
        //     .function_info
        //     .get(&lambda_function.syntax_object_id);

        let alloc_capture_count = self
            .info
            .function_info
            .get(&lambda_function.syntax_object_id)
            .map(|x| {
                x.captured_vars()
                    .values()
                    .filter(|x| x.captured && x.mutated)
                    .count()
            });

        let mut mut_var_offset = 0;

        for (index, arg) in lambda_function.args.iter().enumerate() {
            let name = arg.atom_identifier().unwrap();
            let id = arg.atom_syntax_object().unwrap().syntax_object_id;

            // TODO: Don't need to do these repeated hash lookups over and over
            // can coalesce this into one at the top of the args
            let heap_alloc = if let Some(info) = self
                .info
                .function_info
                .get(&lambda_function.syntax_object_id)
            {
                if let Some(info) = info.arguments.get(name) {
                    // println!("Found information: {:#?}", info);
                    info.mutated && info.captured
                } else {
                    false
                }
            } else {
                false
            };

            // TODO: clean this up like a lot
            if heap_alloc {
                self.scope.define(
                    name.to_string(),
                    ScopeInfo::new_heap_allocated_var(
                        id,
                        index,
                        mut_var_offset + alloc_capture_count.unwrap(),
                    ),
                );

                // Throw in a dummy info so that no matter what, we have something to refer to
                // in the event of a set!
                // Later on in this function this gets updated accordingly
                self.info.insert(
                    arg.atom_syntax_object().unwrap(),
                    SemanticInformation::new(
                        IdentifierStatus::HeapAllocated,
                        depth,
                        arg.atom_syntax_object().unwrap().span,
                    ),
                );

                mut_var_offset += 1;
            } else {
                self.scope
                    .define(name.to_string(), ScopeInfo::new_local(id, index));

                // Throw in a dummy info so that no matter what, we have something to refer to
                // in the event of a set!
                // Later on in this function this gets updated accordingly
                self.info.insert(
                    arg.atom_syntax_object().unwrap(),
                    SemanticInformation::new(
                        IdentifierStatus::Local,
                        depth,
                        arg.atom_syntax_object().unwrap().span,
                    ),
                );
            }
        }
    }

    fn pop_top_layer(&mut self) -> FxHashMap<String, ScopeInfo> {
        let arguments = self
            .scope
            .iter_top()
            // .cloned()
            .map(|x| (x.0.clone(), x.1.clone()))
            .collect::<FxHashMap<_, _>>();

        self.scope.pop_layer();

        arguments
    }

    fn find_and_mark_captured_arguments(
        &mut self,
        lambda_function: &LambdaFunction,
        captured_vars: &FxHashMap<String, ScopeInfo>,
        depth: usize,
        arguments: &FxHashMap<String, ScopeInfo>,
    ) {
        for var in &lambda_function.args {
            let ident = var.atom_identifier().unwrap();

            // let mut heap_offset = None;
            // let mut read_heap_offset = None;

            let kind = if let Some(info) = captured_vars.get(ident) {
                if info.mutated {
                    // heap_offset = info.heap_offset;
                    // read_heap_offset = info.read_heap_offset;

                    IdentifierStatus::HeapAllocated
                } else {
                    IdentifierStatus::Captured
                }
            } else if let Some(info) = self.info.get(var.atom_syntax_object().unwrap()) {
                match info.kind {
                    IdentifierStatus::HeapAllocated => {
                        // heap_offset = info.heap_offset;
                        // read_heap_offset = info.read_heap_offset;

                        IdentifierStatus::HeapAllocated
                    }
                    // IdentifierStatus::Captured => IdentifierStatus::Captured,
                    _ => IdentifierStatus::Local,
                }
            } else {
                IdentifierStatus::Local
            };

            // let kind = if captured_vars.contains_key(ident) {
            //     IdentifierStatus::Captured
            // } else {
            //     IdentifierStatus::Local
            // };

            let mut semantic_info =
                SemanticInformation::new(kind, depth, var.atom_syntax_object().unwrap().span);

            // Update the usage count to collect how many times the variable was referenced
            // Inside of the scope in which the variable existed
            // TODO: merge this into one
            let count = arguments.get(ident).unwrap().usage_count;

            if count == 0 {
                // TODO: Emit warning with the span
                log::warn!("Found unused argument: {:?}", ident);
            }

            // if kind == IdentifierStatus::HeapAllocated {
            //     semantic_info = semantic_info.with_heap_offset(heap_offset.unwrap());
            //     semantic_info = semantic_info.with_read_heap_offset(read_heap_offset.unwrap());
            // }

            semantic_info = semantic_info.with_usage_count(count);

            // If this variable name is already in scope, we should mark that this variable
            // shadows the previous id
            if let Some(shadowed_var) = self.scope.get(ident) {
                semantic_info = semantic_info.shadows(shadowed_var.id)
            }

            // if let Some(info) = self.info.get(&var.atom_syntax_object().unwrap()) {
            //     println!("{:#?}", info);
            // }

            self.info
                .update_with(var.atom_syntax_object().unwrap(), semantic_info);
        }
    }

    fn visit_with_tail_call_eligibility(&mut self, expr: &'a ExprKind, state: bool) {
        let eligibility = self.tail_call_eligible;
        self.tail_call_eligible = state;
        self.visit(expr);
        self.tail_call_eligible = eligibility;
    }
}

impl<'a> VisitorMutUnitRef<'a> for AnalysisPass<'a> {
    // TODO: define expressions are not handled by this for stack offset purposes
    fn visit_define(&mut self, define: &'a crate::parser::ast::Define) {
        self.visit_define_without_body(define, IdentifierStatus::Local);

        let define_ctx = self.defining_context.take();

        // Mark defining context
        self.defining_context = define.name_id();
        self.visit_with_tail_call_eligibility(&define.body, false);
        self.defining_context = define_ctx;
    }

    // Quoted values are just constants - lets ignore them for now?
    fn visit_quote(&mut self, _quote: &'a crate::parser::ast::Quote) {}

    fn visit_if(&mut self, f: &'a crate::parser::ast::If) {
        // Explicitly disallow a tail call in the test expression
        // There is no way that this could be a tail call

        self.visit_with_tail_call_eligibility(&f.test_expr, false);

        self.visit(&f.then_expr);
        self.visit(&f.else_expr);
    }

    fn visit_list(&mut self, l: &'a List) {
        let eligibility = self.tail_call_eligible;
        let escape = self.escape_analysis;

        // In this case, each of the arguments (including the function itself) are not in the tail position
        // However, the function call _itself_ might be in the tail position, so we save that state
        self.tail_call_eligible = false;

        // Save the spot in the recursion - here we might be in something like this:
        // (lambda (x y)
        //      (+ 10 (let ((z (function-call))) (+ x y z))))
        // In this case the 10 will be on the stack, but will be reset after
        // the function call.
        let stack_offset = self.stack_offset;

        // let mut last_used_vars = HashSet::new();

        // // Find each variable that is actually referenced in this tail position
        // // let mut last_used_vars = HashSet::new();
        // std::mem::swap(
        //     &mut self.ids_referenced_in_tail_position,
        //     &mut last_used_vars,
        // );

        for expr in &l.args[1..] {
            self.escape_analysis = true;
            // self.stack_offset += 1;
            self.visit(expr);

            // println!("Stack offset: {:?}", self.stack_offset);

            self.stack_offset += 1;
            // println!("Visiting argument: {}", expr);
            // self.escape_analysis = true;
        }

        if !l.is_empty() {
            // self.tail_call_eligible = eligibility;
            self.escape_analysis = eligibility;

            if let ExprKind::LambdaFunction(_) = &l.args[0] {
                self.escape_analysis = false;
            }

            // println!(
            //     "Visiting argument: {}, with tail call eligibility: {} and escape: {}",
            //     &l.args[0], self.tail_call_eligible, self.escape_analysis
            // );

            self.visit(&l.args[0]);
        }

        // std::mem::swap(
        //     &mut self.ids_referenced_in_tail_position,
        //     &mut last_used_vars,
        // );

        // self.ids_referenced_in_tail_position = HashSet::new();

        self.stack_offset = stack_offset;
        self.tail_call_eligible = eligibility;
        self.escape_analysis = escape;

        // TODO: Come back here on cleanup
        // This just checks that this is actually a real function call and not just an empty list
        // Every actual call site should in fact have a real span, otherwise its a bit of a waste to include
        // that information - it _has_ to at least be calling something
        if !l.is_empty() {
            // Mark the call site - see what happens
            let mut call_site_kind = if eligibility && self.scope.depth() > 1 {
                // Update the last usage of any of these variables now...
                // TODO: This doesn't seem like it will work.
                // for id in self.scope.iter().map(|x| x.1).filter_map(|x| x.last_used) {
                //     self.info.get_mut(&id).unwrap().last_usage = true;
                // }

                // for id in last_used_vars {
                //     self.info.get_mut(&id).unwrap().last_usage = true;
                // }

                CallKind::TailCall
            } else {
                CallKind::Normal
            };

            let syntax_object = l.first().and_then(|x| x.atom_syntax_object());

            if let Some(func) = syntax_object {
                let span = func.span;

                // Assuming this information is here, otherwise we'll panic for whatever reason the symbol is missing
                // But this shouldn't happen given that we're checking this _after_ we visit the function
                let func_info = self.info.get(func).unwrap();

                // If we've managed to resolve this call site to the definition, then we should be able
                // to identify if this refers to the correct definition
                if call_site_kind == CallKind::TailCall
                    && self.defining_context.is_some()
                    && func_info.refers_to == self.defining_context
                {
                    call_site_kind = CallKind::SelfTailCall(self.defining_context_depth);
                }

                self.info.call_info.insert(
                    l.syntax_object_id,
                    CallSiteInformation::new(call_site_kind, span),
                );
            }
        }
    }

    // TODO: -> understand stack offset here. Should begins drop everything inside?
    fn visit_begin(&mut self, begin: &'a crate::parser::ast::Begin) {
        // Collect all of the defines inside of the body first
        for expr in &begin.exprs {
            if let ExprKind::Define(define) = expr {
                if define.body.lambda_function().is_some() {
                    self.visit_define_without_body(
                        define,
                        IdentifierStatus::LocallyDefinedFunction,
                    );
                }
            }
        }

        let last = begin.exprs.len() - 1;
        // let stack_offset = self.stack_offset;

        // TODO: Clean up this bad pattern
        let eligibility = self.tail_call_eligible;
        self.tail_call_eligible = false;

        // After that, we can continue with everything but those
        for (index, expr) in begin.exprs.iter().enumerate() {
            if index == last {
                self.tail_call_eligible = eligibility;
            }

            if let ExprKind::Define(define) = expr {
                let define_ctx = self.defining_context.take();
                let old_depth = self.defining_context_depth;
                self.defining_context = define.name_id();
                self.defining_context_depth = 0;

                if define.body.lambda_function().is_some() {
                    // Continue with the rest of the body here
                    self.visit(&define.body);
                } else {
                    self.visit(expr);
                }

                self.defining_context = define_ctx;
                self.defining_context_depth = old_depth;
            } else {
                self.visit(expr);
            }

            self.tail_call_eligible = false;
            // self.stack_offset += 1;
        }

        // Overall, 1 for the total

        if !begin.exprs.is_empty() {
            self.stack_offset += 1;
        }

        self.tail_call_eligible = eligibility;
        // self.stack_offset = stack_offset
    }

    #[allow(dead_code, unused)]
    fn visit_let(&mut self, l: &'a crate::parser::ast::Let) {
        let eligibility = self.tail_call_eligible;
        self.tail_call_eligible = false;

        let mut stack_offset = self.stack_offset;
        let rollback_offset = stack_offset;

        for expr in l.expression_arguments() {
            self.visit(expr);
            self.stack_offset += 1;
        }

        self.tail_call_eligible = eligibility;

        let is_top_level = self.scope.depth() == 1;

        if is_top_level {
            self.scope.push_layer();
        }

        let alloc_capture_count = self
            .function_context
            .and_then(|x| self.info.function_info.get(&x))
            .map(|x| {
                x.captured_vars()
                    .values()
                    .filter(|x| x.captured && x.mutated)
                    .count()
            });

        // We don't want to include normal variables when keeping track
        // of the offset
        let mut mutable_var_offset = 0;

        for arg in l.local_bindings() {
            let name = arg.atom_identifier().unwrap();
            let id = arg.atom_syntax_object().unwrap().syntax_object_id;
            // println!("Inserting local: {:?} at offset: {}", arg, stack_offset);

            let heap_alloc = if let Some(info) = self.info.let_info.get(&l.syntax_object_id) {
                if let Some(info) = info.arguments.get(name) {
                    info.mutated && info.captured
                } else {
                    false
                }
            } else {
                false
            };

            #[allow(clippy::diverging_sub_expression)]
            if heap_alloc {
                self.scope.define(
                    name.to_string(),
                    ScopeInfo::new_heap_allocated_var(
                        id,
                        todo!("Need to include the stack offset here!"),
                        mutable_var_offset + alloc_capture_count.unwrap(),
                    ),
                );

                // Throw in a dummy info so that no matter what, we have something to refer to
                // in the event of a set!
                // Later on in this function this gets updated accordingly
                self.info.insert(
                    arg.atom_syntax_object().unwrap(),
                    SemanticInformation::new(
                        IdentifierStatus::HeapAllocated,
                        self.scope.depth(),
                        arg.atom_syntax_object().unwrap().span,
                    ),
                );

                mutable_var_offset += 1;
            } else {
                self.scope
                    .define(name.to_string(), ScopeInfo::new_local(id, stack_offset));

                self.info.insert(
                    arg.atom_syntax_object().unwrap(),
                    SemanticInformation::new(
                        IdentifierStatus::LetVar,
                        self.scope.depth(),
                        arg.atom_syntax_object().unwrap().span,
                    ),
                );
            }

            stack_offset += 1;
        }

        self.visit(&l.body_expr);

        // This is a little silly but I'm not sure why I can't call the default method on the FxHashMap directly
        let mut arguments = FxHashMap::with_capacity_and_hasher(
            l.bindings.len(),
            BuildHasherDefault::<FxHasher>::default(),
        );

        for arg in l.local_bindings() {
            let name = arg.atom_identifier().unwrap();

            arguments.insert(name.to_string(), self.scope.remove(name).unwrap());
        }

        if let hash_map::Entry::Vacant(e) = self.info.let_info.entry(l.syntax_object_id) {
            e.insert(LetInformation::new(
                self.stack_offset,
                self.function_context,
                arguments,
            ));
        }

        if is_top_level {
            self.scope.pop_layer();
        }

        self.stack_offset = rollback_offset;
    }

    fn visit_lambda_function(&mut self, lambda_function: &'a crate::parser::ast::LambdaFunction) {
        let stack_offset_rollback = self.stack_offset;

        // The captures correspond to what variables _this_ scope should decide to capture, and also
        // arbitrarily decide the index for that capture
        self.captures.push_layer();

        // In this case, we've actually already seen this function. This works if we're on the second pass
        // of running this analysis with the same information
        if let Some(function_info) = self
            .info
            .function_info
            .get_mut(&lambda_function.syntax_object_id)
        {
            // TODO: see if this was necessary
            if function_info.escapes {
                // println!("Function escapes!");
                self.defining_context = None;
            }

            let vars = &mut function_info.captured_vars;

            // TODO:
            // If this var is both captured and mutated, lets separate it for a different kind
            // of allocation - this way we can actually separately allocate where these go. Since it is possible
            // that a variable is both captured, and mutated by both the closure and the stack, we want to
            // make sure that things that get captured and mutated end up on the heap, and both references
            // point to the same thing

            // Handle the immutable variables being patched in
            {
                let mut sorted_vars = vars.iter_mut().filter(|x| !x.1.mutated).collect::<Vec<_>>();
                sorted_vars.sort_by_key(|x| x.1.id);

                // So for now, we sort by id, then map these directly to indices that will live in the
                // corresponding captured closure
                for (index, (key, value)) in sorted_vars.iter_mut().enumerate() {
                    // value.capture_offset = Some(index);
                    // value.read_capture_offset = Some(index);

                    // If we've already captured this variable, mark it as being captured from the enclosing environment
                    // TODO: If there is shadowing, this might not work?
                    if let Some(captured_var) = self.captures.get(key.as_str()) {
                        // TODO: If the key already exists, we need to check if we're shadowing
                        // the variable - I think we can do this by checking what the variable shadows?

                        // Check if this variable shadows another one. If so, we defer to the
                        // notion that this is a fresh variable
                        // if let Some(analysis) = self.captures.contains_key_at_top(key) {
                        if self.captures.depth_of(key.as_str()).unwrap() > 1 {
                            // todo!()

                            // if analysis.shadows.is_some() {
                            // println!("Found a shadowed var!: {}", key);

                            value.capture_offset = Some(index);
                            value.read_capture_offset = Some(index);
                            let mut value = value.clone();
                            value.captured_from_enclosing = false;

                            self.captures.define(key.clone(), value);

                            continue;
                            // }
                        }

                        value.capture_offset = captured_var.read_capture_offset;

                        value.read_capture_offset = Some(index);

                        let mut value = value.clone();
                        value.captured_from_enclosing = true;

                        // println!("Marking var as captured from the enclosing: {}", key);
                        // println!("value: {:#?}", value);

                        self.captures.define(key.clone(), value)
                    } else {
                        value.capture_offset = Some(index);
                        value.read_capture_offset = Some(index);
                        let mut value = value.clone();
                        value.captured_from_enclosing = false;

                        self.captures.define(key.clone(), value);
                    }
                }
            }

            {
                let mut captured_and_mutated =
                    vars.iter_mut().filter(|x| x.1.mutated).collect::<Vec<_>>();
                captured_and_mutated.sort_by_key(|x| x.1.id);

                // println!("Captured and mutated: {:?}", captured_and_mutated);

                for (index, (key, value)) in captured_and_mutated.iter_mut().enumerate() {
                    // value.heap_offset = Some(index);

                    // If we've already captured this variable, mark it as being captured from the enclosing environment
                    // TODO: If there is shadowing, this might not work?
                    if self.captures.contains_key(key.as_str()) {
                        if self.captures.depth_of(key.as_str()).unwrap() > 1 {
                            value.heap_offset = value.stack_offset;
                            value.read_heap_offset = Some(index);

                            value.parent_heap_offset = value.stack_offset;
                            value.local_heap_offset = Some(index);

                            let mut value = value.clone();
                            value.captured_from_enclosing = false;

                            self.captures.define(key.clone(), value);

                            continue;
                        }

                        // TODO: If theres weird bugs here, match behavior of the captures from above
                        // i.e. heap offset just patches in the read offset from the parent
                        value.heap_offset = self
                            .captures
                            .get(key.as_str())
                            .and_then(|x| x.read_heap_offset);

                        value.read_heap_offset = self
                            .captures
                            .get(key.as_str())
                            .and_then(|x| x.read_heap_offset);

                        value.read_heap_offset = Some(index);

                        value.parent_heap_offset = self
                            .captures
                            .get(key.as_str())
                            .and_then(|x| x.local_heap_offset);

                        value.local_heap_offset = Some(index);

                        let mut value = value.clone();
                        value.captured_from_enclosing = true;

                        self.captures.define(key.clone(), value);
                    } else {
                        value.heap_offset = value.stack_offset;
                        value.read_heap_offset = Some(index);

                        value.parent_heap_offset = value.stack_offset;
                        value.local_heap_offset = Some(index);

                        let mut value = value.clone();
                        value.captured_from_enclosing = false;

                        self.captures.define(key.clone(), value);
                    }
                }
            }
        }

        // Before we enter this, these are all of the variables that should be reset after capturing
        // let uncaptured_from_latest_layer: HashSet<String> = self
        //     .scope
        //     .iter_top()
        //     .filter(|x| x.1.captured)
        //     .map(|x| x.0.clone())
        //     .collect();

        // We're entering a new scope since we've entered a lambda function
        self.scope.push_layer();

        let let_level_bindings = lambda_function.arguments().unwrap();
        let depth = self.scope.depth();

        self.visit_func_args(lambda_function, depth);

        self.stack_offset = lambda_function.args.len();

        let function_context = self.function_context.take();
        self.function_context = Some(lambda_function.syntax_object_id);

        self.defining_context_depth += 1;

        // Try to extract the actual variables that are used
        // Otherwise, we're capturing an insane amount of extra variables
        // let mut used_vars = im_rc::HashSet::new();

        // Save the state of things that have been used on the way down
        // let mut overall_used_down = self.vars_used.clone();

        // Set the single used to this scope to be a new set
        self.vars_used = im_rc::HashSet::new();

        // std::mem::swap(&mut self.vars_used, &mut used_vars);

        // These are the possible captures that _could_ happen
        // let possible_captures = self.get_possible_captures(&let_level_bindings);

        self.contains_lambda_func = false;

        // TODO: Better abstract this pattern - perhaps have the function call be passed in?
        self.visit_with_tail_call_eligibility(&lambda_function.body, true);

        let lambda_bottoms_out = !self.contains_lambda_func;

        self.contains_lambda_func = true;

        // Put it back
        // std::mem::swap(&mut self.vars_used, &mut used_vars);

        // self.vars_used += used_vars.clone();

        for var in &self.vars_used {
            self.total_vars_used.insert(var.clone());
        }

        self.defining_context_depth -= 1;

        self.function_context = function_context;

        // TODO: @Matt: 11/3/2022 -> Seems like theres kind of a bad problem here.
        // This map that we're getting is much bigger than expected
        // Perhaps take the diff of the vars before visiting this, and after? Then reset the state after visiting this tree?
        let mut captured_vars = self.get_captured_vars(&let_level_bindings);

        // println!("{:?}", )

        for (var, value) in self.captures.iter() {
            if let Some(scope_info) = captured_vars.get_mut(var.as_str()) {
                scope_info.captured_from_enclosing = value.captured_from_enclosing;
            }
        }

        log::info!("Captured variables: {:?}", captured_vars);
        // println!("Captured variables: {:?}", captured_vars);

        // Get the arguments to get the counts
        // Pop the layer here - now, we check if any of the arguments below actually already exist
        // in scope. If thats the case, we've shadowed and should mark it accordingly.
        let arguments = self.pop_top_layer();

        // Pop off of the captures
        self.captures.pop_layer();

        // println!("Captures: {:#?}", self.captures.iter().collect::<Vec<_>>());

        // Mark the last usage of the variable after the values go out of scope
        // TODO: This should get moved to every tail call -> if its a tail call, mark
        // the last usage of the variables there. That way, all exit points of the function
        // actually get marked

        for id in arguments.values().filter_map(|x| x.last_used) {
            self.info.get_mut(&id).unwrap().last_usage = true;
        }

        // println!("{:#?}", arguments);

        // Using the arguments, mark the vars that have been captured
        self.find_and_mark_captured_arguments(lambda_function, &captured_vars, depth, &arguments);

        // If we've already seen this function, lets not do anything just quite yet
        if let Some(info) = self
            .info
            .function_info
            .get_mut(&lambda_function.syntax_object_id)
        {
            let mut slated_for_removal = Vec::new();

            if lambda_bottoms_out {
                captured_vars.retain(|x: &String, _| self.vars_used.contains(x));

                for var in info.captured_vars.keys() {
                    if !captured_vars.contains_key(var) {
                        slated_for_removal.push(var.clone());
                    }
                }

                for var in slated_for_removal {
                    info.captured_vars.remove(&var);
                }

                // println!("Captured vars dropped: {}", before - captured_vars.len());
            }

            for (var, value) in captured_vars {
                if let Some(scope_info) = info.captured_vars.get_mut(var.as_str()) {
                    scope_info.captured_from_enclosing = value.captured_from_enclosing;
                    scope_info.heap_offset = value.heap_offset;
                };
            }

            return;
        } else {
            // Capture the information and store it in the semantic analysis for this individual function
            self.info.function_info.insert(
                lambda_function.syntax_object_id,
                FunctionInformation::new(captured_vars, arguments)
                    .escapes(self.escape_analysis)
                    .depth(self.scope.depth()),
            );
        }

        self.stack_offset = stack_offset_rollback;
    }

    fn visit_set(&mut self, s: &'a crate::parser::ast::Set) {
        let name = s.variable.atom_identifier();
        // let id = s.variable.atom_syntax_object().map(|x| x.syntax_object_id);

        if let Some(info) = s
            .variable
            .atom_syntax_object()
            .and_then(|x| self.info.get(x))
        {
            if info.refers_to == self.defining_context {
                self.defining_context = None;
            }
        }

        self.visit_with_tail_call_eligibility(&s.expr, false);

        if let Some(name) = name {
            // Mark that we've used this
            self.vars_used.insert(name.to_string());

            // Gather the id of the variable that is in fact mutated
            if let Some(scope_info) = self.scope.get_mut(name) {
                // Bump the usage count
                // TODO Also mark this as mutated
                scope_info.usage_count += 1;

                let id = scope_info.id;
                if let Some(mut var) = self.info.get_mut(&id) {
                    var.set_bang = true;
                    scope_info.mutated = true;
                    // scope_info.read_heap_offset = var.read_heap_offset;

                    // while let Some(reference) = var.refers_to.and_then(|x| self.info.get_mut(&x)) {
                    //     reference.set_bang = true;
                    //     var = reference;
                    // }

                    // var.refers
                } else {
                    println!("Unable to find var: {name} in info map to update to set!");
                }
            } else {
                println!("Variable not yet in scope: {name}");
            }
        }

        self.visit(&s.variable);
    }

    fn visit_atom(&mut self, a: &'a crate::parser::ast::Atom) {
        let name = a.ident();
        let depth = self.scope.depth();
        // Snag the current id of this node - we're gonna want this for later
        let current_id = a.syn.syntax_object_id;

        // TODO: Check if this is actually a constant - if it is, mark it accordingly and lift it out

        if let Some(ident) = name {
            // Mark that we've seen this one
            self.vars_used.insert(ident.to_string());
            // Mark that this was perhaps used in tail position
            self.ids_referenced_in_tail_position.insert(current_id);

            // Check if its a global var - otherwise, we want to check if its a free
            // identifier
            if let Some(depth) = self.scope.height_of(ident) {
                if depth == 0 {
                    // Mark the parent as used
                    let global_var = self.scope.get_mut(ident).unwrap();
                    global_var.usage_count += 1;

                    self.info.get_mut(&global_var.id).unwrap().usage_count += 1;

                    let mut semantic_information =
                        SemanticInformation::new(IdentifierStatus::Global, depth, a.syn.span)
                            .with_usage_count(1)
                            .refers_to(global_var.id);

                    // TODO: We _really_ should be providing the built-ins in a better way thats not
                    // passing around a thread local
                    if crate::steel_vm::primitives::PRELUDE_MODULE.with(|x| x.contains(ident)) {
                        semantic_information.mark_builtin()
                    }

                    self.info.insert(&a.syn, semantic_information);

                    return;
                }
            }

            // If this contains a key at the top, then it shouldn't be marked as captured by this scope
            if self.scope.contains_key_at_top(ident) {
                // Set it to not be captured if its contained at the top level
                // self.scope.get_mut(ident).unwrap().captured = false;

                let mut_ref = self.scope.get_mut(ident).unwrap();

                // Not sure if this is going to be a problem...
                // Because if its captured at this stage, I think we want it to be marked as captured
                // TODO:
                // mut_ref.captured = false;
                mut_ref.usage_count += 1;

                // Mark this as last touched by this identifier
                mut_ref.last_used = Some(current_id);

                // In the event there is a local define, we want to count the usage here
                if let Some(local_define) = self.info.get_mut(&mut_ref.id) {
                    local_define.usage_count = mut_ref.usage_count;
                }

                let mut semantic_info =
                    SemanticInformation::new(IdentifierStatus::Local, depth, a.syn.span)
                        .with_usage_count(1)
                        .refers_to(mut_ref.id);

                if let Some(stack_offset) = mut_ref.stack_offset {
                    semantic_info = semantic_info.with_offset(stack_offset);
                } else {
                    log::warn!("Stack offset missing from local define")
                }

                if mut_ref.captured && mut_ref.mutated {
                    semantic_info.kind = IdentifierStatus::HeapAllocated;
                    semantic_info.heap_offset = mut_ref.heap_offset;
                    semantic_info.read_heap_offset = mut_ref.read_heap_offset;
                }

                self.info.insert(&a.syn, semantic_info);

                return;
            }

            // This is the result of some fancy fancy stuff
            // We are going to do two of these passes to resolve captures, where the first pass does a holistic
            // analysis of captures for each closure. Then we can do a top down analysis
            // which populates the closures with their capture positions, and then when we do analysis the
            // second time, we'll have closure references populated.
            if let Some(captured) = self.captures.get_mut(ident) {
                // TODO: Fill in this information here - when the vars are captured in the immediate enclosing,
                // we want to do stuff....

                captured.captured = true;
                captured.usage_count += 1;

                // TODO: Make sure we want to mark this identifier as last used
                captured.last_used = Some(current_id);

                let mut identifier_status = if captured.mutated {
                    IdentifierStatus::HeapAllocated
                } else if let Some(info) = self.info.get(&a.syn) {
                    if info.kind == IdentifierStatus::HeapAllocated {
                        IdentifierStatus::HeapAllocated
                    } else {
                        IdentifierStatus::Captured
                    }
                } else {
                    IdentifierStatus::Captured
                };

                // We also want to mark the current var thats actually in scope as last used as well
                if let Some(in_scope) = self.scope.get_mut(ident) {
                    in_scope.last_used = Some(current_id);
                    in_scope.captured = true;

                    // if identifier_status == IdentifierStatus::HeapAllocated {
                    //     in_scope.read_heap_offset = captured.read_heap_offset;
                    // }
                }

                if let Some(local_define) = self.info.get_mut(&captured.id) {
                    local_define.usage_count = captured.usage_count;

                    // If this _is_ in fact a locally defined function, we don't want to capture it
                    // This is something that is going to get lifted to the top environment anyway
                    if local_define.kind == IdentifierStatus::LocallyDefinedFunction {
                        captured.captured = false;
                        identifier_status = IdentifierStatus::LocallyDefinedFunction;
                    }
                }

                let mut semantic_info =
                    SemanticInformation::new(identifier_status, depth, a.syn.span)
                        .with_usage_count(1)
                        .refers_to(captured.id);

                // TODO: Merge the behavior of all of these separate cases into one
                semantic_info.with_captured_from_enclosing(captured.captured_from_enclosing);

                // If we're getting captured and mutated, then we should be fine to do these checks
                // exclusively
                if let Some(capture_offset) = captured.read_capture_offset {
                    semantic_info = semantic_info.with_read_capture_offset(capture_offset);
                    semantic_info =
                        semantic_info.with_capture_index(captured.capture_offset.unwrap());
                }

                if let Some(heap_offset) = captured.read_heap_offset {
                    // semantic_info = semantic_info.with_heap_offset(heap_offset);
                    semantic_info = semantic_info.with_read_heap_offset(heap_offset);
                } else {
                    log::warn!("Stack offset missing from local define")
                }

                if let Some(heap_offset) = captured.heap_offset {
                    // semantic_info = semantic_info.with_heap_offset(heap_offset);
                    semantic_info = semantic_info.with_heap_offset(heap_offset);
                } else {
                    log::warn!("Stack offset missing from local define")
                }

                // if semantic_info.kind == IdentifierStatus::HeapAllocated
                //     && semantic_info.read_heap_offset.is_none()
                // {
                //     panic!("Missing read heap offset here");
                // }

                // println!("Variable {} refers to {}", ident, is_captured.id);

                self.info.insert(&a.syn, semantic_info);

                return;
            }

            // Otherwise, go ahead and mark it as captured if we can find a reference to it
            // TODO: @Matt - There is an opportunity here to also check extra information. If the
            // variables exists in the local scope - i.e, the var has been patched in via a closure,
            // we could 1. do closure conversion on it via rewriting, or could 2. automatically
            // patch those vars in
            if let Some(is_captured) = self.scope.get_mut(ident) {
                is_captured.captured = true;
                is_captured.usage_count += 1;

                // TODO: Make sure we want to mark this identifier as last used
                is_captured.last_used = Some(current_id);

                let mut identifier_status = IdentifierStatus::Captured;

                if let Some(local_define) = self.info.get_mut(&is_captured.id) {
                    local_define.usage_count = is_captured.usage_count;

                    // If this _is_ in fact a locally defined function, we don't want to capture it
                    // This is something that is going to get lifted to the top environment anyway
                    if local_define.kind == IdentifierStatus::LocallyDefinedFunction {
                        // is_captured.captured = false;
                        identifier_status = IdentifierStatus::LocallyDefinedFunction;
                    }
                }

                let mut semantic_info =
                    SemanticInformation::new(identifier_status, depth, a.syn.span)
                        .with_usage_count(1)
                        .refers_to(is_captured.id);
                // .with_offset(
                //     is_captured
                //         .stack_offset
                //         .expect("Local variables should have an offset"),
                // );

                if let Some(stack_offset) = is_captured.stack_offset {
                    semantic_info = semantic_info.with_offset(stack_offset);
                } else {
                    log::warn!("Stack offset missing from local define")
                }

                // println!("Variable {} refers to {}", ident, is_captured.id);

                self.info.insert(&a.syn, semantic_info);

                return;
            }

            let mut semantic_info =
                SemanticInformation::new(IdentifierStatus::Free, depth, a.syn.span);

            // TODO: We _really_ should be providing the built-ins in a better way thats not
            // passing around a thread local
            if crate::steel_vm::primitives::PRELUDE_MODULE.with(|x| x.contains(ident)) {
                semantic_info.mark_builtin();
                semantic_info.kind = IdentifierStatus::Global
            }

            // Otherwise, we've hit a free variable at this point
            self.info.insert(&a.syn, semantic_info);

            log::warn!("Found free var: {}", a);
        }
    }
}

impl<'a> VisitorMutUnitRef<'a> for Analysis {
    fn visit_atom(&mut self, a: &'a crate::parser::ast::Atom) {
        log::info!(
            "Id: {:?}, Atom: {:?}, Semantic Information: {:?}",
            a.syn.syntax_object_id,
            a.syn.ty,
            self.get(&a.syn)
        );
    }

    fn visit_lambda_function(&mut self, lambda_function: &'a crate::parser::ast::LambdaFunction) {
        for arg in &lambda_function.args {
            if let Some(arg) = arg.atom_syntax_object() {
                log::info!(
                    "Id: {:?}, Atom in function argument: {:?}, Semantic Information: {:?}",
                    arg.syntax_object_id,
                    arg.ty,
                    self.get(arg)
                );
            }
        }

        self.visit(&lambda_function.body);
    }
}

pub fn query_top_level_define<A: AsRef<str>>(
    exprs: &[ExprKind],
    name: A,
) -> Option<&crate::parser::ast::Define> {
    let mut found_defines = Vec::new();
    for expr in exprs {
        if let ExprKind::Define(d) = expr {
            match d.name.atom_identifier() {
                Some(n) if name.as_ref() == n => found_defines.push(d.as_ref()),
                _ => {}
            }
        }
    }

    if found_defines.len() > 1 {
        log::info!(
            "Multiple defines found, unable to find one unique value to associate with a name"
        );
        return None;
    }

    if found_defines.len() == 1 {
        return found_defines.into_iter().next();
    }

    None
}

struct FindCallSiteById<'a, F> {
    id: SyntaxObjectId,
    analysis: &'a Analysis,
    func: F,
    modified: bool,
}

impl<'a, F> FindCallSiteById<'a, F> {
    pub fn _new(id: SyntaxObjectId, analysis: &'a Analysis, func: F) -> Self {
        Self {
            id,
            analysis,
            func,
            modified: false,
        }
    }

    // TODO: clean this up a bit
    pub fn is_required_call_site(&self, l: &List) -> bool {
        if let Some(refers_to) = l
            .args
            .first()
            .and_then(|x| x.atom_syntax_object())
            .and_then(|x| self.analysis.get(x))
            .and_then(|x| x.refers_to)
        {
            refers_to == self.id
        } else {
            false
        }
    }
}

impl<'a, F> VisitorMutRefUnit for FindCallSiteById<'a, F>
where
    F: FnMut(&Analysis, &mut crate::parser::ast::List) -> bool,
{
    fn visit_list(&mut self, l: &mut List) {
        // Go downward and visit each of the arguments (including the function call)
        for arg in &mut l.args {
            self.visit(arg);
        }

        // If we're a match, call the function
        if self.is_required_call_site(l) {
            self.modified |= (self.func)(self.analysis, l)
        }
    }
}

struct FindUsages<'a, F> {
    id: SyntaxObjectId,
    analysis: &'a Analysis,
    func: F,
    modified: bool,
}

impl<'a, F> FindUsages<'a, F> {
    pub fn new(id: SyntaxObjectId, analysis: &'a Analysis, func: F) -> Self {
        Self {
            id,
            analysis,
            func,
            modified: false,
        }
    }
}

impl<'a, F> VisitorMutRefUnit for FindUsages<'a, F>
where
    F: FnMut(&Analysis, &mut crate::parser::ast::Atom) -> bool,
{
    fn visit_atom(&mut self, a: &mut Atom) {
        if let Some(refers_to) = self.analysis.get(&a.syn).and_then(|x| x.refers_to) {
            if refers_to == self.id {
                self.modified |= (self.func)(self.analysis, a)
            }
        }
    }
}

struct FindCallSites<'a, F> {
    name: &'a str,
    analysis: &'a Analysis,
    func: F,
}

impl<'a, F> FindCallSites<'a, F> {
    pub fn new(name: &'a str, analysis: &'a Analysis, func: F) -> Self {
        Self {
            name,
            analysis,
            func,
        }
    }
}
impl<'a, F> FindCallSites<'a, F> {
    fn is_required_global_function_call(&self, l: &List) -> bool {
        if let Some(name) = l.first_ident() {
            if let Some(semantic_info) = self.analysis.get(l.args[0].atom_syntax_object().unwrap())
            {
                return name == self.name && semantic_info.kind == IdentifierStatus::Global;
            }
        }

        false
    }
}

impl<'a, F> VisitorMutUnitRef<'a> for FindCallSites<'a, F>
where
    F: FnMut(&Analysis, &crate::parser::ast::List),
{
    fn visit_list(&mut self, l: &'a crate::parser::ast::List) {
        if self.is_required_global_function_call(l) {
            (self.func)(self.analysis, l)
        }

        for arg in &l.args {
            self.visit(arg);
        }
    }
}

impl<'a, F> VisitorMutRefUnit for FindCallSites<'a, F>
where
    F: FnMut(&Analysis, &mut crate::parser::ast::List),
{
    fn visit_list(&mut self, l: &mut crate::parser::ast::List) {
        if self.is_required_global_function_call(l) {
            (self.func)(self.analysis, l)
        }

        for arg in &mut l.args {
            self.visit(arg);
        }
    }
}

struct RefreshVars;

impl VisitorMutRefUnit for RefreshVars {
    fn visit_atom(&mut self, a: &mut Atom) {
        a.syn.syntax_object_id = SyntaxObjectId::fresh();
    }
}

struct MutateCallSites<'a, F> {
    name: &'a str,
    analysis: &'a Analysis,
    func: F,
}

impl<'a, F> MutateCallSites<'a, F> {
    pub fn new(name: &'a str, analysis: &'a Analysis, func: F) -> Self {
        Self {
            name,
            analysis,
            func,
        }
    }
}

impl<'a, F> VisitorMutRefUnit for MutateCallSites<'a, F>
where
    F: FnMut(&Analysis, &mut ExprKind),
{
    fn visit(&mut self, expr: &mut ExprKind) {
        match expr {
            ExprKind::If(f) => self.visit_if(f),
            ExprKind::Define(d) => self.visit_define(d),
            ExprKind::LambdaFunction(l) => self.visit_lambda_function(l),
            ExprKind::Begin(b) => self.visit_begin(b),
            ExprKind::Return(r) => self.visit_return(r),
            ExprKind::Quote(q) => self.visit_quote(q),
            ExprKind::Macro(m) => self.visit_macro(m),
            ExprKind::Atom(a) => self.visit_atom(a),
            list @ ExprKind::List(_) => {
                if let ExprKind::List(l) = &list {
                    if let Some(name) = l.first_ident() {
                        if let Some(semantic_info) =
                            self.analysis.get(l.args[0].atom_syntax_object().unwrap())
                        {
                            if name == self.name && semantic_info.kind == IdentifierStatus::Global {
                                // At this point, call out to the user given function - if we do in fact mutate
                                // where the value points to, we should return a full node that needs to be visited
                                (self.func)(self.analysis, list);

                                // TODO: Analysis should maybe be re run here - mutations might invalidate the analysis
                                // This might make it worth rerunning the analysis

                                return self.visit(list);
                            }
                        }
                    }
                }

                if let ExprKind::List(l) = list {
                    return self.visit_list(l);
                }

                unreachable!()
            }
            ExprKind::SyntaxRules(s) => self.visit_syntax_rules(s),
            ExprKind::Set(s) => self.visit_set(s),
            ExprKind::Require(r) => self.visit_require(r),
            ExprKind::Let(l) => self.visit_let(l),
        }
    }
}

struct LetCallSites<'a, F> {
    analysis: &'a Analysis,
    func: F,
}

impl<'a, F> LetCallSites<'a, F> {
    pub fn new(analysis: &'a Analysis, func: F) -> Self {
        Self { analysis, func }
    }
}

impl<'a, F> VisitorMutRefUnit for LetCallSites<'a, F>
where
    F: FnMut(&Analysis, &mut ExprKind) -> bool,
{
    fn visit(&mut self, expr: &mut ExprKind) {
        match expr {
            ExprKind::If(f) => self.visit_if(f),
            ExprKind::Define(d) => self.visit_define(d),
            ExprKind::LambdaFunction(l) => self.visit_lambda_function(l),
            ExprKind::Begin(b) => self.visit_begin(b),
            ExprKind::Return(r) => self.visit_return(r),
            ExprKind::Quote(q) => self.visit_quote(q),
            ExprKind::Macro(m) => self.visit_macro(m),
            ExprKind::Atom(a) => self.visit_atom(a),
            ExprKind::List(l) => self.visit_list(l),
            ExprKind::SyntaxRules(s) => self.visit_syntax_rules(s),
            ExprKind::Set(s) => self.visit_set(s),
            ExprKind::Require(r) => self.visit_require(r),
            let_expr @ ExprKind::Let(_) => {
                if let ExprKind::Let(l) = let_expr {
                    self.visit_let(l);
                }

                if let ExprKind::Let(_) = &let_expr {
                    if (self.func)(self.analysis, let_expr) {
                        log::info!("Modified let expression");
                    }
                }
            }
        }
    }
}

// TODO: This will need to get changed in the event we actually modify _what_ the mutable pointer points to
// Right now, if we want to modify a call site, we can only change it to a call site - what we _should_ do is have it be able to point
// back to any arbitrary value, and subsequently change l to point to that value by doing another level of the recursion
// Something like:
// where F: FnMut(&mut ExprKind) -> ()
// To do this, the main visit loop would need to be goofed with in the visitor, and we pass in the reference to the wrapped object, rather than the underlying one
struct AnonymousFunctionCallSites<'a, F> {
    analysis: &'a Analysis,
    func: F,
}

impl<'a, F> AnonymousFunctionCallSites<'a, F> {
    pub fn new(analysis: &'a Analysis, func: F) -> Self {
        Self { analysis, func }
    }
}

impl<'a, F> VisitorMutRefUnit for AnonymousFunctionCallSites<'a, F>
where
    F: FnMut(&Analysis, &mut ExprKind) -> bool,
{
    fn visit(&mut self, expr: &mut ExprKind) {
        match expr {
            ExprKind::If(f) => self.visit_if(f),
            ExprKind::Define(d) => self.visit_define(d),
            ExprKind::LambdaFunction(l) => self.visit_lambda_function(l),
            ExprKind::Begin(b) => self.visit_begin(b),
            ExprKind::Return(r) => self.visit_return(r),
            ExprKind::Quote(q) => self.visit_quote(q),
            ExprKind::Macro(m) => self.visit_macro(m),
            ExprKind::Atom(a) => self.visit_atom(a),
            list @ ExprKind::List(_) => {
                // Bottom up approach - visit everything first, then, on the way back up,
                // modify the value
                if let ExprKind::List(l) = list {
                    self.visit_list(l);
                }

                if let ExprKind::List(l) = &list {
                    if l.is_anonymous_function_call() {
                        // TODO: rerunning analysis might be worth it here - we want to be able to trigger a re run if a mutation would cause a change
                        // In the state of the analysis
                        if (self.func)(self.analysis, list) {
                            // return self.visit(list);
                            log::info!("Modified anonymous function call site!");
                        }
                    }
                }
            }
            ExprKind::SyntaxRules(s) => self.visit_syntax_rules(s),
            ExprKind::Set(s) => self.visit_set(s),
            ExprKind::Require(r) => self.visit_require(r),
            ExprKind::Let(l) => self.visit_let(l),
        }
    }
}

struct RemoveUnusedDefineImports<'a> {
    analysis: &'a Analysis,
    depth: usize,
}

impl<'a> RemoveUnusedDefineImports<'a> {
    pub fn new(analysis: &'a Analysis) -> Self {
        Self { analysis, depth: 0 }
    }
}

impl<'a> VisitorMutRefUnit for RemoveUnusedDefineImports<'a> {
    fn visit_lambda_function(&mut self, lambda_function: &mut LambdaFunction) {
        self.depth += 1;
        self.visit(&mut lambda_function.body);
        self.depth -= 1;
    }

    fn visit_begin(&mut self, begin: &mut crate::parser::ast::Begin) {
        // Only remove internal definitions here
        if self.depth > 0 {
            let mut exprs_to_drop = Vec::new();

            for (idx, expr) in begin.exprs.iter().enumerate() {
                if let ExprKind::Define(d) = expr {
                    if d.is_a_builtin_definition() {
                        if let Some(analysis) =
                            self.analysis.get(d.name.atom_syntax_object().unwrap())
                        {
                            if analysis.usage_count == 0 {
                                exprs_to_drop.push(idx);
                            }
                        }
                    }
                }
            }

            for idx in exprs_to_drop.iter().rev() {
                // println!("Removing: {:?}", begin.exprs.get(*idx));
                begin.exprs.remove(*idx);
            }

            if begin.exprs.is_empty() {
                begin.exprs.push(ExprKind::Quote(Box::new(Quote::new(
                    ExprKind::atom("void".to_string()),
                    RawSyntaxObject::default(TokenType::Quote),
                ))));
            }

            // println!("Resulting expr: {:?}", begin);
        }

        for expr in &mut begin.exprs {
            self.visit(expr);
        }
    }
}

struct RemovedUnusedImports<'a> {
    analysis: &'a Analysis,
}

impl<'a> RemovedUnusedImports<'a> {
    pub fn new(analysis: &'a Analysis) -> Self {
        Self { analysis }
    }
}

impl<'a> VisitorMutRefUnit for RemovedUnusedImports<'a> {
    fn visit_list(&mut self, l: &mut List) {
        let mut unused_arguments = Vec::new();

        if l.is_anonymous_function_call() {
            let argument_count = l.args.len() - 1;
            if let Some(func) = l.first_func() {
                if argument_count != func.args.len() {
                    println!("-- Static arity mismatch -- Should actually error here");
                } else {
                    unused_arguments = func
                        .args
                        .iter()
                        .enumerate()
                        .filter(|x| {
                            self.analysis
                                .get(x.1.atom_syntax_object().unwrap())
                                .unwrap()
                                .usage_count
                                == 0
                        })
                        .map(|x| x.0)
                        .filter(|x| match l.args.get(*x) {
                            Some(ExprKind::List(l)) => l.is_a_builtin_expr(),
                            Some(ExprKind::Quote(_)) => true,
                            Some(ExprKind::Atom(a)) => matches!(
                                a.syn.ty,
                                TokenType::NumberLiteral(_)
                                    | TokenType::IntegerLiteral(_)
                                    | TokenType::BooleanLiteral(_)
                            ),
                            _ => false,
                        })
                        .collect();
                }
            }
        }

        if let Some(func) = l.first_func_mut() {
            for index in unused_arguments.iter().rev() {
                func.args.remove(*index);
            }
        }

        for index in unused_arguments.iter().rev() {
            l.args.remove(index + 1);
        }

        // println!("Resulting expression: {}", l);

        for arg in &mut l.args {
            self.visit(arg);
        }
    }
}

struct UnusedArguments<'a> {
    analysis: &'a Analysis,
    unused_args: Vec<Span>,
}

impl<'a> UnusedArguments<'a> {
    pub fn new(analysis: &'a Analysis) -> Self {
        Self {
            analysis,
            unused_args: Vec::new(),
        }
    }
}

impl<'a> VisitorMutUnitRef<'a> for UnusedArguments<'a> {
    fn visit_lambda_function(&mut self, lambda_function: &'a LambdaFunction) {
        for arg in &lambda_function.args {
            if let Some(syntax_object) = arg.atom_syntax_object() {
                if let Some(info) = self.analysis.get(syntax_object) {
                    // println!("Ident: {}, Info: {:?}", arg, info);
                    if info.usage_count == 0 {
                        self.unused_args.push(syntax_object.span);
                    }
                }
            }
        }

        self.visit(&lambda_function.body);
    }
}

struct LiftPureFunctionsToGlobalScope<'a> {
    analysis: &'a Analysis,
    lifted_functions: Vec<ExprKind>,
}

impl<'a> LiftPureFunctionsToGlobalScope<'a> {
    pub fn new(analysis: &'a Analysis) -> Self {
        Self {
            analysis,
            lifted_functions: Vec::new(),
        }
    }
}

fn atom(name: String) -> ExprKind {
    ExprKind::Atom(Atom::new(SyntaxObject::default(TokenType::Identifier(
        name,
    ))))
}

impl<'a> VisitorMutRefUnit for LiftPureFunctionsToGlobalScope<'a> {
    fn visit(&mut self, expr: &mut ExprKind) {
        match expr {
            ExprKind::If(f) => self.visit_if(f),
            ExprKind::Define(d) => self.visit_define(d),
            lambda @ ExprKind::LambdaFunction(_) => {
                // Do this bottom up - visit the children and on the way up we're going to
                // apply the transformation
                if let ExprKind::LambdaFunction(l) = lambda {
                    for var in &mut l.args {
                        self.visit(var);
                    }
                    self.visit(&mut l.body);
                }

                if let ExprKind::LambdaFunction(l) = lambda {
                    if let Some(info) = self.analysis.get_function_info(l) {
                        // println!("depth: {}", info.depth);

                        // We don't need to lift top level functions to the top level already
                        if info.depth == 1 {
                            return;
                        }

                        // If we have no captured variables, this is a pure function for our purposes
                        if info.captured_vars.is_empty() {
                            // Name the closure something mangled, but something we can refer to later
                            let constructed_name = "##__lifted_pure_function".to_string()
                                + l.syntax_object_id.to_string().as_ref();

                            // Point the reference to a dummy list - this is just an ephemeral placeholder
                            let mut dummy = ExprKind::List(List::new(Vec::new()));

                            // Have the lambda now actually point to this dummy list
                            std::mem::swap(lambda, &mut dummy);

                            // Construct the global definition, this is something like
                            //
                            // (define ##__lifted_pure_function42 (lambda (x) 10))
                            //
                            // This is going to go into our vec of lifted functions
                            let constructed_definition = ExprKind::Define(Box::new(Define::new(
                                atom(constructed_name.clone()),
                                dummy,
                                SyntaxObject::default(TokenType::Define),
                            )));

                            self.lifted_functions.push(constructed_definition);

                            // Now update the reference to just point straight to a variable reference
                            // This _should_ be globally unique
                            *lambda = atom(constructed_name);
                        }
                    }
                }

                // if let ExprKind::LambdaFunction(l) = lambda
            }
            ExprKind::Begin(b) => self.visit_begin(b),
            ExprKind::Return(r) => self.visit_return(r),
            ExprKind::Quote(q) => self.visit_quote(q),
            ExprKind::Macro(m) => self.visit_macro(m),
            ExprKind::Atom(a) => self.visit_atom(a),
            ExprKind::List(l) => self.visit_list(l),
            ExprKind::SyntaxRules(s) => self.visit_syntax_rules(s),
            ExprKind::Set(s) => self.visit_set(s),
            ExprKind::Require(r) => self.visit_require(r),
            ExprKind::Let(l) => self.visit_let(l),
        }
    }
}

struct LiftLocallyDefinedFunctions<'a> {
    analysis: &'a Analysis,
    lifted_functions: Vec<ExprKind>,
}

impl<'a> LiftLocallyDefinedFunctions<'a> {
    pub fn new(analysis: &'a Analysis) -> Self {
        Self {
            analysis,
            lifted_functions: Vec::new(),
        }
    }
}

impl<'a> VisitorMutRefUnit for LiftLocallyDefinedFunctions<'a> {
    fn visit_begin(&mut self, begin: &mut crate::parser::ast::Begin) {
        // Traverse down the tree first - start bubbling up the lifted functions
        // on the way back up
        for expr in &mut begin.exprs {
            self.visit(expr);
        }

        let mut functions: Vec<(usize, String, SyntaxObjectId)> = Vec::new();

        for (index, expr) in begin.exprs.iter().enumerate() {
            if let ExprKind::Define(define) = expr {
                let ident = define.name.atom_syntax_object().unwrap();
                if let Some(info) = define
                    .body
                    .lambda_function()
                    .and_then(|func| self.analysis.get_function_info(func))
                {
                    let ident_info = self.analysis.get(ident).unwrap();

                    if ident_info.depth > 1 {
                        if !info.captured_vars.is_empty() {
                            log::info!(
                            target: "lambda-lifting",
                                "Found a local function which captures variables: {} - captures vars: {:#?}",
                                define.name,
                                info.captured_vars
                            );
                        } else {
                            log::info!(target: "lambda-lifting", "Found a pure local function: {}", define.name);
                            functions.push((
                                index,
                                define.name.atom_identifier().unwrap().to_string(),
                                define.name_id().unwrap(),
                            ));
                        }
                    }
                }
            }
        }

        for (index, _name, _id) in functions.into_iter().rev() {
            // let constructed_name =
            //     "##lambda-lifting##".to_string() + &name + id.0.to_string().as_str();

            // let mut dummy_define = ExprKind::Define(Box::new(Define::new(
            //     ExprKind::atom(name),
            //     ExprKind::atom(constructed_name),
            //     SyntaxObject::default(TokenType::Define),
            // )));

            // let mut removed_function = begin.exprs.get_mut(index).unwrap();

            // std::mem::swap(&mut dummy_define, &mut removed_function);

            let removed_function = begin.exprs.remove(index);
            self.lifted_functions.push(removed_function);
        }
    }
}

struct ExprContainsIds<'a> {
    analysis: &'a Analysis,
    ids: &'a HashSet<SyntaxObjectId>,
}

impl<'a> ExprContainsIds<'a> {
    pub fn contains(
        analysis: &'a Analysis,
        ids: &'a HashSet<SyntaxObjectId>,
        expr: &ExprKind,
    ) -> bool {
        matches!(
            ExprContainsIds { analysis, ids }.visit(expr),
            std::ops::ControlFlow::Break(_)
        )
    }
}

impl<'a> VisitorMutControlFlow for ExprContainsIds<'a> {
    fn visit_atom(&mut self, a: &Atom) -> std::ops::ControlFlow<()> {
        if let Some(refers_to) = self.analysis.get(&a.syn).and_then(|x| x.refers_to) {
            if self.ids.contains(&refers_to) {
                return std::ops::ControlFlow::Break(());
            }
        }

        std::ops::ControlFlow::Continue(())
    }
}

struct FlattenAnonymousFunctionCalls<'a> {
    analysis: &'a Analysis,
}

impl<'a> FlattenAnonymousFunctionCalls<'a> {
    pub fn flatten(analysis: &'a Analysis, exprs: &mut Vec<ExprKind>) {
        for expr in exprs {
            Self { analysis }.visit(expr);
        }
    }
}

impl<'a> VisitorMutRefUnit for FlattenAnonymousFunctionCalls<'a> {
    fn visit_list(&mut self, l: &mut List) {
        // let mut replacement_body: Option<ExprKind> = None;
        let mut args = Vec::new();

        let mut inner_body = None;
        let mut changed = false;

        // This is an anonymous function call
        if let Some(function_a) = l.first_func_mut() {
            // The body is also just an anonymous function call
            if let ExprKind::List(inner_l) = &mut function_a.body {
                let arg_ids = function_a
                    .args
                    .iter()
                    .map(|x| x.atom_syntax_object().unwrap().syntax_object_id)
                    .collect::<HashSet<_>>();

                let all_dont_contain_references = inner_l.args[1..]
                    .iter()
                    .all(|x| !ExprContainsIds::contains(self.analysis, &arg_ids, x));

                if all_dont_contain_references {
                    if let Some(function_b) = inner_l.first_func_mut() {
                        // Then we should be able to flatten the function into one

                        // TODO: Check that any of the vars we're using are used in the body expression
                        // They can only be used in the argument position

                        let mut dummy = ExprKind::empty();

                        // Extract the inner body
                        std::mem::swap(&mut function_b.body, &mut dummy);

                        inner_body = Some(dummy);

                        // TODO: This doesn't work quite yet -
                        function_a.args.append(&mut function_b.args);
                        args.extend(inner_l.args.drain(1..));

                        changed = true;
                    }
                }
            }

            if let Some(new_body) = inner_body {
                function_a.body = new_body;
            }
        }

        l.args.append(&mut args);

        if changed {
            self.visit_list(l);
        } else {
            // Visit the children after
            for expr in &mut l.args {
                self.visit(expr)
            }
        }
    }
}

// TODO: There might be opportunity to parallelize this here - perhaps shard the analysis between threads
// across some subset of expressions and then merge afterwards
pub struct SemanticAnalysis<'a> {
    // We want to reserve the right to add or remove expressions from the program as needed
    exprs: &'a mut Vec<ExprKind>,
    pub(crate) analysis: Analysis,
}

impl<'a> SemanticAnalysis<'a> {
    pub fn from_analysis(exprs: &'a mut Vec<ExprKind>, analysis: Analysis) -> Self {
        Self { exprs, analysis }
    }

    pub fn new(exprs: &'a mut Vec<ExprKind>) -> Self {
        let analysis = Analysis::from_exprs(exprs);
        Self { exprs, analysis }
    }

    pub fn populate_captures(&mut self) {
        self.analysis.populate_captures(self.exprs);
    }

    pub fn get(&self, object: &SyntaxObject) -> Option<&SemanticInformation> {
        self.analysis.get(object)
    }

    pub fn query_top_level_define<A: AsRef<str>>(
        &self,
        name: A,
    ) -> Option<&crate::parser::ast::Define> {
        query_top_level_define(self.exprs, name)
    }

    // Takes the function call, and inlines it at the call sites. In theory, with constant evaluation and
    // dead code elimination, this should help streamline some of the more complex cases. This is also just a start.
    pub fn inline_function_call<A: AsRef<str>>(&mut self, name: A) -> Result<(), SteelErr> {
        // find_call_sites_and_mutate_with

        // TODO: Cloning here is expensive. We should strive to make these trees somehow share the nodes a bit more elegantly.
        // As it stands, each time we close a syntax tree, we're going to do a deep clone of the whole thing, which we really don't
        // want to do.
        let top_level_define_body = self.query_top_level_define(name.as_ref()).ok_or_else(
            throw!(TypeMismatch => format!("Cannot inline free identifier!: {}", name.as_ref())),
        )?.body.lambda_function().ok_or_else(throw!(TypeMismatch => format!("Cannot inline non function for: {}", name.as_ref())))?.clone();

        self.find_call_sites_and_modify_with(
            name.as_ref(),
            |_: &Analysis, lst: &mut crate::parser::ast::List| {
                lst.args[0] = ExprKind::LambdaFunction(Box::new(top_level_define_body.clone()));
            },
        );

        Ok(())
    }

    pub fn get_global_id<A: AsRef<str>>(&self, name: A) -> Option<SyntaxObjectId> {
        self.query_top_level_define(name)?
            .name
            .atom_syntax_object()
            .map(|x| x.syntax_object_id)
    }

    pub fn find_let_call_sites_and_mutate_with<F>(&mut self, func: F)
    where
        F: FnMut(&Analysis, &mut ExprKind) -> bool,
    {
        let mut let_call_sites = LetCallSites::new(&self.analysis, func);
        for expr in self.exprs.iter_mut() {
            let_call_sites.visit(expr);
        }
    }

    /// In this case, `let` also translates directly to an anonymous function call
    pub fn find_anonymous_function_calls_and_mutate_with<F>(&mut self, func: F)
    where
        F: FnMut(&Analysis, &mut ExprKind) -> bool,
    {
        let mut anonymous_function_call_sites =
            AnonymousFunctionCallSites::new(&self.analysis, func);
        for expr in self.exprs.iter_mut() {
            anonymous_function_call_sites.visit(expr);
        }
    }

    /// Find all local pure functions, except those defined already at the top level and those defined with 'define',
    /// and replace them with a globally defined function. This means we're not going to be recreating
    /// the function _on every instance_ and instead can just grab them each time.
    /// TODO: @Matt - figure out a way to simplify the construction of closures as well - perhaps introduce
    /// some sort of combinator for closures as well.
    pub fn lift_all_local_functions(&mut self) -> &mut Self {
        let mut lifter = LiftPureFunctionsToGlobalScope::new(&self.analysis);

        for expr in self.exprs.iter_mut() {
            lifter.visit(expr);
        }

        if !lifter.lifted_functions.is_empty() {
            // This is a silly little way to put the lifted functions actually at the top of
            // the expression list
            lifter.lifted_functions.append(self.exprs);

            *self.exprs = lifter.lifted_functions;

            log::info!("Re-running the analysis after lifting local functions");
            self.analysis = Analysis::from_exprs(self.exprs);
            self.analysis.populate_captures(self.exprs);
        }

        self
    }

    /// Find lets without arguments and replace these with just the body of the function.
    /// For instance:
    /// ```scheme
    /// (let () (+ 1 2 3 4 5)) ;; => (+ 1 2 3 4 5)
    /// ```
    ///
    /// The other function - `replace_pure_empty_lets_with_body` explicitly refers to anonymous functions
    /// that lets are naively translated to, so something like this:
    ///
    /// ```scheme
    /// ((lambda () (+ 1 2 3 4 5)))
    /// ```
    ///
    pub fn replace_no_argument_lets_with_body(&mut self) -> &mut Self {
        let mut re_run_analysis = false;

        let func = |_: &Analysis, anon: &mut ExprKind| {
            if let ExprKind::Let(l) = anon {
                if l.bindings.is_empty() {
                    let mut dummy = ExprKind::List(List::new(Vec::new()));
                    std::mem::swap(&mut l.body_expr, &mut dummy);
                    *anon = dummy;

                    re_run_analysis = true;

                    return true;
                }
            } else {
                unreachable!()
            }

            false
        };

        self.find_let_call_sites_and_mutate_with(func);

        if re_run_analysis {
            log::info!("Re-running the semantic analysis after modifying let call sites");

            self.analysis = Analysis::from_exprs(self.exprs);
        }

        self
    }

    /// Find anonymous function calls with no arguments that don't capture anything,
    /// and replace this with just the body of the function. For instance:
    ///
    /// ```scheme
    /// (let () (+ 1 2 3 4 5)) ;; => (+ 1 2 3 4 5)
    ///
    /// ```
    pub fn replace_pure_empty_lets_with_body(&mut self) -> &mut Self {
        let mut re_run_analysis = false;

        let func = |analysis: &Analysis, anon: &mut ExprKind| {
            if let ExprKind::List(l) = anon {
                let arg_count = l.args.len() - 1;
                let function = l.args.get_mut(0).unwrap();

                if let ExprKind::LambdaFunction(f) = function {
                    let analysis = analysis.get_function_info(f).unwrap();

                    if analysis.captured_vars.is_empty() {
                        log::info!("Found a function that does not capture variables");

                        if f.args.is_empty() && arg_count == 0 {
                            // Take out the body of the function - we're going to want to use that now
                            let mut dummy = ExprKind::List(List::new(Vec::new()));
                            std::mem::swap(&mut f.body, &mut dummy);
                            *anon = dummy;

                            re_run_analysis = true;

                            // We changed the function call - we should adjust accordingly
                            return true;
                        }
                    }
                } else {
                    unreachable!()
                }
            } else {
                unreachable!()
            }

            false
        };

        self.find_anonymous_function_calls_and_mutate_with(func);

        if re_run_analysis {
            log::info!("Re-running the semantic analysis after modifications");

            self.analysis = Analysis::from_exprs(self.exprs);
        }

        self
    }

    pub fn replace_anonymous_function_calls_with_plain_lets(&mut self) -> &mut Self {
        let mut re_run_analysis = false;

        let func = |_: &Analysis, anon: &mut ExprKind| {
            if let ExprKind::List(l) = anon {
                let function = l.args.remove(0);

                if let ExprKind::LambdaFunction(mut f) = function {
                    let mut function_body = ExprKind::List(List::new(Vec::new()));
                    std::mem::swap(&mut f.body, &mut function_body);

                    let let_expr = Let::new(
                        f.args
                            .iter()
                            .zip(l.args.iter())
                            .map(|x| (x.0.clone(), x.1.clone()))
                            .collect(),
                        function_body,
                        f.location.clone(),
                    );

                    *anon = ExprKind::Let(let_expr.into());

                    re_run_analysis = true;
                    log::info!("Replaced anonymous function call with let");

                    true
                } else {
                    unreachable!()
                }
            } else {
                unreachable!()
            }

            // false
        };

        self.find_anonymous_function_calls_and_mutate_with(func);

        if re_run_analysis {
            log::info!("Re-running the semantic analysis after modifications");

            self.analysis = Analysis::from_exprs(self.exprs);
        }

        self
    }

    pub fn refresh_variables(&mut self) -> &mut Self {
        for expr in self.exprs.iter_mut() {
            RefreshVars.visit(expr);
        }

        self.analysis = Analysis::from_exprs(self.exprs);

        self
    }

    // Modify the call site to point to another kind of expression
    pub fn find_call_sites_and_mutate_with<F>(&mut self, name: &str, func: F)
    where
        F: FnMut(&Analysis, &mut ExprKind),
    {
        let mut find_call_sites = MutateCallSites::new(name, &self.analysis, func);

        for expr in self.exprs.iter_mut() {
            find_call_sites.visit(expr);
        }
    }

    // Locate the call sites of the given global function name, and calls the given function
    // on the node
    pub fn find_call_sites_and_call<F>(&self, name: &str, func: F)
    where
        F: FnMut(&Analysis, &crate::parser::ast::List),
    {
        let mut find_call_sites = FindCallSites::new(name, &self.analysis, func);

        for expr in self.exprs.iter() {
            find_call_sites.visit(expr);
        }
    }

    // Locate the call sites of the given global function, and calls the given function
    // on the node
    pub fn find_call_sites_and_modify_with<F>(&mut self, name: &str, func: F)
    where
        F: FnMut(&Analysis, &mut crate::parser::ast::List),
    {
        let mut find_call_sites = FindCallSites::new(name, &self.analysis, func);

        for expr in self.exprs.iter_mut() {
            find_call_sites.visit(expr);
        }
    }

    pub fn last_usages(&self) -> impl Iterator<Item = &'_ SemanticInformation> {
        self.analysis.info.values().filter(|x| x.last_usage)
    }

    pub fn free_identifiers(&self) -> impl Iterator<Item = &'_ SemanticInformation> {
        self.analysis
            .info
            .values()
            .filter(|x| x.kind == IdentifierStatus::Free)
    }

    pub fn unused_variables(&self) -> impl Iterator<Item = &'_ SemanticInformation> {
        self.analysis.info.values().filter(|x| {
            x.usage_count == 0
                && matches!(x.kind, IdentifierStatus::Local | IdentifierStatus::Global)
        })
    }

    pub fn global_defs(&self) -> impl Iterator<Item = &'_ SemanticInformation> {
        self.analysis
            .info
            .values()
            .filter(|x| x.kind == IdentifierStatus::Global)
    }

    pub fn built_ins(&self) -> impl Iterator<Item = &'_ SemanticInformation> {
        self.analysis.info.values().filter(|x| x.builtin)
    }

    pub fn find_free_identifiers(&self) -> impl Iterator<Item = &'_ SemanticInformation> {
        self.analysis
            .info
            .values()
            .filter(|x| x.kind == IdentifierStatus::Free)
    }

    pub fn find_unused_arguments(&self) -> Vec<Span> {
        let mut unused = UnusedArguments::new(&self.analysis);

        for expr in self.exprs.iter() {
            unused.visit(expr);
        }

        unused.unused_args
    }

    // TODO: Right now this lifts and renames, but it does not handle
    // The extra arguments necessary for this to work
    pub fn lift_pure_local_functions(&mut self) -> &mut Self {
        let mut overall_lifted = Vec::new();
        let mut re_run_analysis = false;

        let exprs_len = self.exprs.len();

        // Window in to the individual values - we're going to want to get access to each of the other ones individually
        for i in 0..exprs_len {
            let mut local_funcs = LiftLocallyDefinedFunctions::new(&self.analysis);

            // Just borrow for exactly how long we need it for
            local_funcs.visit(self.exprs.get_mut(i).unwrap());

            if !local_funcs.lifted_functions.is_empty() {
                re_run_analysis = true;
            }

            // Move out the local functions
            let mut local_functions = local_funcs.lifted_functions;

            let ids = local_functions
                .iter()
                .map(|x| {
                    if let ExprKind::Define(d) = x {
                        log::info!("Found a local function to lift: {}", d.name);
                        d.name.atom_syntax_object().unwrap().syntax_object_id
                    } else {
                        unreachable!()
                    }
                })
                .collect::<Vec<_>>();

            for id in ids {
                let mut find_call_site_by_id =
                    FindUsages::new(id, &self.analysis, |_: &Analysis, usage: &mut Atom| {
                        if let Some(ident) = usage.ident_mut() {
                            *ident = "##lambda-lifting##".to_string()
                                + ident
                                + id.0.to_string().as_str();
                            true
                        } else {
                            false
                        }
                    });

                // Mutate the call sites of the original expression we were visiting
                // Similar to here - just borrow exactly how long we need it for
                find_call_site_by_id.visit(self.exprs.get_mut(i).unwrap());
                // Now also mutate the bodies of any of the functions as well
                for local_function in &mut local_functions {
                    find_call_site_by_id.visit(local_function);
                    // println!("Function: {}", local_function);

                    // Also update the name
                    if let ExprKind::Define(define) = local_function {
                        if let Some(syntax_object) = define.name.atom_syntax_object() {
                            if syntax_object.syntax_object_id != id {
                                continue;
                            }
                        }

                        if let Some(name) = define.name.atom_identifier_mut() {
                            *name =
                                "##lambda-lifting##".to_string() + name + id.0.to_string().as_str();
                        } else {
                            unreachable!("This should explicitly be an identifier here - perhaps a macro was invalid?");
                        }
                    } else {
                        unreachable!("These should only be defines by design");
                    }
                }

                // Visit the other ones
                for j in 0..exprs_len {
                    if i != j {
                        // println!("Revisiting: {}", self.exprs.get_mut(j).unwrap());
                        find_call_site_by_id.visit(self.exprs.get_mut(j).unwrap());
                    }
                }

                // Also visit those that have been lifted out
                // for expr in &mut overall_lifted {
                //     println!("Visiting: {}", expr);
                //     find_call_site_by_id.visit(expr);
                // }

                // Check if we need to re run this analysis - if we've made any modifications up to this point, we
                // Need to re run the analysis afterwards
                re_run_analysis |= find_call_site_by_id.modified;
            }

            // Put the lifted expressions back at the end - they _probably_ should go to the front, but for now
            // Lets just put them at the back
            overall_lifted.append(&mut local_functions);
        }

        // Same as the above - just getting around a double mutable borrow.
        // Move the lifted functions to the back of the original expression list
        // self.exprs.append(&mut overall_lifted);

        overall_lifted.append(self.exprs);
        *self.exprs = overall_lifted;

        if re_run_analysis {
            log::info!(
                "Re-running the semantic analysis after modifications during lambda lifting"
            );

            self.analysis = Analysis::from_exprs(self.exprs);
            self.analysis.populate_captures(self.exprs);
        }

        self
    }

    pub fn resolve_alias(&self, id: SyntaxObjectId) -> Option<SyntaxObjectId> {
        self.analysis.resolve_alias(id)
    }

    pub fn flatten_anonymous_functions(&mut self) {
        FlattenAnonymousFunctionCalls::flatten(&self.analysis, self.exprs);
    }

    pub fn remove_unused_imports(&mut self) {
        let mut unused = RemovedUnusedImports::new(&self.analysis);
        for expr in self.exprs.iter_mut() {
            unused.visit(expr);
        }
    }

    pub fn remove_unused_define_imports(&mut self) {
        let mut unused = RemoveUnusedDefineImports::new(&self.analysis);
        for expr in self.exprs.iter_mut() {
            unused.visit(expr);
        }
    }
}

#[cfg(test)]
mod analysis_pass_tests {

    use crate::{
        parser::{ast::AstTools, parser::Parser},
        rerrs::ErrorKind,
    };

    use super::*;

    #[test]
    fn local_defines() {
        let script = r#"
        (define (applesauce x y z) (list x y z))

        (define (bananas blegh) (applesauce blegh 10 20))

        "#;

        let mut exprs = Parser::parse(script).unwrap();
        let mut analysis = SemanticAnalysis::new(&mut exprs);
        analysis.populate_captures();

        // Inline top level definition -> naively just replace the value with the updated value
        // This should allow constant propagation to take place. TODO: Log optimization misses
        analysis.inline_function_call("applesauce").unwrap();

        analysis.exprs.pretty_print();

        // analysis.

        // for var in analysis.last_usages() {
        //     crate::rerrs::report_info(
        //         ErrorKind::FreeIdentifier.to_error_code(),
        //         "input.rkt",
        //         script,
        //         format!("last usage"),
        //         var.span,
        //     );
        // }
    }

    #[test]
    fn transducer_last_usages() {
        let script = r#"
        
        (define tmap
            (λ (f)
              (λ (reducer)
                (λ args
                  (let ((l (length args)))
                    (if (= l (length (quote ())))
                      (apply (λ () (reducer)) args)
                      (if (= l (length (quote (result))))
                        (apply (λ (result) (reducer result)) args)
                        (if (= l (length (quote (result input))))
                          (apply
                             (λ (result input)
                               (reducer result (f input)))
                             args)
                          (error! "Arity mismatch")))))))))
        "#;

        let mut exprs = Parser::parse(script).unwrap();
        let mut analysis = SemanticAnalysis::new(&mut exprs);
        analysis.populate_captures();

        for var in analysis.last_usages() {
            crate::rerrs::report_info(
                ErrorKind::FreeIdentifier.to_error_code(),
                "input.rkt",
                script,
                "last usage".to_string(),
                var.span,
            );
        }
    }

    #[test]
    fn escaping_functions() {
        let script = r#"
        (define generate-one-element-at-a-time
            (λ (lst)
              ((λ (control-state)
                   ((λ (control-state0)
                        (begin
                         (set! control-state control-state0)
                          (λ ()
                            (call/cc control-state))))
                      (λ (return)
                        (begin
                         (for-each
                             (λ (element)
                               (set! return
                                 (call/cc
                                    (λ (resume-here)
                                      (begin
                                       (set! control-state resume-here)
                                        (return element))))))
                             lst)
                          (return (quote you-fell-off-the-end))))))
                 123)))

        "#;

        let mut exprs = Parser::parse(script).unwrap();
        let mut analysis = SemanticAnalysis::new(&mut exprs);
        analysis.populate_captures();

        let let_vars = analysis
            .analysis
            .info
            .values()
            .filter(|x| x.kind == IdentifierStatus::HeapAllocated);

        for var in let_vars {
            println!("{var:?}");
            crate::rerrs::report_info(
                ErrorKind::FreeIdentifier.to_error_code(),
                "input.rkt",
                script,
                "let-var".to_string(),
                var.span,
            );
        }
    }

    #[test]
    fn last_usages_test() {
        let script = r#"
        (define Y 
            (lambda (f)
                ((lambda (x) (x x))
                (lambda (x) (f (lambda (y) ((x x) y)))))))
        "#;

        let mut exprs = Parser::parse(script).unwrap();
        let mut analysis = SemanticAnalysis::new(&mut exprs);
        analysis.populate_captures();

        for var in analysis.last_usages() {
            crate::rerrs::report_info(
                ErrorKind::FreeIdentifier.to_error_code(),
                "input.rkt",
                script,
                "last usage".to_string(),
                var.span,
            );
        }
    }

    #[test]
    fn mutated_and_captured() {
        let script = r#"
            (define (foo x)
                (lambda (y) 
                    (displayln x)
                    (set! x y)))
        "#;

        let mut exprs = Parser::parse(script).unwrap();

        let mut analysis = SemanticAnalysis::new(&mut exprs);
        analysis.populate_captures();
    }

    #[test]
    fn local_vars() {
        let script = r#"
            (define (applesauce)
                (+ 10 20 30 
                    (%plain-let ((a 10) (b 20))
                        (+ a b))))
        "#;

        let mut exprs = Parser::parse(script).unwrap();

        let analysis = SemanticAnalysis::new(&mut exprs);

        let let_vars = analysis
            .analysis
            .info
            .values()
            .filter(|x| x.kind == IdentifierStatus::Local);

        for var in let_vars {
            println!("{var:?}");
            crate::rerrs::report_info(
                ErrorKind::FreeIdentifier.to_error_code(),
                "input.rkt",
                script,
                "let-var".to_string(),
                var.span,
            );
        }
    }

    #[test]
    fn tail_call_eligible_test_with_let() {
        let script = r#"
            (define (loop value accum)
                ;; This should not get registered as a tail call
                (loop value (cons value accum))
                (let ((applesauce 10))
                    (if #true
                        (if #true
                            (loop value (cons value accum))
                            (loop value (cons value accum)))
                        (loop value accum))))
        "#;

        let mut exprs = Parser::parse(script).unwrap();
        let mut analysis = SemanticAnalysis::new(&mut exprs);
        analysis.populate_captures();

        let tail_calls = analysis
            .analysis
            .call_info
            .values()
            .filter(|x| matches!(x.kind, CallKind::SelfTailCall(_)))
            .collect::<Vec<_>>();

        assert_eq!(tail_calls.len(), 3);

        for var in tail_calls {
            println!("{var:?}");

            crate::rerrs::report_info(
                ErrorKind::FreeIdentifier.to_error_code(),
                "input.rkt",
                script,
                "tail call".to_string(),
                var.span,
            );
        }
    }

    #[test]
    fn tail_call_eligible_test_apply() {
        let script = r#"
        (define list-transduce
            (λ args
              (displayln args)
              (let ((l (length args)))
                (if (= l (length (quote (xform f coll))))
                  (apply
                     (λ (xform f coll)
                       (displayln f)
                       (displayln (multi-arity? f))
                       (displayln list-transduce)
                       (list-transduce xform f (f) coll))
                     args)
                  (if (= l (length (quote (xform f init coll))))
                    (apply
                       (λ (xform f init coll)
                         (let ((xf (xform f)))
                           (let ((result (list-reduce xf init coll)))
                             (xf result))))
                       args)
                    (error! "Arity mismatch"))))))
        "#;

        let mut exprs = Parser::parse(script).unwrap();
        let mut analysis = SemanticAnalysis::new(&mut exprs);
        analysis.populate_captures();

        let tail_calls = analysis
            .analysis
            .call_info
            .values()
            .filter(|x| matches!(x.kind, CallKind::SelfTailCall(_)));

        for var in tail_calls {
            crate::rerrs::report_info(
                ErrorKind::FreeIdentifier.to_error_code(),
                "input.rkt",
                script,
                "tail call".to_string(),
                var.span,
            );
        }
    }

    #[test]
    fn tail_call_eligible_test() {
        let script = r#"
            (begin
                (define loop1
                    (λ (x)
                    (if (= x 100) x (loop1 (+ x 1))))))

            (define (loop value accum)
                ;; This should not get registered as a tail call
                (loop value (cons value accum))
                (if #true
                    (if #true
                        (loop value (cons value accum))
                        (loop value (cons value accum)))
                    (loop value accum)))
        "#;

        let mut exprs = Parser::parse(script).unwrap();
        let mut analysis = SemanticAnalysis::new(&mut exprs);
        analysis.populate_captures();

        let tail_calls = analysis
            .analysis
            .call_info
            .values()
            .filter(|x| matches!(x.kind, CallKind::SelfTailCall(_)));

        for var in tail_calls {
            crate::rerrs::report_info(
                ErrorKind::FreeIdentifier.to_error_code(),
                "input.rkt",
                script,
                "tail call".to_string(),
                var.span,
            );
        }
    }

    #[test]
    fn find_last_usages() {
        let script = r#"
            (define (loop value accum)
                (+ value accum)
                (loop value (cons value accum)))
        "#;

        let mut exprs = Parser::parse(script).unwrap();
        let analysis = SemanticAnalysis::new(&mut exprs);

        let last_usages = analysis.last_usages().collect::<Vec<_>>();
        // In this case, we should be identifying just the two usages for
        // value and accum in the recursive call
        assert_eq!(last_usages.len(), 2);
    }

    #[test]
    fn analysis_pass_finds_call_sites() {
        let script = r#"
            (define (foo) (+ 1 2 3 4 5))
            (define (test) (let ((a 10) (b 20)) (foo)))
            (foo)
            (foo)
            (begin
                (foo)
                (foo)
                (foo))
        "#;

        let mut exprs = Parser::parse(script).unwrap();
        let analysis = SemanticAnalysis::new(&mut exprs);

        let mut count = 0;

        analysis.find_call_sites_and_call("foo", |_, _| count += 1);

        assert_eq!(count, 6);
    }

    #[test]
    fn resolve_alias() {
        let script = r#"
            (define list (%module-get%))
            (define alias-list list)
            (define alias-list2 alias-list)
            (define alias-list3 alias-list2)
            (define alias-list4 alias-list3)
        "#;

        // let mut analysis = Analysis::default();
        let mut exprs = Parser::parse(script).unwrap();
        {
            let analysis = SemanticAnalysis::new(&mut exprs);

            let list_id = analysis
                .query_top_level_define("list")
                .and_then(|x| x.name_id())
                .unwrap();

            let alias_list_4_id = analysis
                .query_top_level_define("alias-list4")
                .and_then(|x| x.name_id())
                .unwrap();

            let found = analysis.resolve_alias(alias_list_4_id);

            assert_eq!(list_id, found.unwrap());
        }
    }

    #[test]
    fn test_capture() {
        let script = r#"

            (define (adder x y z)
                (set! x 100)
                (lambda ()
                    (+ x y z)
                    (lambda ()
                        (+ x y z)
                        (lambda ()
                            (+ x y z)
                            (lambda () (+ x y z))))))

        "#;

        // let mut analysis = Analysis::default();
        let mut exprs = Parser::parse(script).unwrap();
        {
            let mut analysis = SemanticAnalysis::new(&mut exprs);
            analysis.populate_captures();

            // This _should_ print I think 4 lambdas that will have captured x -> each one should actually cascade the capture downward, copying in each value directly
            // Since x is an immutable capture, we're okay with this
            for function in analysis
                .analysis
                .function_info
                .values()
                .filter(|x| !x.captured_vars().is_empty())
            {
                println!("{:?}", function.captured_vars());
            }

            println!("{:?}", analysis.analysis.info.get(&SyntaxObjectId(2)));

            // println!("{:#?}", escaping_functions);
        }
    }

    #[test]
    fn test_complicated_escape_analysis() {
        let script = r#"

            (define (func-lift x) x)

            (define (adder x)

                ;; x == 2

                (define func (lambda () x))
                ;; This is pretty eligible for some sort of
                ;; lifting? closure conversion?
                (define func 
                    (let ((a 10) (b 20) (c 30))
                        (lambda () (+ a b c))))

                (func) -> (func-lift x) ;; replace known callsites with the lifted version
                (func) -> (func-lift x)

                (set! x 20)

                (func) -> (func-lift x)
                (func) -> (func-lift x)

                func ;; replace last callsite with something like
                     ;; (lambda () x) -> doesn't interfere with previous calls, and now captures the var
                )
        "#;

        // let mut analysis = Analysis::default();
        let mut exprs = Parser::parse(script).unwrap();
        {
            let analysis = SemanticAnalysis::new(&mut exprs);

            let escaping_functions = analysis
                .analysis
                .function_info
                .values()
                .filter(|x| x.escapes)
                .collect::<Vec<_>>();

            println!("{escaping_functions:#?}");
        }
    }

    #[test]
    fn test_lifting_local_functions_to_global_scope() {
        let script = r#"
            (define (test)
                (mapping (lambda (x) 10) (list 1 2 3 4 5)))        
        "#;

        let mut exprs = Parser::parse(script).unwrap();

        {
            let mut analysis = SemanticAnalysis::new(&mut exprs);

            analysis.lift_all_local_functions();
        }

        for expr in exprs {
            println!("{}", expr.to_pretty(60))
        }
    }

    #[test]
    fn test_rudimentary_escape_analysis() {
        let script = r#"
            (define (adder x)
                
                (map (lambda (y) (+ x y)) (list 1 2 3 4 5))

                (black-box (lambda (y) (+ x y))))
        "#;

        // let mut analysis = Analysis::default();
        let mut exprs = Parser::parse(script).unwrap();
        {
            let analysis = SemanticAnalysis::new(&mut exprs);

            let escaping_functions = analysis
                .analysis
                .function_info
                .values()
                .filter(|x| x.escapes)
                .collect::<Vec<_>>();

            println!("{escaping_functions:#?}");
        }
    }

    #[test]
    fn check_analysis_pass() {
        // let mut builder = Builder::new();

        // builder
        //     .is_test(true)
        //     .filter(
        //         Some("steel::compiler::passes::analysis"),
        //         LevelFilter::Trace,
        //     )
        //     .init();

        let script = r#"

        (define + (%module-get%))
        (define list (%module-get%))

        (define alias-list list)
        (define alias-list2 alias-list)
        (define alias-list3 alias-list2)
        (define alias-list4 alias-list3)

        (let ()
            (let ()
                (let ()
                    (let () (+ 1 2 3 4 5)))))

        (+ applesauce 20)

        (define applesauce 100)

        ;(let ((a 10) (b 20))
        ;    (+ a b c))

        ;(define (foo x y z)
        ;    (define (inner-func-that-captures a b c)
        ;        (inner-func-that-captures x y z))
        ;    (inner-func-that-captures 1 2 3))

        (define (test)
            (define (foo)
                    (bar))
            (define (bar)
                    (foo))
            (foo))

        (define (loop value accum)
            (loop (cons value accum)))

        ;(define (foo x y z)
        ;    (let ((x x) (y y))
        ;        (lambda (extra-arg x) 
        ;            (set! x 100)
        ;            (set! foo "hello world")
        ;            (+ z z z))))

        ;(define (test x y z)
        ;    (let ((foo foo))
        ;        (foo x y z)
        ;        (foo 1 2 3)
        ;        (foo 10 20 30)))
        
        ;(foo "applesauce" "bananas" "sauce")
        "#;

        // let mut analysis = Analysis::default();
        let mut exprs = Parser::parse(script).unwrap();
        {
            let mut analysis = SemanticAnalysis::new(&mut exprs);
            analysis.replace_pure_empty_lets_with_body();

            // Log the free identifiers
            let free_vars = analysis.find_free_identifiers();

            for var in free_vars {
                crate::rerrs::report_error(
                    ErrorKind::FreeIdentifier.to_error_code(),
                    "input.rkt",
                    script,
                    "Free identifier".to_string(),
                    var.span,
                );
            }

            let unused_args = analysis.find_unused_arguments();

            println!("Unused args: {unused_args:?}");

            for var in analysis.unused_variables() {
                crate::rerrs::report_warning(
                    ErrorKind::FreeIdentifier.to_error_code(),
                    "input.rkt",
                    script,
                    "Unused variable".to_string(),
                    var.span,
                );
            }

            for var in analysis.global_defs() {
                crate::rerrs::report_info(
                    ErrorKind::FreeIdentifier.to_error_code(),
                    "input.rkt",
                    script,
                    "global var".to_string(),
                    var.span,
                );
            }

            // for var in analysis.built_ins() {
            //     crate::rerrs::report_info(
            //         ErrorKind::FreeIdentifier.to_error_code(),
            //         "input.rkt",
            //         script,
            //         format!("built in function"),
            //         var.span,
            //     );
            // }

            for var in analysis.last_usages() {
                crate::rerrs::report_info(
                    ErrorKind::FreeIdentifier.to_error_code(),
                    "input.rkt",
                    script,
                    "last usage of variable".to_string(),
                    var.span,
                );
            }

            analysis.lift_pure_local_functions();
            // analysis.lift_local_functions();

            for expr in analysis.exprs.iter() {
                println!("{expr}");
            }

            let list_id = analysis
                .query_top_level_define("list")
                .unwrap()
                .name
                .atom_syntax_object()
                .unwrap()
                .syntax_object_id;

            let alias_list_4_id = analysis
                .query_top_level_define("alias-list4")
                .unwrap()
                .name
                .atom_syntax_object()
                .unwrap()
                .syntax_object_id;

            let found = analysis.resolve_alias(alias_list_4_id);

            println!(
                "List id: {list_id}, list 4 id: {alias_list_4_id}, resolved alias id: {found:?}"
            );
        }

        // println!("{}", exprs[0]);

        // let function_definit

        // analysis.find_call_sites_and_call(name, func)

        // analysis.run(&exprs);

        // for expr in &exprs {
        //     analysis.visit(expr);
        // }

        // find_call_sites_and_modify_with("foo", &analysis, &mut exprs, |l| {
        //     log::info!("Found a call site: {:?}", l.to_string())
        // });
    }
}
