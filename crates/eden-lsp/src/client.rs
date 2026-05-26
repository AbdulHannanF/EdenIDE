//! `LspClient` — manages one language-server child process.
//!
//! The client is designed to be called from the render thread without ever
//! blocking it: all outgoing messages are queued in an unbounded channel
//! (non-blocking send) and all incoming results are stored in shared
//! `Arc<Mutex<…>>` state that the render thread reads on each frame.
//!
//! The LSP initialise handshake runs automatically on spawn. Messages sent
//! before the handshake completes are buffered and flushed once `initialized`
//! fires.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use anyhow::Context as _;
use serde_json::{json, Value};
use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::mpsc;

use crate::rpc::{self, IncomingMessage};
use crate::types::{CompletionItem, DefinitionResult, Diagnostic, HoverCard, Position, Severity};

// ── shared-state aliases ──────────────────────────────────────────────────────

type DiagMap = Arc<RwLock<HashMap<String, Vec<Diagnostic>>>>;

// ── LspClient ────────────────────────────────────────────────────────────────

/// Manages one language-server child process.
///
/// All public methods are non-blocking and safe to call from the render thread.
pub struct LspClient {
    outbox: mpsc::UnboundedSender<String>,
    next_id: Arc<AtomicU64>,
    initialized: Arc<AtomicBool>,
    pending_queue: Arc<Mutex<Vec<String>>>,
    diagnostics: DiagMap,
    hover: Arc<Mutex<Option<HoverCard>>>,
    completions: Arc<Mutex<Vec<CompletionItem>>>,
    hover_req_id: Arc<Mutex<Option<u64>>>,
    completion_req_id: Arc<Mutex<Option<u64>>>,
    definition: Arc<Mutex<Option<DefinitionResult>>>,
    definition_req_id: Arc<Mutex<Option<u64>>>,
    /// Keeps the child process alive while the client exists.
    _child: Arc<Mutex<Child>>,
}

impl LspClient {
    /// Spawns the language server at `cmd args`, sends `initialize`, and
    /// starts the background read/write tasks on `handle`.
    ///
    /// `root_uri` is the workspace root as a `file://` URI string.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be spawned (e.g. the binary is
    /// not installed). Callers should treat this as "LSP unavailable" and fall
    /// back to no-op behaviour.
    pub fn spawn(
        handle: &tokio::runtime::Handle,
        cmd: &str,
        args: &[&str],
        root_uri: &str,
    ) -> anyhow::Result<Self> {
        let mut child = tokio::process::Command::new(cmd)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("spawn language server `{cmd}`"))?;

        let stdin: ChildStdin = child.stdin.take().context("child stdin")?;
        let stdout: ChildStdout = child.stdout.take().context("child stdout")?;

        let (tx, rx) = mpsc::unbounded_channel::<String>();
        let next_id = Arc::new(AtomicU64::new(1));
        let initialized = Arc::new(AtomicBool::new(false));
        let pending_queue: Arc<Mutex<Vec<String>>> = Arc::default();
        let diagnostics: DiagMap = Arc::default();
        let hover: Arc<Mutex<Option<HoverCard>>> = Arc::default();
        let completions: Arc<Mutex<Vec<CompletionItem>>> = Arc::default();
        let hover_req_id: Arc<Mutex<Option<u64>>> = Arc::default();
        let completion_req_id: Arc<Mutex<Option<u64>>> = Arc::default();
        let definition: Arc<Mutex<Option<DefinitionResult>>> = Arc::default();
        let definition_req_id: Arc<Mutex<Option<u64>>> = Arc::default();

        handle.spawn(write_loop(tokio::io::BufWriter::new(stdin), rx));
        handle.spawn(read_loop(
            BufReader::new(stdout),
            tx.clone(),
            initialized.clone(),
            pending_queue.clone(),
            diagnostics.clone(),
            hover.clone(),
            completions.clone(),
            hover_req_id.clone(),
            completion_req_id.clone(),
            definition.clone(),
            definition_req_id.clone(),
        ));

