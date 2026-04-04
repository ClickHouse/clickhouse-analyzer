pub mod completion;
pub mod goto_definition;
pub mod hover;
pub mod line_index;
pub mod semantic_tokens;
pub mod signature_help;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use line_index::LineIndex;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;
use tower_lsp::jsonrpc::Result;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::diagnostics::{self, Severity as OurSeverity};
use crate::formatter::{self, FormatConfig};
use crate::metadata::cache::{MetadataCache, SharedMetadata};
use crate::parser;
use crate::parser::diagnostic::Parse;

/// Maximum number of simultaneously open documents.
const MAX_DOCUMENTS: usize = 1000;
/// Maximum size of a single document in bytes (10 MB).
const MAX_DOCUMENT_SIZE: usize = 10 * 1024 * 1024;

struct DocumentState {
    parse: Parse,
    line_index: LineIndex,
}

impl DocumentState {
    fn new(source: String) -> Self {
        let line_index = LineIndex::new(&source);
        let parse = parser::parse(&source);
        Self { parse, line_index }
    }

    fn source(&self) -> &str {
        &self.parse.source
    }
}

pub struct Backend {
    client: Client,
    documents: Mutex<HashMap<Url, DocumentState>>,
    metadata: SharedMetadata,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Mutex::new(HashMap::new()),
            metadata: Arc::new(RwLock::new(MetadataCache::from_compiled_defaults())),
        }
    }

    fn lock_documents(&self) -> std::sync::MutexGuard<'_, HashMap<Url, DocumentState>> {
        self.documents.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn publish_diagnostics(&self, uri: Url, parse: &Parse, line_index: &LineIndex) {
        let enriched = diagnostics::enrich_diagnostics(parse, &parse.source);

        let diags: Vec<Diagnostic> = enriched
            .iter()
            .map(|d| {
                let range = line_index.range(d.range.0 as u32, d.range.1 as u32);
                let severity = Some(match d.severity {
                    OurSeverity::Error => DiagnosticSeverity::ERROR,
                    OurSeverity::Warning => DiagnosticSeverity::WARNING,
                    OurSeverity::Hint => DiagnosticSeverity::HINT,
                });
                let related_information = if d.related.is_empty() {
                    None
                } else {
                    Some(
                        d.related
                            .iter()
                            .map(|r| DiagnosticRelatedInformation {
                                location: Location {
                                    uri: uri.clone(),
                                    range: line_index
                                        .range(r.range.0 as u32, r.range.1 as u32),
                                },
                                message: r.message.clone(),
                            })
                            .collect(),
                    )
                };
                Diagnostic {
                    range,
                    severity,
                    code: d.code.map(|c| NumberOrString::String(c.to_owned())),
                    source: Some("clickhouse-analyzer".to_owned()),
                    message: d.message.clone(),
                    related_information,
                    ..Default::default()
                }
            })
            .collect();

        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn publish_size_error(&self, uri: Url) {
        let diag = Diagnostic {
            range: Range::default(),
            severity: Some(DiagnosticSeverity::WARNING),
            source: Some("clickhouse-analyzer".to_owned()),
            message: format!(
                "Document exceeds maximum size of {} bytes and will not be analyzed",
                MAX_DOCUMENT_SIZE
            ),
            ..Default::default()
        };
        self.client.publish_diagnostics(uri, vec![diag], None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        _params: InitializeParams,
    ) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".into(),
                        " ".into(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".into(), ",".into()]),
                    retrigger_characters: Some(vec![",".into()]),
                    ..Default::default()
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: semantic_tokens::legend(),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            ..Default::default()
                        },
                    ),
                ),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "clickhouse-analyzer".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "clickhouse-analyzer LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let source = params.text_document.text;

        if source.len() > MAX_DOCUMENT_SIZE {
            self.publish_size_error(uri).await;
            return;
        }

        {
            let docs = self.lock_documents();
            if docs.len() >= MAX_DOCUMENTS && !docs.contains_key(&uri) {
                return;
            }
        }

        let doc = DocumentState::new(source);
        self.publish_diagnostics(uri.clone(), &doc.parse, &doc.line_index).await;
        self.lock_documents().insert(uri, doc);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        // We requested FULL sync, so there is exactly one change with the full text.
        if let Some(change) = params.content_changes.into_iter().next() {
            let source = change.text;

            if source.len() > MAX_DOCUMENT_SIZE {
                self.publish_size_error(uri).await;
                return;
            }

            let doc = DocumentState::new(source);
            self.publish_diagnostics(uri.clone(), &doc.parse, &doc.line_index).await;
            self.lock_documents().insert(uri, doc);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.lock_documents().remove(&uri);
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let docs = self.lock_documents();
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };
        let formatted = formatter::format(&doc.parse.tree, &FormatConfig::default(), &doc.parse.source);

        // Replace the entire document.
        let end_pos = doc.line_index.position(doc.source().len() as u32);
        Ok(Some(vec![TextEdit {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: end_pos,
            },
            new_text: formatted,
        }]))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let docs = self.lock_documents();
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };
        let tokens = semantic_tokens::compute(&doc.parse.tree, &doc.parse.source, &doc.line_index);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let doc_data = {
            let docs = self.lock_documents();
            docs.get(&uri)
                .map(|doc| (doc.parse.clone(), doc.line_index.clone()))
        };
        let Some((parse, line_index)) = doc_data else {
            self.client
                .log_message(MessageType::WARNING, "completion: document not found")
                .await;
            return Ok(None);
        };
        let items =
            completion::handle_completion(&parse, &line_index, position, &self.metadata).await;
        let msg = format!(
            "completion: {} items at line {} char {}",
            items.len(),
            position.line,
            position.character
        );
        self.client.log_message(MessageType::INFO, msg).await;
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let (parse, line_index) = {
            let docs = self.lock_documents();
            let Some(doc) = docs.get(&uri) else {
                return Ok(None);
            };
            (doc.parse.clone(), doc.line_index.clone())
        };
        Ok(hover::handle_hover(&parse, &line_index, position, &self.metadata).await)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let (parse, line_index) = {
            let docs = self.lock_documents();
            let Some(doc) = docs.get(&uri) else {
                return Ok(None);
            };
            (doc.parse.clone(), doc.line_index.clone())
        };
        Ok(goto_definition::handle_goto_definition(
            &parse,
            &line_index,
            position,
            &uri,
        ))
    }

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let (parse, line_index) = {
            let docs = self.lock_documents();
            let Some(doc) = docs.get(&uri) else {
                return Ok(None);
            };
            (doc.parse.clone(), doc.line_index.clone())
        };
        Ok(
            signature_help::handle_signature_help(&parse, &line_index, position, &self.metadata)
                .await,
        )
    }
}

pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
