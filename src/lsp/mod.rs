pub mod line_index;
pub mod semantic_tokens;

use std::collections::HashMap;
use std::sync::Mutex;

use line_index::LineIndex;
use tower_lsp::lsp_types::*;
use tower_lsp::jsonrpc::Result;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::diagnostics::{self, Severity as OurSeverity};
use crate::formatter::{self, FormatConfig};
use crate::parser;

/// Maximum number of simultaneously open documents.
const MAX_DOCUMENTS: usize = 1000;
/// Maximum size of a single document in bytes (10 MB).
const MAX_DOCUMENT_SIZE: usize = 10 * 1024 * 1024;

struct DocumentState {
    source: String,
    line_index: LineIndex,
}

pub struct Backend {
    client: Client,
    documents: Mutex<HashMap<Url, DocumentState>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Mutex::new(HashMap::new()),
        }
    }

    fn lock_documents(&self) -> std::sync::MutexGuard<'_, HashMap<Url, DocumentState>> {
        self.documents.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn publish_diagnostics(&self, uri: Url, source: &str, line_index: &LineIndex) {
        let parse = parser::parse(source);
        let enriched = diagnostics::enrich_diagnostics(&parse, source);

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

        let line_index = LineIndex::new(&source);
        self.publish_diagnostics(uri.clone(), &source, &line_index).await;
        self.lock_documents().insert(
            uri,
            DocumentState {
                source,
                line_index,
            },
        );
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

            let line_index = LineIndex::new(&source);
            self.publish_diagnostics(uri.clone(), &source, &line_index).await;
            self.lock_documents().insert(
                uri,
                DocumentState {
                    source,
                    line_index,
                },
            );
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
        let parse = parser::parse(&doc.source);
        let formatted = formatter::format(&parse.tree, &FormatConfig::default(), &parse.source);

        // Replace the entire document.
        let end_pos = doc.line_index.position(doc.source.len() as u32);
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
        let parse = parser::parse(&doc.source);
        let tokens = semantic_tokens::compute(&parse.tree, &parse.source, &doc.line_index);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }
}

pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
