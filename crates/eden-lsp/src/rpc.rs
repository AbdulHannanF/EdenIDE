//! Minimal JSON-RPC 2.0 message framing for the LSP stdio transport.
//!
//! The LSP wire format is:
//! ```text
//! Content-Length: <N>\r\n
//! \r\n
//! <N bytes of UTF-8 JSON>
//! ```
//! We only implement what an LSP *client* needs: serialise outgoing
//! requests/notifications and deserialise incoming responses/notifications.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── outgoing message shapes ───────────────────────────────────────────────────

#[derive(Serialize)]
struct Request<'a, P: Serialize> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: P,
}

#[derive(Serialize)]
struct Notification<'a, P: Serialize> {
    jsonrpc: &'static str,
    method: &'a str,
    params: P,
}

// ── incoming message shape ────────────────────────────────────────────────────

/// A decoded incoming message from the server (response or notification).
#[derive(Deserialize, Debug)]
pub struct IncomingMessage {
    /// Present on responses; absent on notifications.
    #[serde(default)]
    pub id: Option<u64>,
    /// Present on notifications and server-to-client requests; absent on
    /// pure responses.
    pub method: Option<String>,
    /// Present on successful responses.
    pub result: Option<Value>,
    /// Present on error responses (retained for completeness; not currently acted on).
    #[allow(dead_code)]
    pub error: Option<Value>,
    /// Present on notifications and server-to-client requests.
    pub params: Option<Value>,
}

// ── serialise helpers ─────────────────────────────────────────────────────────

/// Serialises a JSON-RPC request into a fully-framed LSP wire message.
pub fn frame_request(id: u64, method: &str, params: impl Serialize) -> String {
    let body = serde_json::to_string(&Request {
        jsonrpc: "2.0",
        id,
        method,
        params,
    })
    .expect("request serialisation is infallible");
    frame(&body)
}

/// Serialises a JSON-RPC notification into a fully-framed LSP wire message.
pub fn frame_notification(method: &str, params: impl Serialize) -> String {
    let body = serde_json::to_string(&Notification {
        jsonrpc: "2.0",
        method,
        params,
    })
    .expect("notification serialisation is infallible");
    frame(&body)
}

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

// ── async reader ──────────────────────────────────────────────────────────────

/// Reads one JSON-RPC message from an async buffered reader.
///
/// Returns `Ok(None)` on EOF (server closed stdout).
pub async fn read_message(
    reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
) -> anyhow::Result<Option<IncomingMessage>> {
    use tokio::io::AsyncBufReadExt as _;
    use tokio::io::AsyncReadExt as _;

    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // blank line separates headers from body
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse().ok();
        }
    }

    let len = content_length
        .ok_or_else(|| anyhow::anyhow!("LSP message missing Content-Length header"))?;

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;

    let msg: IncomingMessage = serde_json::from_slice(&body)
        .map_err(|e| anyhow::anyhow!("LSP JSON parse error: {e}"))?;
    Ok(Some(msg))
}
