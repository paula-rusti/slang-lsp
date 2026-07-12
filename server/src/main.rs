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

use crate::resolution::ResolutionContext;

type FileEvent = (String, Option<String>);

#[derive(Debug)]
struct Backend {
    client: Client,
    workspace_root: Arc<std::sync::Mutex<Option<String>>>,  // store a shared ref to workspace root
    tx: mpsc::UnboundedSender<FileEvent>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        // currently we only support open, save, close events
        Ok(InitializeResult {
           capabilities: ServerCapabilities {
               text_document_sync: Some(TextDocumentSyncCapability::Options(
                   TextDocumentSyncOptions {
                       open_close: Some(true),
                       save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                       change: Some(TextDocumentSyncKind::FULL),
                       ..Default::default()
                   }
               )),
               ..Default::default()
           },
           ..Default::default()
       })
    }
    
    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "sv lsp initialized").await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let path = params.text_document.uri.to_file_path().unwrap();
        let path_str = path.to_str().unwrap();

        self.client.log_message(MessageType::INFO, format!("saved {}", path_str)).await;
        check_and_publish(path_str, None, &self.client, &self.workspace_root).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client.publish_diagnostics(params.text_document.uri, vec![], None).await;
    }
    
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let path = params.text_document.uri.to_file_path().unwrap();
        let path_str = path.to_str().unwrap();

        self.client.log_message(MessageType::INFO, format!("saved {}", path_str)).await;
        check_and_publish(path_str, None, &self.client, &self.workspace_root).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let path = params.text_document.uri.to_file_path().unwrap();
        let path_str = path.to_str().unwrap().to_string();

        let content = params.content_changes.into_iter().next().map(|c| c.text);

        self.client.log_message(MessageType::INFO, format!("did chane event for file {}", path_str)).await;
        
        let _ = self.tx.send((path_str, content));
    }
}

async fn check_and_publish(
    path: &str,
    content: Option<String>,
    client: &Client,
    workspace_root: &Arc<std::sync::Mutex<Option<String>>>,
) {
    let uri = match tower_lsp::lsp_types::Url::from_file_path(path) {
        Ok(uri) => uri,
        Err(_) => return 
    };

    let root = workspace_root.lock().unwrap().clone().unwrap_or_else(|| ".".to_string());
    let db = config::read_config(&root, Some("1800.2-2020.3.1/src".to_string()));
    let ctx = db.get(path);

    let diagnostics = run_check(path, content, ctx);
    
    let lsp_diags: Vec<Diagnostic> = diagnostics.iter().map(|d| {
        Diagnostic {
            range: Range { 
                start: Position { 
                    line: d.line.saturating_sub(1), 
                    character: d.column.saturating_sub(1), 
                }, 
                end: Position { 
                    line: d.line.saturating_sub(1), 
                    character: d.column 
                }
            },
            severity: Some(match d.severity.as_str() {
                "error" => DiagnosticSeverity::ERROR,
                "warning" => DiagnosticSeverity::WARNING,
                _ => DiagnosticSeverity::INFORMATION,
            }),
            message: d.message.clone(),
            source: Some("slang".to_string()),
            ..Default::default()
        }
    }).collect();

    client.log_message(MessageType::INFO, format!("published {} diagnostics for {}", lsp_diags.len(), path) ).await;
    client.publish_diagnostics(uri, lsp_diags, None).await;


}

// this is concerned with calling the invoker appropriately and collecting results and then cleanup
fn run_check(path: &str, content: Option<String>, ctx: &ResolutionContext) -> Vec<crate::invoker::Diagnostic> {
    let check_path;
    if let Some(ref text) = content {
        check_path = format!("{}.tmp_check.sv", path);
        std::fs::write(&check_path, text).expect("failed to write temp file!");
    } else {
        check_path = path.to_string();
    }

    let mut diagnostics = invoker::check_file(&check_path, ctx);

    if content.is_some() {
        let _ = std::fs::remove_file(&check_path);
        for d in &mut diagnostics {
            d.file = path.to_string();
        }
    }

    diagnostics
}

async fn debounce_worker(
    mut rx: mpsc::UnboundedReceiver<FileEvent>, 
    workspace_root: Arc<std::sync::Mutex<Option<String>>>,
    client: Client
) {
    
    // latest content per file 
    let mut last_touch: HashMap<String, (Instant, Option<String>)> = HashMap::new();

    loop {
        tokio::select! {
            // branch 1, new content was received, we update the content map
            Some((path, content)) = rx.recv() => {
                eprintln!("[worker] received: {}", path); 
                last_touch.insert(path, (Instant::now(), content));
            }
            // branch 2, 50ms passed without any updates, we check if we have old content to process (user stopped typing)
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                let now = Instant::now();
                let stale: Vec<FileEvent> = last_touch
                    .iter()
                    .filter(|(_, (t, _))| now.duration_since(*t) >= Duration::from_millis(300))
                    .map( |(p, (_, c))| (p.clone(), c.clone()) )
                    .collect();

                for (path, content) in &stale {
                    check_and_publish(path, content.clone(), &client, &workspace_root).await;
                    last_touch.remove(path);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let workspace_root = Arc::new(std::sync::Mutex::new(None::<String>));

    let (tx, rx) = mpsc::unbounded_channel::<(String, Option<String>)>();

    let (service, socket) = LspService::new(|client| {
        
        let worker_client = client.clone();
        let worker_root = workspace_root.clone();
        tokio::spawn(debounce_worker(rx, worker_root, worker_client));
        
        Backend {
            client,
            workspace_root,
            tx,
        }
    });
    Server::new(stdin, stdout, socket)
        .serve(service).await;
}