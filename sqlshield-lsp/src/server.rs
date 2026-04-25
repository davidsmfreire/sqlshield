//! Language Server backend. Holds open documents in a DashMap, re-validates
//! on didOpen / didChange, and publishes diagnostics back to the client.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use dashmap::DashMap;
use sqlshield::schema::{self, TablesAndColumns};
use sqlshield::validation;
use sqlshield::Dialect;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, InitializeResult, InitializedParams, MessageType,
    Position, Range, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer};
use tracing::{debug, error, info, warn};

use crate::config::{self, ServerConfig};

/// Per-request loaded state: the parsed schema plus the chosen dialect.
/// `schema_mtime` is captured at load time so the server can detect when the
/// underlying schema file has been edited and trigger a reload.
struct LoadedState {
    schema: TablesAndColumns,
    dialect: Dialect,
    schema_source: Option<PathBuf>,
    schema_mtime: Option<SystemTime>,
}

pub struct Backend {
    client: Client,
    documents: DashMap<Url, String>,
    state: RwLock<Option<Arc<LoadedState>>>,
    /// Workspace root captured at `initialize` time; needed so reloads can
    /// re-discover `.sqlshield.toml` after the schema file changes.
    root_dir: RwLock<Option<PathBuf>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
            state: RwLock::new(None),
            root_dir: RwLock::new(None),
        }
    }

    async fn load_state_from_dir(&self, dir: &Path) {
        *self.root_dir.write().await = Some(dir.to_path_buf());

        let cfg = match config::discover(dir) {
            Ok(cfg) => cfg,
            Err(err) => {
                self.log_error(format!(".sqlshield.toml problem: {err}"))
                    .await;
                ServerConfig::default()
            }
        };

        let loaded = match cfg.schema_path.as_ref() {
            Some(path) => match schema::load_schema_from_file(path, cfg.dialect) {
                Ok(schema) => LoadedState {
                    schema,
                    dialect: cfg.dialect,
                    schema_source: Some(path.clone()),
                    schema_mtime: file_mtime(path),
                },
                Err(err) => {
                    self.log_error(format!("failed to load schema {}: {err}", path.display()))
                        .await;
                    // Use empty schema so validation still runs; tables won't match.
                    LoadedState {
                        schema: TablesAndColumns::new(),
                        dialect: cfg.dialect,
                        schema_source: None,
                        schema_mtime: None,
                    }
                }
            },
            None => {
                self.log_info("no schema configured — LSP will not flag missing tables or columns")
                    .await;
                LoadedState {
                    schema: TablesAndColumns::new(),
                    dialect: cfg.dialect,
                    schema_source: None,
                    schema_mtime: None,
                }
            }
        };

        if let Some(ref p) = loaded.schema_source {
            self.log_info(format!("loaded schema from {}", p.display()))
                .await;
        }
        *self.state.write().await = Some(Arc::new(loaded));
    }

    /// Poll the schema file's mtime; if it has changed since the last load,
    /// re-run discovery + load. Cheap enough to call before every validation
    /// (one stat per buffer-change), and avoids the need for a dedicated file
    /// watcher or `workspace/didChangeWatchedFiles` registration.
    async fn maybe_reload(&self) {
        let needs_reload = {
            let guard = self.state.read().await;
            let Some(state) = guard.as_ref() else {
                return;
            };
            let Some(path) = state.schema_source.as_ref() else {
                return;
            };
            let current = file_mtime(path);
            current != state.schema_mtime
        };
        if !needs_reload {
            return;
        }
        let root = self.root_dir.read().await.clone();
        if let Some(root) = root {
            self.log_info("schema file changed — reloading").await;
            self.load_state_from_dir(&root).await;
        }
    }

    async fn validate_and_publish(&self, uri: Url, text: &str) {
        self.maybe_reload().await;
        let Some(state) = self.state.read().await.clone() else {
            return;
        };

        let Some(file_ext) = uri
            .to_file_path()
            .ok()
            .and_then(|p| p.extension().map(|e| e.to_string_lossy().to_string()))
        else {
            return;
        };

        let diagnostics = compute_diagnostics(text, &file_ext, &state);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn log_info(&self, msg: impl Into<String>) {
        let s = msg.into();
        info!("{s}");
        self.client.log_message(MessageType::INFO, s).await;
    }

    async fn log_error(&self, msg: impl Into<String>) {
        let s = msg.into();
        error!("{s}");
        self.client.log_message(MessageType::ERROR, s).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        // Pick the first workspace root (or the legacy root_uri) as the base
        // for config discovery.
        let root = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first())
            .map(|f| f.uri.clone())
            .or(params.root_uri.clone());

        if let Some(root_uri) = root {
            if let Ok(root_path) = root_uri.to_file_path() {
                self.load_state_from_dir(&root_path).await;
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "sqlshield-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.log_info("sqlshield-lsp ready").await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text;
        debug!("did_open: {uri}");
        self.documents.insert(uri.clone(), text.clone());
        self.validate_and_publish(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // TextDocumentSyncKind::FULL → exactly one content_change with the
        // full new text.
        let Some(change) = params.content_changes.into_iter().next_back() else {
            warn!("did_change with no content changes");
            return;
        };
        let uri = params.text_document.uri;
        self.documents.insert(uri.clone(), change.text.clone());
        self.validate_and_publish(uri, &change.text).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.remove(&uri);
        // Clear the client-side diagnostic list on close.
        self.client.publish_diagnostics(uri, vec![], None).await;
    }
}

/// Pure function: given document text + extension + server state, return the
/// list of LSP Diagnostics. Separated from `Backend` so it can be unit-tested
/// without spinning up the transport.
fn compute_diagnostics(text: &str, file_ext: &str, state: &LoadedState) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    match file_ext {
        "sql" => {
            // Treat the whole buffer as a single SQL query, validated directly
            // against the server's cached schema. Column info isn't threaded
            // through, so every diagnostic points at line 0.
            let dialect = state.dialect.as_sqlparser();
            match sqlparser::parser::Parser::parse_sql(dialect.as_ref(), text) {
                Ok(statements) => {
                    for desc in validation::validate_statements_with_schema(
                        &statements,
                        &state.schema,
                        state.dialect,
                    ) {
                        diagnostics.push(make_diagnostic(0, 0, 0, 0, desc));
                    }
                }
                Err(err) => {
                    diagnostics.push(make_diagnostic(0, 0, 0, 0, err.to_string()));
                }
            }
        }
        "py" | "rs" => {
            let dialect = state.dialect.as_sqlparser();
            match sqlshield::finder::find_queries_in_code_with_dialect(
                text.as_bytes(),
                file_ext,
                dialect.as_ref(),
            ) {
                Ok(queries) => {
                    let errors = validation::validate_queries_in_code(
                        &queries,
                        &state.schema,
                        state.dialect,
                    );
                    for err in errors {
                        // err.line is 1-based; LSP is 0-based.
                        let line = err.line.saturating_sub(1) as u32;
                        diagnostics.push(make_diagnostic(line, 0, line, u32::MAX, err.description));
                    }
                }
                Err(err) => {
                    warn!("finder error: {err}");
                }
            }
        }
        _ => {}
    }

    diagnostics
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

fn make_diagnostic(
    start_line: u32,
    start_char: u32,
    end_line: u32,
    end_char: u32,
    message: String,
) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: start_line,
                character: start_char,
            },
            end: Position {
                line: end_line,
                character: end_char,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("sqlshield".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn state() -> LoadedState {
        let mut schema = HashMap::new();
        schema.insert(
            "users".to_string(),
            HashSet::from(["id".to_string(), "name".to_string()]),
        );
        LoadedState {
            schema,
            dialect: Dialect::Generic,
            schema_source: None,
            schema_mtime: None,
        }
    }

    #[test]
    fn python_embedded_sql_flags_missing_column() {
        let s = state();
        let source = r#"
q = "SELECT email FROM users"
"#;
        let diags = compute_diagnostics(source, "py", &s);
        assert!(
            diags.iter().any(|d| d.message.contains("email")),
            "got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn rust_embedded_sql_flags_missing_table() {
        let s = state();
        let source = r#"fn x() { let _ = "SELECT id FROM ghosts"; }"#;
        let diags = compute_diagnostics(source, "rs", &s);
        assert!(diags.iter().any(|d| d.message.contains("ghosts")));
    }

    #[test]
    fn valid_python_embedded_sql_has_no_diagnostics() {
        let s = state();
        let source = r#"q = "SELECT id, name FROM users""#;
        let diags = compute_diagnostics(source, "py", &s);
        assert!(diags.is_empty(), "got: {diags:?}");
    }

    #[test]
    fn unknown_extension_yields_nothing() {
        let s = state();
        let diags = compute_diagnostics("SELECT email FROM users", "txt", &s);
        assert!(diags.is_empty());
    }

    #[test]
    fn python_diagnostic_line_is_zero_based() {
        let s = state();
        let source = "# line 1\n# line 2\nq = \"SELECT email FROM users\"\n";
        let diags = compute_diagnostics(source, "py", &s);
        assert_eq!(diags.len(), 1);
        // Line 3 in 1-based sqlshield output → line 2 in LSP.
        assert_eq!(diags[0].range.start.line, 2);
    }

    #[test]
    fn severity_is_error_and_source_is_sqlshield() {
        let s = state();
        let source = r#"q = "SELECT email FROM users""#;
        let diags = compute_diagnostics(source, "py", &s);
        assert!(!diags.is_empty());
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diags[0].source.as_deref(), Some("sqlshield"));
    }
}
