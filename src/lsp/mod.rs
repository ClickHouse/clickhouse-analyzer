pub mod completion;
pub mod document_symbols;
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

use crate::connection::client::ConnectionConfig;
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

        let mut diags: Vec<Diagnostic> = enriched
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

        // Server-side validation via EXPLAIN PLAN (Tier 2+3)
        // Only run if connected and local parse produced no errors
        // (no point sending broken SQL to the server)
        // EXPLAIN PLAN validates columns, types, and table existence
        // without actually executing the query.
        // Split on semicolons to handle multi-statement files.
        if parse.errors.is_empty() {
            let meta = self.metadata.read().await;
            if meta.is_connected() {
                if let Some(client) = meta.client_ref() {
                    // Track byte offset of each statement for error highlighting
                    let full_source = &parse.source;
                    let mut offset = 0usize;
                    for stmt_text in full_source.split(';') {
                        let trimmed = stmt_text.trim();
                        let stmt_offset = offset
                            + stmt_text
                                .find(trimmed.chars().next().unwrap_or(' '))
                                .unwrap_or(0);
                        offset += stmt_text.len() + 1; // +1 for the semicolon

                        if trimmed.is_empty() {
                            continue;
                        }

                        // Skip non-query statements that EXPLAIN PLAN can't handle
                        let upper = trimmed.to_uppercase();
                        if !upper.starts_with("SELECT")
                            && !upper.starts_with("WITH")
                            && !upper.starts_with("INSERT")
                        {
                            continue;
                        }

                        let query = format!("EXPLAIN PLAN {trimmed}");
                        if let Err(e) = client.query_text(&query).await {
                            let msg = format!("{e}");
                            let range =
                                extract_error_range(&msg, trimmed, line_index, stmt_offset);
                            diags.push(Diagnostic {
                                range,
                                severity: Some(DiagnosticSeverity::WARNING),
                                source: Some("clickhouse-server".to_owned()),
                                message: msg,
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }

        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn try_connect(&self, settings: &serde_json::Value) {
        let ch = &settings["connection"];
        let enabled = ch["enabled"].as_bool().unwrap_or(false);

        if !enabled {
            let mut meta = self.metadata.write().await;
            if meta.is_connected() {
                meta.disconnect();
                self.client
                    .log_message(MessageType::INFO, "ClickHouse connection disabled")
                    .await;
            }
            return;
        }

        let config = ConnectionConfig {
            url: ch["url"]
                .as_str()
                .unwrap_or("http://localhost:8123")
                .to_string(),
            database: ch["database"]
                .as_str()
                .unwrap_or("default")
                .to_string(),
            username: ch["username"]
                .as_str()
                .unwrap_or("default")
                .to_string(),
            password: ch["password"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        };

        let url = config.url.clone();
        let mut meta = self.metadata.write().await;
        match meta.connect(config).await {
            Ok(()) => {
                let version = meta.server_version.as_deref().unwrap_or("unknown");
                let msg = format!("Connected to ClickHouse {} at {}", version, url);
                self.client.log_message(MessageType::INFO, msg).await;
            }
            Err(e) => {
                let msg = format!("Failed to connect to {}: {}", url, e);
                self.client.log_message(MessageType::WARNING, msg).await;
            }
        }
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
                document_symbol_provider: Some(OneOf::Left(true)),
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

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        // VS Code sends { "clickhouse-analyzer": { "connection": { ... } } }
        // or just the inner object depending on the client.
        let settings = &params.settings;
        let ch_settings = if settings.get("clickhouse-analyzer").is_some() {
            &settings["clickhouse-analyzer"]
        } else {
            settings
        };
        self.try_connect(ch_settings).await;
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

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let docs = self.lock_documents();
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };
        let symbols = document_symbols::handle_document_symbols(&doc.parse, &doc.line_index);
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }
}

/// Try to extract a source range from a ClickHouse error message.
/// Looks for backtick-quoted identifiers (e.g., `jdfdjfb`) or
/// single-quoted identifiers (e.g., 'nonexistent_table') in the error,
/// then finds them in the source to highlight the right word.
/// `stmt_offset` is the byte offset of the statement within the full document.
fn extract_error_range(
    error_msg: &str,
    stmt_source: &str,
    line_index: &LineIndex,
    stmt_offset: usize,
) -> tower_lsp::lsp_types::Range {
    // Try backtick-quoted identifier first: `name`
    // Then single-quoted: 'name'
    let identifier = extract_quoted(error_msg, '`')
        .or_else(|| extract_quoted(error_msg, '\''));

    if let Some(ident) = identifier {
        // Find this identifier in the statement source (case-insensitive)
        let lower_source = stmt_source.to_lowercase();
        let lower_ident = ident.to_lowercase();
        if let Some(pos) = lower_source.find(&lower_ident) {
            let abs_start = (stmt_offset + pos) as u32;
            let abs_end = (stmt_offset + pos + ident.len()) as u32;
            return line_index.range(abs_start, abs_end);
        }
    }

    // Fallback: highlight the first line of the statement
    let first_line_end = stmt_source.find('\n').unwrap_or(stmt_source.len());
    line_index.range(
        stmt_offset as u32,
        (stmt_offset + first_line_end) as u32,
    )
}

fn extract_quoted(s: &str, quote: char) -> Option<String> {
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == quote {
            let inner: String = chars.by_ref().take_while(|&c| c != quote).collect();
            if !inner.is_empty() {
                return Some(inner);
            }
        }
    }
    None
}

pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
