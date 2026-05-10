mod resolution;
mod config;
mod invoker;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    workspace_root: std::sync::Mutex<Option<String>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {

    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let root = params
            .workspace_folders
            .and_then(|folders| folders.into_iter().next())
            .map(|f| f.uri.to_file_path().unwrap().to_string_lossy().into_owned())
            .or_else(|| {
                params
                    .root_uri
                    .and_then(|u| u.to_file_path().ok().map(|p| p.to_string_lossy().into_owned()))
            })
            .unwrap_or_else(|| ".".to_string());

        *self.workspace_root.lock().unwrap() = Some(root);

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close:            Some(true),
                        change:                Some(TextDocumentSyncKind::FULL),
                        save:                  Some(TextDocumentSyncSaveOptions::Supported(true)),
                        will_save:             Some(false),
                        will_save_wait_until:  Some(false),
                    }
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name:    "sv-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "sv-lsp server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri      = params.text_document.uri.clone();
        let path     = uri.to_file_path().unwrap();
        let path_str = path.to_str().unwrap();

        self.client
            .log_message(MessageType::INFO, format!("did_open: {}", path_str))
            .await;

        let root = self.workspace_root.lock().unwrap().clone().unwrap_or_else(|| ".".to_string());
        let db  = config::read_config(&root, Some("1800.2-2020.3.1/src".to_string()));
        let ctx = db.get(path_str);

        let diagnostics = invoker::check_file(path_str, ctx);

        self.client
            .log_message(MessageType::INFO, format!("slang found {} diagnostic(s)", diagnostics.len()))
            .await;

        let lsp_diags: Vec<Diagnostic> = diagnostics.iter().map(|d| {
            Diagnostic {
                range: Range {
                    start: Position {
                        line:      d.line.saturating_sub(1),
                        character: d.column.saturating_sub(1),
                    },
                    end: Position {
                        line:      d.line.saturating_sub(1),
                        character: d.column,
                    },
                },
                severity: Some(match d.severity.as_str() {
                    "error"   => DiagnosticSeverity::ERROR,
                    "warning" => DiagnosticSeverity::WARNING,
                    _         => DiagnosticSeverity::INFORMATION,
                }),
                message: d.message.clone(),
                source:  Some("slang".to_string()),
                ..Default::default()
            }
        }).collect();

        self.client
            .log_message(MessageType::INFO, format!("publishing {} diagnostic(s)", lsp_diags.len()))
            .await;

        self.client.publish_diagnostics(uri, lsp_diags, None).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri      = params.text_document.uri;
        let path     = uri.to_file_path().unwrap();
        let path_str = path.to_str().unwrap();

        let root = self.workspace_root.lock().unwrap().clone().unwrap_or_else(|| ".".to_string());
        let db  = config::read_config(&root, Some("1800.2-2020.3.1/src".to_string()));
        let ctx = db.get(path_str);

        let diagnostics = invoker::check_file(path_str, ctx);

        let lsp_diags: Vec<Diagnostic> = diagnostics.iter().map(|d| {
            Diagnostic {
                range: Range {
                    start: Position {
                        line:      d.line.saturating_sub(1),
                        character: d.column.saturating_sub(1),
                    },
                    end: Position {
                        line:      d.line.saturating_sub(1),
                        character: d.column,
                    },
                },
                severity: Some(match d.severity.as_str() {
                    "error"   => DiagnosticSeverity::ERROR,
                    "warning" => DiagnosticSeverity::WARNING,
                    _         => DiagnosticSeverity::INFORMATION,
                }),
                message: d.message.clone(),
                source:  Some("slang".to_string()),
                ..Default::default()
            }
        }).collect();

        self.client.publish_diagnostics(uri, lsp_diags, None).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri      = params.text_document.uri.clone();
        let path     = uri.to_file_path().unwrap();
        let path_str = path.to_str().unwrap();

        self.client
            .log_message(MessageType::INFO, format!("saved: {}", path_str))
            .await;

        let root = self.workspace_root.lock().unwrap().clone().unwrap_or_else(|| ".".to_string());
        let db  = config::read_config(&root, Some("1800.2-2020.3.1/src".to_string()));
        let ctx = db.get(path_str);

        let diagnostics = invoker::check_file(path_str, ctx);

        let lsp_diags: Vec<Diagnostic> = diagnostics.iter().map(|d| {
            Diagnostic {
                range: Range {
                    start: Position {
                        line:      d.line.saturating_sub(1),
                        character: d.column.saturating_sub(1),
                    },
                    end: Position {
                        line:      d.line.saturating_sub(1),
                        character: d.column,
                    },
                },
                severity: Some(match d.severity.as_str() {
                    "error"   => DiagnosticSeverity::ERROR,
                    "warning" => DiagnosticSeverity::WARNING,
                    _         => DiagnosticSeverity::INFORMATION,
                }),
                message: d.message.clone(),
                source:  Some("slang".to_string()),
                ..Default::default()
            }
        }).collect();

        self.client
            .log_message(MessageType::INFO, format!("saved+checked: {} diag(s)", lsp_diags.len()))
            .await;

        self.client.publish_diagnostics(uri, lsp_diags, None).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        // clear diagnostics when file is closed
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }
}

#[tokio::main]
async fn main() {
    let stdin  = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        workspace_root: std::sync::Mutex::new(None),
    });

    Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}