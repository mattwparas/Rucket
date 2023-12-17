use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    error::Error,
    path::PathBuf,
    sync::Arc,
};

use dashmap::{DashMap, DashSet};

use ropey::Rope;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use steel::{
    compiler::passes::analysis::{RequiredIdentifierInformation, SemanticAnalysis},
    parser::{ast::ExprKind, expander::SteelMacro, interner::InternedString, parser::SourceId},
    rvals::FromSteelVal,
    steel_vm::{builtin::BuiltInModule, engine::Engine},
};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::notification::Notification;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use tower_lsp::lsp_types::SemanticTokenType;

use crate::diagnostics::{
    DiagnosticContext, DiagnosticGenerator, FreeIdentifiersAndUnusedIdentifiers,
};

pub const LEGEND_TYPE: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::STRING,
    SemanticTokenType::COMMENT,
    SemanticTokenType::NUMBER,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::PARAMETER,
];

pub struct Backend {
    pub client: Client,
    pub ast_map: DashMap<String, Vec<ExprKind>>,
    pub document_map: DashMap<String, Rope>,
    // TODO: This needs to hold macros to help with resolving definitions
    pub _macro_map: DashMap<String, HashMap<InternedString, SteelMacro>>,
    pub ignore_set: Arc<DashSet<InternedString>>,
    pub globals_set: Arc<DashSet<InternedString>>,
    pub defined_globals: DashSet<String>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            offset_encoding: None,
            capabilities: ServerCapabilities {
                inlay_hint_provider: Some(OneOf::Left(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec!["(".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![],
                    work_done_progress_options: Default::default(),
                }),

                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: {
                                TextDocumentRegistrationOptions {
                                    document_selector: Some(vec![DocumentFilter {
                                        language: Some("scheme".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    }]),
                                }
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions::default(),
                                legend: SemanticTokensLegend {
                                    token_types: LEGEND_TYPE.into(),
                                    token_modifiers: vec![],
                                },
                                range: Some(true),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                            },
                            static_registration_options: StaticRegistrationOptions::default(),
                        },
                    ),
                ),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
    }
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: params.text_document.text,
            version: params.text_document.version,
        })
        .await
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: std::mem::take(&mut params.content_changes[0].text),
            version: params.text_document.version,
        })
        .await
    }

    // TODO: In order to get changes in one file to propagate to another,
    // This _should_ trigger a recompile for the file - this will need to probably
    // read the whole file, and then re run the analysis?
    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }

    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        // TODO: In order for this to work, we'll have to both
        // expose a span -> URI function, as well as figure out how to
        // decide if a definition refers to an import. I think deciding
        // if something is a module import should be like:
        let _definition = async {
            let uri = params.text_document_position_params.text_document.uri;
            let mut ast = self.ast_map.get_mut(uri.as_str())?;
            let mut rope = self.document_map.get(uri.as_str())?.clone();

            let position = params.text_document_position_params.position;
            let char = rope.try_line_to_char(position.line as usize).ok()?;
            let offset = char + position.character as usize;

            let analysis = SemanticAnalysis::new(&mut ast);

            let (_syntax_object_id, information) =
                analysis.find_identifier_at_offset(offset, uri_to_source_id(&uri).unwrap())?;


            let refers_to = information.refers_to?;

            let maybe_definition = analysis.get_identifier(refers_to)?;

            let mut resulting_span = maybe_definition.span;

            log::debug!("Refers to information: {:?}", &maybe_definition);

            if maybe_definition.is_required_identifier {
                match analysis.resolve_required_identifier(refers_to)? {
                    RequiredIdentifierInformation::Resolved(resolved) => {
                        resulting_span = resolved.span;
                    }

                    RequiredIdentifierInformation::Unresolved(interned, name) => {

                        log::debug!("Found unresolved identifier: {} - {}", interned, name);

                        let module_path_to_check = name
                            .trim_start_matches("mangler")
                            .trim_end_matches(interned.resolve())
                            .trim_end_matches("__%#__");

                        resulting_span = ENGINE.with_borrow(|engine| {
                            log::debug!(
                                "Compiled modules: {:?}",
                                engine.modules().keys().collect::<Vec<_>>()
                            );

                            log::debug!("Searching for: {} in {}", name, module_path_to_check);

                            let module = engine
                                .modules()
                                .get(&PathBuf::from(module_path_to_check))
                                .unwrap();

                            let module_ast = module.get_ast();

                            // for expr in ast {
                            //     log::debug!("{}", expr);
                            // }

                            let top_level_define =
                                steel::compiler::passes::analysis::query_top_level_define(
                                    module_ast,
                                    interned.resolve(),
                                ).or_else(||
                                steel::compiler::passes::analysis::query_top_level_define_on_condition(
                                    module_ast, interned.resolve(), |name, target| name.ends_with(target))
                                )?;

                            log::debug!("Found define: {}", top_level_define);

                            top_level_define.name.atom_syntax_object().map(|x| x.span)

                            // top_level_define
                        })?;
                    }
                }

                log::debug!("Found new definition: {:?}", maybe_definition);
            }

            let location = source_id_to_uri(resulting_span.source_id()?)?;

            log::debug!("Location: {:?}", location);
            log::debug!("Rope length: {:?}", rope.len_chars());
            // log::debug!("span: {:?}", maybe_definition.span);

            if location != uri {
                log::debug!("Jumping to definition that is not yet in the document map!");

                let expression =
                    ENGINE.with_borrow(|x| x.get_source(&resulting_span.source_id().unwrap()))?;

                rope = self
                    .document_map
                    .get(location.as_str())
                    .map(|x| x.clone())
                    .unwrap_or_else(|| Rope::from_str(&expression));

                self.document_map.insert(location.to_string(), rope.clone());
            }

            log::debug!("Location: {:?}", location);
            log::debug!("Rope length: {:?}", rope.len_chars());
            // log::debug!("span: {:?}", maybe_definition.span);

            let start_position = offset_to_position(resulting_span.start, &rope)?;
            let end_position = offset_to_position(resulting_span.end, &rope)?;

            let range = Range::new(start_position, end_position);

            log::debug!("{:?}", range);

            Some(GotoDefinitionResponse::Scalar(Location::new(
                location, range,
            )))

        }
        .await;
        // Ok(definition)

        todo!()
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        // TODO: In order for this to work, we'll have to both
        // expose a span -> URI function, as well as figure out how to
        // decide if a definition refers to an import. I think deciding
        // if something is a module import should be like:
        let definition = async {
            let uri = params.text_document_position_params.text_document.uri;
            let mut ast = self.ast_map.get_mut(uri.as_str())?;
            let mut rope = self.document_map.get(uri.as_str())?.clone();

            let position = params.text_document_position_params.position;
            let char = rope.try_line_to_char(position.line as usize).ok()?;
            let offset = char + position.character as usize;

            let analysis = SemanticAnalysis::new(&mut ast);

            let (_syntax_object_id, information) =
                analysis.find_identifier_at_offset(offset, uri_to_source_id(&uri).unwrap())?;


            let refers_to = information.refers_to?;

            let maybe_definition = analysis.get_identifier(refers_to)?;

            let mut resulting_span = maybe_definition.span;

            log::debug!("Refers to information: {:?}", &maybe_definition);

            if maybe_definition.is_required_identifier {
                match analysis.resolve_required_identifier(refers_to)? {
                    RequiredIdentifierInformation::Resolved(resolved) => {
                        resulting_span = resolved.span;
                    }

                    RequiredIdentifierInformation::Unresolved(interned, name) => {

                        log::debug!("Found unresolved identifier: {} - {}", interned, name);

                        let module_path_to_check = name
                            .trim_start_matches("mangler")
                            .trim_end_matches(interned.resolve())
                            .trim_end_matches("__%#__");

                        resulting_span = ENGINE.with_borrow(|engine| {
                            log::debug!(
                                "Compiled modules: {:?}",
                                engine.modules().keys().collect::<Vec<_>>()
                            );

                            log::debug!("Searching for: {} in {}", name, module_path_to_check);

                            let module = engine
                                .modules()
                                .get(&PathBuf::from(module_path_to_check))
                                .unwrap();

                            let module_ast = module.get_ast();

                            // for expr in ast {
                            //     log::debug!("{}", expr);
                            // }

                            let top_level_define =
                                steel::compiler::passes::analysis::query_top_level_define(
                                    module_ast,
                                    interned.resolve(),
                                ).or_else(||
                                steel::compiler::passes::analysis::query_top_level_define_on_condition(
                                    module_ast, interned.resolve(), |name, target| name.ends_with(target))
                                )?;

                            log::debug!("Found define: {}", top_level_define);

                            top_level_define.name.atom_syntax_object().map(|x| x.span)

                            // top_level_define
                        })?;
                    }
                }

                log::debug!("Found new definition: {:?}", maybe_definition);
            }

            let location = source_id_to_uri(resulting_span.source_id()?)?;

            log::debug!("Location: {:?}", location);
            log::debug!("Rope length: {:?}", rope.len_chars());
            // log::debug!("span: {:?}", maybe_definition.span);

            if location != uri {
                log::debug!("Jumping to definition that is not yet in the document map!");

                let expression =
                    ENGINE.with_borrow(|x| x.get_source(&resulting_span.source_id().unwrap()))?;

                rope = self
                    .document_map
                    .get(location.as_str())
                    .map(|x| x.clone())
                    .unwrap_or_else(|| Rope::from_str(&expression));

                self.document_map.insert(location.to_string(), rope.clone());
            }

            log::debug!("Location: {:?}", location);
            log::debug!("Rope length: {:?}", rope.len_chars());
            // log::debug!("span: {:?}", maybe_definition.span);

            let start_position = offset_to_position(resulting_span.start, &rope)?;
            let end_position = offset_to_position(resulting_span.end, &rope)?;

            let range = Range::new(start_position, end_position);

            log::debug!("{:?}", range);

            Some(GotoDefinitionResponse::Scalar(Location::new(
                location, range,
            )))

        }
        .await;
        Ok(definition)
    }

    async fn references(&self, _params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        Ok(None)
    }

    async fn semantic_tokens_full(
        &self,
        _params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        Ok(None)
    }

    async fn semantic_tokens_range(
        &self,
        _params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        Ok(None)
    }

    async fn inlay_hint(
        &self,
        _params: tower_lsp::lsp_types::InlayHintParams,
    ) -> Result<Option<Vec<InlayHint>>> {
        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let mut filter_character = None;

        let completions = || -> Option<Vec<CompletionItem>> {
            let rope = self.document_map.get(&uri.to_string())?;
            let mut ast = self.ast_map.get_mut(&uri.to_string())?;
            let char = rope.try_line_to_char(position.line as usize).ok()?;

            let offset = char + position.character as usize;

            if offset > 0 {
                let previously_typed = rope.get_char(offset - 1);

                if previously_typed.is_some() && previously_typed != Some('(') {
                    if offset > 2 {
                        let prior = rope.get_char(offset - 2);

                        if prior.is_some() && prior.map(char::is_whitespace).unwrap() {
                            filter_character = previously_typed;
                        }
                    } else {
                        filter_character = previously_typed;
                    }
                }
            }

            let filter_interned_string = |interned_string: &InternedString| {
                filter_interned_string_with_char(interned_string, filter_character)
            };

            let analysis = SemanticAnalysis::new(&mut ast);

            // Finds the scoped contexts that we're currently inside of by the span
            let contexts =
                analysis.find_contexts_with_offset(offset, uri_to_source_id(&uri).unwrap());

            let now = std::time::Instant::now();

            let mut completions: HashSet<String> =
                HashSet::with_capacity(contexts.len() + self.defined_globals.len());

            for context in contexts {
                match context {
                    steel::compiler::passes::analysis::SemanticInformationType::Function(info) => {
                        completions.extend(
                            info.arguments()
                                .keys()
                                .filter_map(filter_interned_string)
                                .chain(
                                    info.captured_vars()
                                        .keys()
                                        .filter_map(filter_interned_string),
                                ),
                        );
                    }
                    steel::compiler::passes::analysis::SemanticInformationType::Let(info) => {
                        completions.extend(info.arguments.keys().filter_map(filter_interned_string))
                    }
                    _ => {}
                }
            }

            // A bit sillys
            completions.extend(
                analysis
                    .find_global_defs()
                    .into_iter()
                    .filter_map(|x| filter_interned_string(&x.0)),
            );

            completions.extend(self.defined_globals.iter().filter_map(|x| {
                if let Some(c) = filter_character {
                    if !x.starts_with(c) {
                        return None;
                    }
                }

                Some(x.clone())
            }));

            // TODO: Build completions from macros that have been introduced into this scope
            completions.extend(ENGINE.with_borrow(|engine| {
                engine
                    .in_scope_macros()
                    .keys()
                    .filter_map(|x| {
                        if let Some(c) = filter_character {
                            if !x.resolve().starts_with(c) {
                                return None;
                            }
                        }

                        Some(x.resolve().to_string())
                    })
                    .collect::<Vec<_>>()
            }));

            let mut ret = Vec::with_capacity(completions.len());
            for var in completions {
                ret.push(CompletionItem {
                    label: var.clone(),
                    insert_text: Some(var.clone()),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(var),
                    ..Default::default()
                });
            }

            log::debug!("Time to calculate completions: {:?}", now.elapsed());

            Some(ret)
        }();

        Ok(completions.map(CompletionResponse::Array))
    }

    async fn rename(&self, _params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        Ok(None)
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "configuration changed!")
            .await;
    }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        self.client
            .log_message(MessageType::INFO, "workspace folders changed!")
            .await;
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        self.client
            .log_message(MessageType::INFO, "watched files have changed!")
            .await;
    }

    async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
        self.client
            .log_message(MessageType::INFO, "command executed!")
            .await;

        match self.client.apply_edit(WorkspaceEdit::default()).await {
            Ok(res) if res.applied => self.client.log_message(MessageType::INFO, "applied").await,
            Ok(_) => self.client.log_message(MessageType::INFO, "rejected").await,
            Err(err) => self.client.log_message(MessageType::ERROR, err).await,
        }

        Ok(None)
    }
}

