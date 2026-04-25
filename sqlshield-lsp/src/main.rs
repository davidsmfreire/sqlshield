//! sqlshield-lsp — a Language Server that publishes schema-aware SQL
//! diagnostics for embedded SQL (`.py`, `.rs`) and plain `.sql` files.
//!
//! ## Configuration
//!
//! The server discovers `.sqlshield.toml` by walking up from the opened
//! workspace directory. See the repo README for the schema.

mod config;
mod server;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sqlshield_lsp=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(server::Backend::new).finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}