        let client = Self {
            outbox: tx,
            next_id,
            initialized,
            pending_queue,
            diagnostics,
            hover,
            completions,
            hover_req_id,
            completion_req_id,
            definition,
            definition_req_id,
            _child: Arc::new(Mutex::new(child)),
        };

        // Begin the handshake. The read loop completes it by sending
        // `initialized` after the server responds.
        let id = client.next_id.fetch_add(1, Ordering::Relaxed);
        client
            .outbox
            .send(rpc::frame_request(
                id,
                "initialize",
                json!({
                    "processId": std::process::id(),
                    "rootUri": root_uri,
                    "capabilities": {
                        "textDocument": {
                            "hover": { "contentFormat": ["plaintext", "markdown"] },
                            "completion": {
                                "completionItem": { "snippetSupport": false }
                            },
                            "publishDiagnostics": {}
                        }
                    }
                }),
            ))
            .ok();

        Ok(client)
    }

    // ── document lifecycle ─────────────────────────────────────────────────

    /// Sends `textDocument/didOpen` for `uri`.
    pub fn open_document(&self, uri: &str, language_id: &str, text: &str) {
        self.enqueue_or_send(rpc::frame_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            }),
        ));
    }

    /// Sends `textDocument/didChange` (full-document sync) for `uri`.
    pub fn change_document(&self, uri: &str, version: i32, text: &str) {
        self.enqueue_or_send(rpc::frame_notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }]
            }),
        ));
    }

    // ── queries ────────────────────────────────────────────────────────────

    /// The current diagnostics for `uri`. Returns an empty vec if none yet.
    pub fn diagnostics(&self, uri: &str) -> Vec<Diagnostic> {
        self.diagnostics
            .read()
            .map(|m| m.get(uri).cloned().unwrap_or_default())
            .unwrap_or_default()
    }

    /// Fires a `textDocument/hover` request. The result is available via
    /// [`hover`][Self::hover] on a subsequent frame.
    pub fn request_hover(&self, uri: &str, pos: Position) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        *self.hover_req_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
        self.enqueue_or_send(rpc::frame_request(
            id,
            "textDocument/hover",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": pos.line, "character": pos.character }
            }),
        ));
    }

    /// The most recently received hover card, if any.
    pub fn hover(&self) -> Option<HoverCard> {
        self.hover.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Discards the cached hover card (call when the cursor moves).
    pub fn clear_hover(&self) {
        *self.hover.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Fires a `textDocument/completion` request. Results are available via
    /// [`completions`][Self::completions] on a subsequent frame.
    pub fn request_completions(&self, uri: &str, pos: Position) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        *self.completion_req_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
        // Clear stale results immediately so the caller can detect "fresh".
        self.completions.lock().unwrap_or_else(|e| e.into_inner()).clear();
        self.enqueue_or_send(rpc::frame_request(
            id,
            "textDocument/completion",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": pos.line, "character": pos.character },
                "context": { "triggerKind": 1 }
            }),
        ));
    }

    /// The most recently received completion items (may be empty while a
    /// request is in flight).
    pub fn completions(&self) -> Vec<CompletionItem> {
        self.completions.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Fires a `textDocument/definition` request. The result is available via
    /// [`definition_result`][Self::definition_result] on a subsequent frame.
    pub fn request_definition(&self, uri: &str, pos: Position) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        *self.definition_req_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
        self.definition.lock().unwrap_or_else(|e| e.into_inner()).take();
        self.enqueue_or_send(rpc::frame_request(
            id,
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": pos.line, "character": pos.character }
            }),
        ));
        id
    }

    /// The most recently received definition result, if any.
    pub fn definition_result(&self) -> Option<DefinitionResult> {
        self.definition.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Discards the cached definition result.
    pub fn clear_definition(&self) {
        self.definition.lock().unwrap_or_else(|e| e.into_inner()).take();
    }

    // ── internals ─────────────────────────────────────────────────────────

    /// Sends `msg` immediately if the handshake is done, otherwise queues it.
    fn enqueue_or_send(&self, msg: String) {
        if self.initialized.load(Ordering::Acquire) {
            self.outbox.send(msg).ok();
        } else {
            self.pending_queue.lock().unwrap_or_else(|e| e.into_inner()).push(msg);
        }
    }
}