fn filter_interned_string_with_char(
    interned: &InternedString,
    filter_char: Option<char>,
) -> Option<String> {
    let resolved = interned.resolve();

    if let Some(c) = filter_char {
        if !resolved.starts_with(c) {
            return None;
        }
    }

    if !resolved.starts_with("#")
        && !resolved.starts_with("%")
        && !resolved.starts_with("mangler#%")
        && !resolved.starts_with("!!dummy-rest")
    {
        Some(resolved.to_string())
    } else {
        None
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct InlayHintParams {
    path: String,
}

enum CustomNotification {}
impl Notification for CustomNotification {
    type Params = InlayHintParams;
    const METHOD: &'static str = "custom/notification";
}
struct TextDocumentItem {
    uri: Url,
    text: String,
    version: i32,
}
impl Backend {
    async fn on_change(&self, params: TextDocumentItem) {
        let now = std::time::Instant::now();

        self.client
            .log_message(MessageType::INFO, "on change")
            .await;

        let rope = ropey::Rope::from_str(&params.text);
        self.document_map
            .insert(params.uri.to_string(), rope.clone());

        self.client
            .log_message(MessageType::INFO, &params.uri.as_str())
            .await;

        let expression = params.text;

        let diagnostics = {
            let program = ENGINE.with_borrow_mut(|x| {
                // TODO: Reuse this!a
                let macro_env_before: HashSet<InternedString> =
                    x.in_scope_macros().keys().copied().collect();

                // TODO: Add span to the macro definition!
                let mut introduced_macros: HashMap<InternedString, SteelMacro> = HashMap::new();

                let expressions = x.emit_expanded_ast_without_optimizations(
                    &expression,
                    params.uri.to_file_path().ok(),
                );

                x.in_scope_macros_mut().retain(|key, value| {
                    if macro_env_before.contains(key) {
                        return true;
                    } else {
                        // FIXME: Try to avoid this clone!
                        introduced_macros.insert(*key, value.clone());
                        false
                    }
                });

                expressions
            });

            let mut ast = match program {
                Ok(ast) => ast,
                Err(e) => {
                    // drop(engine_guard);

                    self.client
                        .log_message(MessageType::INFO, e.to_string())
                        .await;

                    if let Some(span) = e.span() {
                        let diagnostics = || {
                            let start_position = offset_to_position(span.start, &rope)?;
                            let end_position = offset_to_position(span.end, &rope)?;

                            Some(vec![Diagnostic::new_simple(
                                Range::new(start_position, end_position),
                                e.to_string(),
                            )])
                        };

                        if let Some(diagnostics) = diagnostics() {
                            self.client
                                .publish_diagnostics(
                                    params.uri.clone(),
                                    diagnostics,
                                    Some(params.version),
                                )
                                .await;
                        }
                    }

                    return;
                }
            };

            let id = uri_to_source_id(&params.uri);

            let analysis = SemanticAnalysis::new(&mut ast);

            let diagnostics = ENGINE.with_borrow(|engine| {
                FreeIdentifiersAndUnusedIdentifiers.diagnose(&mut DiagnosticContext {
                    engine: &engine,
                    analysis: &analysis,
                    uri: &params.uri,
                    source_id: id,
                    rope: rope.clone(),
                    globals_set: &self.globals_set,
                    ignore_set: &self.ignore_set,
                })
            });

            // let diagnostics = ENGINE.with_borrow(|engine| {
            //     analysis
            //         .free_identifiers_with_globals(engine.symbol_map())
            //         .into_iter()
            //         .flat_map(|(ident, info)| {
            //             if info.span.source_id() != id {
            //                 return None;
            //             }

            //             if self.globals_set.contains(&ident) {
            //                 return None;
            //             }

            //             let start_position = offset_to_position(info.span.start, &rope)?;
            //             let end_position = offset_to_position(info.span.end, &rope)?;

            //             // TODO: Publish the diagnostics for each file separately, if we have them
            //             Some(make_error(Diagnostic::new_simple(
            //                 Range::new(start_position, end_position),
            //                 format!("free identifier: {}", ident.resolve()),
            //             )))
            //         })
            //         .chain(analysis.find_unused_arguments().into_iter().flat_map(
            //             |(ident, span)| {
            //                 if span.source_id() != id {
            //                     return None;
            //                 }

            //                 if self.ignore_set.contains(&ident) {
            //                     return None;
            //                 }

            //                 let resolved = ident.resolve();

            //                 // This identifier has been renamed - so we can unmangle it and see if it
            //                 // is in the ignore set
            //                 if resolved.starts_with("##") && resolved.ends_with(char::is_numeric) {
            //                     if self.ignore_set.contains(
            //                         &resolved
            //                             .trim_start_matches("##")
            //                             .trim_end_matches(char::is_numeric)
            //                             .into(),
            //                     ) {
            //                         return None;
            //                     }
            //                 }

            //                 let start_position = offset_to_position(span.start, &rope)?;
            //                 let end_position = offset_to_position(span.end, &rope)?;

            //                 let mut diagnostic = Diagnostic::new_simple(
            //                     Range::new(start_position, end_position),
            //                     format!("unused variable"),
            //                 );

            //                 diagnostic.severity = Some(DiagnosticSeverity::INFORMATION);

            //                 Some(diagnostic)
            //             },
            //         ))
            //         .collect()
            // });

            self.ast_map.insert(params.uri.to_string(), ast);

            diagnostics
        };

        self.client
            .publish_diagnostics(params.uri.clone(), diagnostics, Some(params.version))
            .await;

        log::debug!("On change time taken: {:?}", now.elapsed());
    }
}

fn uri_to_source_id(uri: &Url) -> Option<steel::parser::parser::SourceId> {
    let id = ENGINE.with_borrow(|x| x.get_source_id(&uri.to_file_path().unwrap()));
    id
}

fn source_id_to_uri(source_id: SourceId) -> Option<Url> {
    let path = ENGINE.with_borrow(|x| x.get_path_for_source_id(&source_id))?;

    Some(Url::from_file_path(path).unwrap())
}

pub fn make_error(mut diagnostic: Diagnostic) -> Diagnostic {
    diagnostic.severity = Some(DiagnosticSeverity::ERROR);
    diagnostic
}

pub struct ExternalModuleResolver {
    modules: HashMap<String, BuiltInModule>,
}

impl ExternalModuleResolver {
    pub fn new(
        engine: &mut Engine,
        directory: PathBuf,
    ) -> std::result::Result<Self, Box<dyn Error>> {
        let mut modules = HashMap::new();

        for file in std::fs::read_dir(&directory)? {
            let file = file?;

            if file.path().is_file() {
                let contents = std::fs::read_to_string(file.path())?;

                let result = engine.compile_and_run_raw_program(&contents)?;

                for value in result {
                    if let Ok(module) = BuiltInModule::from_steelval(&value) {
                        modules.insert(module.name().to_string(), module);
                    }
                }
            }
        }

        // Check globals now - for anything that isn't returned directly
        for global in engine.globals() {
            if let Ok(module) = engine.extract::<BuiltInModule>(global.resolve()) {
                if !modules.contains_key(module.name().as_ref()) {
                    modules.insert(module.name().to_string(), module);
                }
            }
        }

        Ok(ExternalModuleResolver { modules })
    }
}

impl steel::steel_vm::engine::ModuleResolver for ExternalModuleResolver {
    fn resolve(&self, module: &str) -> Option<BuiltInModule> {
        self.modules.get(module).cloned()
    }
}

thread_local! {
    pub static ENGINE: RefCell<Engine> = RefCell::new(Engine::new());
}

// #[tokio::main]
// async fn main() {
//     env_logger::init();

//     let stdin = tokio::io::stdin();
//     let stdout = tokio::io::stdout();

//     // Use this to resolve the module configuration files
//     let mut resolver_engine = Engine::new();

//     let globals_set = Arc::new(DashSet::new());

//     globals_set.insert("#%ignore-unused-identifier".into());
//     globals_set.insert("#%register-global".into());

//     let cloned_set = globals_set.clone();
//     resolver_engine.register_fn("#%register-global", move |global: String| {
//         cloned_set.insert(InternedString::from(global))
//     });

//     let ignore_unused_set = Arc::new(DashSet::new());
//     let cloned_ignore_set = ignore_unused_set.clone();
//     resolver_engine.register_fn("#%ignore-unused-identifier", move |global: String| {
//         cloned_ignore_set.insert(InternedString::from(global));
//     });

//     ENGINE.with_borrow_mut(|x| {
//         x.register_module_resolver(
//             ExternalModuleResolver::new(
//                 &mut resolver_engine,
//                 PathBuf::from("/home/matt/.config/steel-lsp/"),
//             )
//             .unwrap(),
//         )
//     });

//     let defined_globals = DashSet::new();

//     ENGINE.with_borrow(|engine| {
//         for global in engine.globals() {
//             let resolved = global.resolve();
//             if !resolved.starts_with("#")
//                 && !resolved.starts_with("%")
//                 && !resolved.starts_with("mangler#%")
//             {
//                 defined_globals.insert(resolved.to_string());
//             }
//         }
//     });

//     let (service, socket) = LspService::build(|client| Backend {
//         client,
//         ast_map: DashMap::new(),
//         document_map: DashMap::new(),
//         _macro_map: DashMap::new(),
//         globals_set,
//         ignore_set: ignore_unused_set,
//         defined_globals,
//     })
//     .finish();

//     Server::new(stdin, stdout, socket).serve(service).await;
// }

pub fn offset_to_position(offset: usize, rope: &Rope) -> Option<Position> {
    let line = rope.try_char_to_line(offset).ok()?;
    let first_char_of_line = rope.try_line_to_char(line).ok()?;
    let column = offset - first_char_of_line;

    Some(Position::new(line as u32, column as u32))
}
