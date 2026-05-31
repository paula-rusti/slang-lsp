mod resolution;
mod config;
mod invoker;

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Instant, Duration};

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tokio::sync::mpsc;

struct Backend {
    client: Client,
    workspace_root: Arc<std::sync::Mutex<Option<String>>>,  // Backend and worker hold a ref at the same time
    tx: mpsc::UnboundedSender<(String, Option<String>)>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {

    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // vsc sends this when connecting and the server needs to know which folder the user opened
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

        // lock the mutex to prevent data races and store workspace root so other methods can access it without needing to recompute
        *self.workspace_root.lock().unwrap() = Some(root);

        // this tell vsc what the server can do
        /*
        - text_document_sync with open_close: true and change: FULL — means "send me the full file content on open/change"                              
        - save: Supported(true) — means "tell me when the user saves"                                                                                   
        Without this, VS Code wouldn't send you didOpen, didChange, or didSave notifications. The server_info is just cosmetic (shows in VS Code's output 
        panel). 
         */
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
        let path     = params.text_document.uri.to_file_path().unwrap();
        let path_str = path.to_str().unwrap();

        self.client
            .log_message(MessageType::INFO, format!("did_open: {}", path_str))
            .await;

        check_and_publish(path_str, None, &self.workspace_root, &self.client).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let path     = params.text_document.uri.to_file_path().unwrap();
        let path_str = path.to_str().unwrap().to_string();
        let content = params.content_changes.into_iter().next().map(|c| c.text);
        let _ = self.tx.send((path_str.clone(), content));
        self.client.log_message(MessageType::INFO, format!("did_change: queued {}", path_str)).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let path     = params.text_document.uri.to_file_path().unwrap();
        let path_str = path.to_str().unwrap();

        self.client
            .log_message(MessageType::INFO, format!("saved: {}", path_str))
            .await;

        check_and_publish(path_str, None, &self.workspace_root, &self.client).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }
}

async fn check_and_publish(
    path: &str,
    content: Option<&str>,
    workspace_root: &Arc<std::sync::Mutex<Option<String>>>,
    client: &Client,
) {
    let root = workspace_root.lock().unwrap().clone().unwrap_or_else(|| ".".to_string());
    let db = config::read_config(&root, Some("1800.2-2020.3.1/src".to_string()));
    let ctx = db.get(path);

    let (check_path, temp_file) = if let Some(text) = content {
        let tmp = format!("{}.tmp_check.sv", path);
        std::fs::write(&tmp, text).expect("failed to write temp file");
        (tmp.clone(), Some(tmp))
    } else {
        (path.to_string(), None)
    };

    let mut diagnostics = invoker::check_file(&check_path, ctx);

    if temp_file.is_some() {
        for d in &mut diagnostics {
            d.file = path.to_string();
        }
    }

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

    if let Some(tmp) = temp_file {
        let _ = std::fs::remove_file(tmp);
    }

    if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(path) {
        client.log_message(MessageType::INFO, format!("check_and_publish: {} diag(s) for {}", lsp_diags.len(), path)).await;
        client.publish_diagnostics(uri, lsp_diags, None).await;
    }
}

#[tokio::main]
async fn main() {
    let stdin  = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (tx, mut rx) =  mpsc::unbounded_channel::<(String, Option<String>)>();

    let workspace_root = Arc::new(std::sync::Mutex::new(None::<String>));
    let worker_root = workspace_root.clone();

    let (service, socket) = LspService::new(|client| {
        let worker_client = client.clone();

        tokio::spawn(async move {
            let mut last_touch: HashMap<String, (Instant, Option<String>)> = HashMap::new();

            loop {
                tokio::select! {
                    Some((path, content)) = rx.recv() => {
                        eprintln!("[worker] received: {}", path); 
                        last_touch.insert(path, (Instant::now(), content));
                    }
                    _ = tokio::time::sleep(Duration::from_millis(50)) => {
                        let now = Instant::now();
                        let stale: Vec<(String, Option<String>)> = last_touch
                            .iter()
                            .filter(|(_, (t, _))| now.duration_since(*t) >= Duration::from_millis(300))
                            .map(|(p, (_, c))| (p.clone(), c.clone()))
                            .collect();

                        eprintln!("[worker] stale files: {:?}", stale);
                        for (path, content) in &stale {
                            check_and_publish(path, content.as_deref(), &worker_root, &worker_client).await;
                        }

                        for (path, _) in stale {
                            last_touch.remove(&path);
                        }
                    }
                }
            }
        });

        Backend {
            client,
            workspace_root,
            tx,
        }
    });

    Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}