// ── background tasks ──────────────────────────────────────────────────────────

async fn write_loop(
    mut writer: tokio::io::BufWriter<ChildStdin>,
    mut rx: mpsc::UnboundedReceiver<String>,
) {
    use tokio::io::AsyncWriteExt as _;
    while let Some(msg) = rx.recv().await {
        if writer.write_all(msg.as_bytes()).await.is_err() {
            break;
        }
        if writer.flush().await.is_err() {
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn read_loop(
    mut reader: BufReader<ChildStdout>,
    outbox: mpsc::UnboundedSender<String>,
    initialized: Arc<AtomicBool>,
    pending_queue: Arc<Mutex<Vec<String>>>,
    diagnostics: DiagMap,
    hover: Arc<Mutex<Option<HoverCard>>>,
    completions: Arc<Mutex<Vec<CompletionItem>>>,
    hover_req_id: Arc<Mutex<Option<u64>>>,
    completion_req_id: Arc<Mutex<Option<u64>>>,
    definition: Arc<Mutex<Option<DefinitionResult>>>,
    definition_req_id: Arc<Mutex<Option<u64>>>,
) {
    loop {
        match rpc::read_message(&mut reader).await {
            Ok(Some(msg)) => handle_message(
                msg,
                &outbox,
                &initialized,
                &pending_queue,
                &diagnostics,
                &hover,
                &completions,
                &hover_req_id,
                &completion_req_id,
                &definition,
                &definition_req_id,
            ),
            Ok(None) => {
                tracing::info!("LSP server closed connection");
                break;
            }
            Err(err) => {
                tracing::warn!("LSP read error: {err:#}");
                break;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_message(
    msg: IncomingMessage,
    outbox: &mpsc::UnboundedSender<String>,
    initialized: &AtomicBool,
    pending_queue: &Mutex<Vec<String>>,
    diagnostics: &DiagMap,
    hover: &Mutex<Option<HoverCard>>,
    completions: &Mutex<Vec<CompletionItem>>,
    hover_req_id: &Mutex<Option<u64>>,
    completion_req_id: &Mutex<Option<u64>>,
    definition: &Mutex<Option<DefinitionResult>>,
    definition_req_id: &Mutex<Option<u64>>,
) {
    let is_init_response =
        msg.id.is_some() && !initialized.load(Ordering::Acquire) && msg.result.is_some();

    if is_init_response {
        outbox
            .send(rpc::frame_notification("initialized", json!({})))
            .ok();
        initialized.store(true, Ordering::Release);
        let queued: Vec<String> = pending_queue.lock().unwrap_or_else(|e| e.into_inner()).drain(..).collect();
        for queued_msg in queued {
            outbox.send(queued_msg).ok();
        }
        tracing::info!("LSP server initialised");
    } else if let Some(id) = msg.id {
        let result = msg.result.unwrap_or(Value::Null);
        if hover_req_id.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|h| h == id) {
            *hover.lock().unwrap_or_else(|e| e.into_inner()) = parse_hover(&result);
        } else if completion_req_id.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|c| c == id) {
            *completions.lock().unwrap_or_else(|e| e.into_inner()) = parse_completions(&result);
        } else if definition_req_id.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|d| d == id) {
            *definition.lock().unwrap_or_else(|e| e.into_inner()) = parse_definition(&result);
        }
    } else if let Some(ref method) = msg.method {
        handle_notification(method, msg.params, diagnostics);
    }
}

fn handle_notification(method: &str, params: Option<Value>, diagnostics: &DiagMap) {
    if method != "textDocument/publishDiagnostics" {
        return;
    }
    let Some(params) = params else { return };
    let uri = params["uri"].as_str().unwrap_or("").to_owned();
    let diags = params["diagnostics"]
        .as_array()
        .map(|arr| arr.iter().filter_map(parse_diagnostic).collect())
        .unwrap_or_default();
    if let Ok(mut map) = diagnostics.write() {
        map.insert(uri, diags);
    }
}

// ── LSP JSON → Eden types ─────────────────────────────────────────────────────

fn parse_diagnostic(v: &Value) -> Option<Diagnostic> {
    let sl = v["range"]["start"]["line"].as_u64()? as u32;
    let sc = v["range"]["start"]["character"].as_u64()? as u32;
    let el = v["range"]["end"]["line"].as_u64()? as u32;
    let ec = v["range"]["end"]["character"].as_u64()? as u32;
    let severity = match v["severity"].as_u64().unwrap_or(1) {
        2 => Severity::Warning,
        3 => Severity::Information,
        4 => Severity::Hint,
        _ => Severity::Error,
    };
    let message = v["message"].as_str()?.to_owned();
    Some(Diagnostic {
        start: Position { line: sl, character: sc },
        end: Position { line: el, character: ec },
        severity,
        message,
    })
}

fn parse_hover(v: &Value) -> Option<HoverCard> {
    if v.is_null() {
        return None;
    }
    let contents = match &v["contents"] {
        Value::String(s) => s.clone(),
        obj @ Value::Object(_) => obj["value"].as_str().unwrap_or("").to_owned(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|item| match item {
                Value::String(s) => Some(s.as_str()),
                obj @ Value::Object(_) => obj["value"].as_str(),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n---\n"),
        _ => return None,
    };
    if contents.trim().is_empty() {
        return None;
    }
    Some(HoverCard { contents })
}

fn parse_completions(v: &Value) -> Vec<CompletionItem> {
    let items = if let Some(arr) = v.as_array() {
        arr
    } else if let Some(arr) = v["items"].as_array() {
        arr
    } else {
        return Vec::new();
    };
    items.iter().filter_map(parse_completion_item).take(50).collect()
}

fn parse_completion_item(v: &Value) -> Option<CompletionItem> {
    let label = v["label"].as_str()?.to_owned();
    let insert_text = v["insertText"].as_str().unwrap_or(&label).to_owned();
    let detail = v["detail"].as_str().map(ToOwned::to_owned);
    Some(CompletionItem { label, insert_text, detail })
}

fn parse_definition(v: &Value) -> Option<DefinitionResult> {
    // Handle Location, LocationLink, or array thereof.
    let item = if v.is_array() {
        v.as_array()?.first()?
    } else if v.is_null() {
        return None;
    } else {
        v
    };
    let uri = item["uri"]
        .as_str()
        .or_else(|| item["targetUri"].as_str())?
        .to_owned();
    let range = if item["targetSelectionRange"].is_object() {
        &item["targetSelectionRange"]
    } else {
        &item["range"]
    };
    let line = range["start"]["line"].as_u64()? as u32;
    let character = range["start"]["character"].as_u64()? as u32;
    Some(DefinitionResult { uri, position: Position { line, character } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_diagnostic() {
        let v = json!({
            "range": {
                "start": { "line": 3, "character": 5 },
                "end":   { "line": 3, "character": 12 }
            },
            "severity": 1,
            "message": "mismatched types"
        });
        let d = parse_diagnostic(&v).unwrap();
        assert_eq!(d.start.line, 3);
        assert_eq!(d.start.character, 5);
        assert!(matches!(d.severity, Severity::Error));
        assert_eq!(d.message, "mismatched types");
    }

    #[test]
    fn parses_hover_string() {
        // Hover response wraps contents in an object; bare MarkedString form.
        let v = json!({ "contents": "just a string" });
        let h = parse_hover(&v).unwrap();
        assert_eq!(h.contents, "just a string");
    }

    #[test]
    fn parses_hover_object() {
        let v = json!({ "contents": { "kind": "markdown", "value": "**fn** main()" } });
        let h = parse_hover(&v).unwrap();
        assert_eq!(h.contents, "**fn** main()");
    }

    #[test]
    fn parses_completions_array() {
        let v = json!([
            { "label": "println", "insertText": "println!($0)", "detail": "macro" },
            { "label": "eprintln" }
        ]);
        let items = parse_completions(&v);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "println");
        assert_eq!(items[1].label, "eprintln");
        assert_eq!(items[1].insert_text, "eprintln");
    }

    #[test]
    fn null_hover_returns_none() {
        assert!(parse_hover(&Value::Null).is_none());
    }
}
