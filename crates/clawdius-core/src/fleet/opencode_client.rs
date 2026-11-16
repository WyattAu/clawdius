//! OpenCode fleet client — one control plane over a running OpenCode
//! background server.
//!
//! Every OpenCode TUI tab is a client of a single background server
//! (`opencode service start`). This module lets clawdius attach to that
//! same server and supervise all sessions from one place: list sessions
//! with token/outcome state, read history, create sessions, prompt them,
//! and abort runaway runs.
//!
//! # Wire contract (verified against opencode v2.3.0)
//!
//! - `GET  /api/session` → `{data: [session]}`
//! - `GET  /api/session/{id}` → `{data: session}`
//! - `GET  /api/session/{id}/message?directory=…` → `{data: [message]}`
//! - `POST /api/session?directory=…` body `{title?}` → new session
//! - `POST /api/session/{id}/message?directory=…` body
//!   `{parts: [{type:"text", text}], agent?, model?}` → prompt; returns the
//!   assistant message after generation
//! - `POST /api/session/{id}/abort?directory=…` → stop a busy session
//!
//! Session-scoped calls take a `directory` query because the server scopes
//! routes per project directory (the SDK does the same).
//!
//! # Discovery and auth
//!
//! [`OpenCodeClient::discover`] shells out to `opencode service status`
//! (prints a bare URL) and reads the Basic password from
//! `~/.config/opencode/service.json`. The password is never logged; the
//! [`std::fmt::Debug`] impl redacts it.

use crate::error::Error;
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// One session on the OpenCode server, flattened for dashboards.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    /// Session id (`ses_…`).
    pub id: String,
    /// Human title (often the first prompt's summary).
    pub title: Option<String>,
    /// `succeeded` / `failed` (server strings, kept verbatim).
    pub outcome: Option<String>,
    /// Agent that ran the session (`build`, `plan`, …).
    pub agent: Option<String>,
    /// Provider id of the session's model.
    pub provider_id: Option<String>,
    /// Model id of the session's model.
    pub model_id: Option<String>,
    /// Working directory the session is scoped to.
    pub directory: Option<String>,
    /// Server-side project id.
    pub project_id: Option<String>,
    /// Input tokens billed to the session.
    pub input_tokens: u64,
    /// Output tokens generated.
    pub output_tokens: u64,
    /// Reasoning tokens generated.
    pub reasoning_tokens: u64,
    /// Cache-read tokens.
    pub cache_read_tokens: u64,
    /// Billed cost (0 on free/self-hosted providers).
    pub cost: f64,
    /// Last activity, unix millis.
    pub updated_ms: Option<u64>,
}

/// One message in a session's history, with the text parts joined.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageSummary {
    /// Message id (`msg_…`).
    pub id: String,
    /// `user` / `assistant`.
    pub role: String,
    /// All `text` parts joined with newlines (reasoning parts excluded).
    pub text: String,
}

/// Provider/model pair for prompting a session.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelRef {
    /// Provider id (e.g. `zai-coding-plan`).
    pub provider_id: String,
    /// Model id (e.g. `glm-5.3-flash`).
    pub model_id: String,
}

impl ModelRef {
    /// Build a model reference.
    pub fn new(provider_id: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            model_id: model_id.into(),
        }
    }
}

/// One event from the server's `GET /api/event` SSE stream, narrowed to
/// what a fleet dashboard reacts to. Everything unrecognised stays
/// [`OpenCodeEvent::Other`] with its type string — never dropped silently.
#[derive(Debug, Clone, PartialEq)]
pub enum OpenCodeEvent {
    /// A session finished generating (`session.idle`).
    SessionIdle {
        /// Session that went idle.
        session_id: String,
    },
    /// A run failed (`session.error`).
    SessionError {
        /// Session that errored.
        session_id: String,
        /// Error text, when the server carried one.
        error: Option<String>,
    },
    /// Session metadata changed (`session.updated`).
    SessionUpdated {
        /// Session that changed.
        session_id: String,
    },
    /// Any other event type (message.part.updated, …).
    Other {
        /// The server's event type string.
        kind: String,
    },
}

impl OpenCodeEvent {
    /// Parse one SSE `data:` payload (a JSON event envelope).
    #[must_use]
    pub fn parse(payload: &str) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(payload).ok()?;
        let kind = v.get("type").and_then(|t| t.as_str())?.to_string();
        let props = v.get("properties");
        let session_id = props
            .and_then(|p| p.get("sessionID"))
            .and_then(|s| s.as_str())
            .map(str::to_string)
            .or_else(|| {
                props
                    .and_then(|p| p.get("info"))
                    .and_then(|i| i.get("id"))
                    .and_then(|s| s.as_str())
                    .map(str::to_string)
            });
        match kind.as_str() {
            "session.idle" => Some(Self::SessionIdle {
                session_id: session_id.unwrap_or_default(),
            }),
            "session.error" => Some(Self::SessionError {
                session_id: session_id.unwrap_or_default(),
                error: props.and_then(|p| p.get("error")).map(|e| e.to_string()),
            }),
            "session.updated" => Some(Self::SessionUpdated {
                session_id: session_id.unwrap_or_default(),
            }),
            _ => Some(Self::Other { kind }),
        }
    }
}

/// Parse one SSE frame into its `data:` payloads (multi-line data
/// joined). Returns `None` for comments, keep-alives, and frames with no
/// data line.
fn parse_sse_frame(frame: &str) -> Option<String> {
    let mut data_lines: Vec<&str> = Vec::new();
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
        // "event:", "id:", ":" comments are ignored — the payload's own
        // `type` field is the discriminator.
    }
    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    }
}

// --- wire types (server adds fields freely; everything defaults) ---

#[derive(Deserialize, Default)]
struct WireSession {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    model: Option<WireModel>,
    #[serde(default)]
    tokens: WireTokens,
    #[serde(default)]
    location: Option<WireLocation>,
    #[serde(default)]
    time: Option<WireTime>,
    #[serde(default)]
    cost: Option<f64>,
    #[serde(rename = "projectID", default)]
    project_id: Option<String>,
}

#[derive(Deserialize, Default)]
struct WireModel {
    #[serde(rename = "providerID", default)]
    provider_id: Option<String>,
    // Session responses carry the model id as `id`; prompt request bodies
    // spell it `modelID`. Accept both.
    #[serde(rename = "modelID", alias = "id", default)]
    model_id: Option<String>,
}

#[derive(Deserialize, Default)]
struct WireTokens {
    #[serde(default)]
    input: u64,
    #[serde(default)]
    output: u64,
    #[serde(default)]
    reasoning: u64,
    #[serde(default)]
    cache: Option<WireCache>,
}

#[derive(Deserialize, Default)]
struct WireCache {
    #[serde(default)]
    read: u64,
}

#[derive(Deserialize)]
struct WireLocation {
    #[serde(default)]
    directory: Option<String>,
}

#[derive(Deserialize)]
struct WireTime {
    #[serde(default)]
    updated: Option<u64>,
}

#[derive(Deserialize)]
struct WireMessage {
    id: String,
    #[serde(rename = "type", default)]
    role: Option<String>,
    // v2 stores parts under `parts`; older shapes used `content`.
    #[serde(default)]
    parts: Option<serde_json::Value>,
    #[serde(default)]
    content: Option<serde_json::Value>,
}

impl From<WireSession> for SessionSummary {
    fn from(w: WireSession) -> Self {
        Self {
            id: w.id,
            title: w.title,
            outcome: w.outcome,
            agent: w.agent,
            provider_id: w.model.as_ref().and_then(|m| m.provider_id.clone()),
            model_id: w.model.as_ref().and_then(|m| m.model_id.clone()),
            directory: w.location.and_then(|l| l.directory),
            project_id: w.project_id,
            input_tokens: w.tokens.input,
            output_tokens: w.tokens.output,
            reasoning_tokens: w.tokens.reasoning,
            cache_read_tokens: w.tokens.cache.map(|c| c.read).unwrap_or(0),
            cost: w.cost.unwrap_or(0.0),
            updated_ms: w.time.and_then(|t| t.updated),
        }
    }
}

fn joined_text(raw: Option<&serde_json::Value>) -> String {
    let Some(parts) = raw.and_then(|v| v.as_array()) else {
        return String::new();
    };
    parts
        .iter()
        .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("text"))
        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn unwrap_data(v: serde_json::Value) -> serde_json::Value {
    match v {
        // Envelopes carry `data` alongside metadata ({data, cursor, ...}).
        serde_json::Value::Object(map) if map.contains_key("data") => map["data"].clone(),
        other => other,
    }
}

/// One-word description of a JSON value for error messages.
fn short_kind(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "bool".to_string(),
        serde_json::Value::Number(_) => "number".to_string(),
        serde_json::Value::String(_) => "string".to_string(),
        serde_json::Value::Array(a) => format!("array({})", a.len()),
        serde_json::Value::Object(m) => {
            let mut keys: Vec<String> = m.keys().take(4).cloned().collect();
            keys.sort();
            format!("object{{{}}}", keys.join(","))
        },
    }
}

/// Default path of the service credentials file.
fn default_service_json() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/opencode/service.json"))
}

/// Client for one running OpenCode background server.
pub struct OpenCodeClient {
    base_url: String,
    password: String,
    http: reqwest::Client,
}

impl fmt::Debug for OpenCodeClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenCodeClient")
            .field("base_url", &self.base_url)
            .field("password", &"<redacted>")
            .finish()
    }
}

impl Clone for OpenCodeClient {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            password: self.password.clone(),
            http: self.http.clone(),
        }
    }
}

impl OpenCodeClient {
    /// Attach to a known server URL with the service password.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] when the URL cannot be parsed or the HTTP client
    /// cannot be built.
    pub fn attach(base_url: impl Into<String>, password: impl Into<String>) -> Result<Self, Error> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        if reqwest::Url::parse(&base_url).is_err() {
            return Err(Error::Config(format!(
                "opencode server url is not a valid URL: {base_url}"
            )));
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Config(format!("http client build failed: {e}")))?;
        Ok(Self {
            base_url,
            password: password.into(),
            http,
        })
    }

    /// Discover the running server: URL from `opencode service status`,
    /// password from `~/.config/opencode/service.json`.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] when the service is not running, the status output
    /// does not carry a URL, or the credentials file is missing/invalid.
    pub fn discover() -> Result<Self, Error> {
        let bin = std::env::var("OPENCODE_BIN").unwrap_or_else(|_| "opencode".to_string());
        let service_json = default_service_json().ok_or_else(|| {
            Error::Config("HOME is not set; cannot locate opencode service.json".to_string())
        })?;
        Self::discover_with(&bin, &service_json)
    }

    /// Testable discovery core: explicit binary name and credentials path.
    ///
    /// # Errors
    ///
    /// Same as [`Self::discover`].
    pub fn discover_with(bin: &str, service_json: &Path) -> Result<Self, Error> {
        let out = Command::new(bin)
            .arg("service")
            .arg("status")
            .output()
            .map_err(|e| {
                Error::Config(format!(
                    "cannot run `{bin} service status` ({e}); is the opencode service running?"
                ))
            })?;
        if !out.status.success() {
            return Err(Error::Config(format!(
                "`{bin} service status` failed ({}); start the server with `opencode service start`",
                out.status
            )));
        }
        let url = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("http://") || l.starts_with("https://"))
            .ok_or_else(|| {
                Error::Config(format!(
                    "`{bin} service status` printed no URL; is the background server running?"
                ))
            })?
            .to_string();

        let raw = std::fs::read_to_string(service_json).map_err(|e| {
            Error::Config(format!(
                "cannot read {}: {e}; the service password is required for the API",
                service_json.display()
            ))
        })?;
        let parsed: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
            Error::Config(format!("{} does not parse: {e}", service_json.display()))
        })?;
        let password = parsed
            .get("password")
            .and_then(|p| p.as_str())
            .ok_or_else(|| {
                Error::Config(format!(
                    "{} has no `password` field",
                    service_json.display()
                ))
            })?
            .to_string();

        Self::attach(url, password)
    }

    /// Basic auth header value (`opencode:<password>`).
    fn auth_header(&self) -> String {
        use base64::Engine as _;
        let raw = format!("opencode:{}", self.password);
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw)
        )
    }

    /// GET a JSON path; returns the parsed envelope with `data` unwrapped.
    async fn get_json(&self, path: &str) -> Result<serde_json::Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| Error::Session(format!("GET {path} failed: {e}")))?;
        Self::check(resp, path).await
    }

    /// POST a JSON body; returns the parsed envelope with `data` unwrapped.
    async fn post_json(
        &self,
        path: &str,
        body: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&body)
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| Error::Session(format!("POST {path} failed: {e}")))?;
        Self::check(resp, path).await
    }

    async fn check(resp: reqwest::Response, path: &str) -> Result<serde_json::Value, Error> {
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Session(format!(
                "GET/POST {path}: unauthorized; the service password in \
                 ~/.config/opencode/service.json does not match the running server"
            )));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Session(format!(
                "{path}: HTTP {status}: {}",
                body.chars().take(300).collect::<String>()
            )));
        }
        resp.json()
            .await
            .map(unwrap_data)
            .map_err(|e| Error::Session(format!("{path}: invalid JSON response: {e}")))
    }

    fn directory_query(directory: Option<&str>) -> String {
        match directory {
            Some(d) => format!("?directory={}", urlencoding_lite(d)),
            None => String::new(),
        }
    }

    /// All sessions on the server.
    ///
    /// # Errors
    ///
    /// [`Error::Session`] on HTTP or parse failures.
    pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>, Error> {
        let v = self.get_json("/api/session").await?;
        let Some(arr) = v.as_array().cloned() else {
            return Err(Error::Session(format!(
                "list_sessions: expected an array, got {}",
                short_kind(&v)
            )));
        };
        Ok(arr
            .into_iter()
            .filter_map(|s| serde_json::from_value::<WireSession>(s).ok())
            .map(SessionSummary::from)
            .collect())
    }

    /// One session by id, if it exists.
    ///
    /// # Errors
    ///
    /// [`Error::Session`] on HTTP or parse failures.
    pub async fn session(&self, id: &str) -> Result<Option<SessionSummary>, Error> {
        let v = self.get_json(&format!("/api/session/{id}")).await?;
        if v.is_null() {
            return Ok(None);
        }
        let wire: WireSession = serde_json::from_value(v)
            .map_err(|e| Error::Session(format!("session {id}: unexpected shape: {e}")))?;
        Ok(Some(SessionSummary::from(wire)))
    }

    /// A session's message history (text parts joined per message).
    ///
    /// # Errors
    ///
    /// [`Error::Session`] on HTTP or parse failures.
    pub async fn messages(
        &self,
        id: &str,
        directory: Option<&str>,
    ) -> Result<Vec<MessageSummary>, Error> {
        let path = format!(
            "/api/session/{id}/message{}",
            Self::directory_query(directory)
        );
        let v = self.get_json(&path).await?;
        let Some(arr) = v.as_array().cloned() else {
            return Err(Error::Session(format!(
                "messages({id}): expected an array, got {}",
                short_kind(&v)
            )));
        };
        Ok(arr
            .into_iter()
            .filter_map(|m| serde_json::from_value::<WireMessage>(m).ok())
            .map(|m| MessageSummary {
                text: joined_text(m.parts.as_ref().or(m.content.as_ref())),
                id: m.id,
                role: m.role.unwrap_or_else(|| "unknown".to_string()),
            })
            .collect())
    }

    /// Create a session; returns it.
    ///
    /// # Errors
    ///
    /// [`Error::Session`] on HTTP or parse failures.
    pub async fn create_session(
        &self,
        title: Option<&str>,
        directory: Option<&str>,
    ) -> Result<SessionSummary, Error> {
        let path = format!("/api/session{}", Self::directory_query(directory));
        let body = match title {
            Some(t) => serde_json::json!({ "title": t }),
            None => serde_json::json!({}),
        };
        let v = self.post_json(&path, body, Duration::from_secs(30)).await?;
        let wire: WireSession = serde_json::from_value(v)
            .map_err(|e| Error::Session(format!("create_session: unexpected shape: {e}")))?;
        Ok(SessionSummary::from(wire))
    }

    /// Prompt a session and wait for the assistant's reply.
    ///
    /// Generation can take minutes; the per-request timeout is generous.
    ///
    /// # Errors
    ///
    /// [`Error::Session`] on HTTP or parse failures.
    pub async fn prompt(
        &self,
        id: &str,
        text: &str,
        directory: Option<&str>,
        agent: Option<&str>,
        model: Option<ModelRef>,
    ) -> Result<MessageSummary, Error> {
        let path = format!(
            "/api/session/{id}/message{}",
            Self::directory_query(directory)
        );
        let mut body = serde_json::json!({
            "parts": [{ "type": "text", "text": text }]
        });
        if let Some(agent) = agent {
            body["agent"] = serde_json::json!(agent);
        }
        if let Some(m) = model {
            body["model"] = serde_json::json!({
                "providerID": m.provider_id,
                "modelID": m.model_id
            });
        }
        let v = self
            .post_json(&path, body, Duration::from_secs(600))
            .await?;
        // 200 shape: {info: message, parts: [...]}.
        let text_out = joined_text(v.get("parts"));
        let id_out = v
            .get("info")
            .and_then(|i| i.get("id"))
            .and_then(|i| i.as_str())
            .unwrap_or("unknown")
            .to_string();
        Ok(MessageSummary {
            id: id_out,
            role: "assistant".to_string(),
            text: text_out,
        })
    }

    /// Abort a busy session.
    ///
    /// # Errors
    ///
    /// [`Error::Session`] on HTTP failures.
    pub async fn abort(&self, id: &str, directory: Option<&str>) -> Result<(), Error> {
        let path = format!(
            "/api/session/{id}/abort{}",
            Self::directory_query(directory)
        );
        self.post_json(&path, serde_json::json!({}), Duration::from_secs(10))
            .await?;
        Ok(())
    }

    /// Subscribe to the server's live event stream (`GET /api/event`).
    ///
    /// The stream stays open until the server closes it or the request
    /// fails; frames without a parseable JSON payload are skipped. On a
    /// stream error, drop the stream and call again — the server treats
    /// each request as a fresh subscription.
    ///
    /// # Errors
    ///
    /// [`Error::Session`] when the request cannot be made or is rejected.
    pub async fn events(
        &self,
    ) -> Result<impl futures::Stream<Item = Result<OpenCodeEvent, Error>>, Error> {
        use futures::StreamExt;

        let url = format!("{}/api/event", self.base_url);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "text/event-stream")
            .timeout(Duration::from_secs(u64::from(u32::MAX)))
            .send()
            .await
            .map_err(|e| Error::Session(format!("GET /api/event failed: {e}")))?;
        let resp = Self::check_status_only(resp, "/api/event").await?;
        Ok(resp
            .bytes_stream()
            .scan(SseSplitter::default(), |st, chunk| {
                let frames = match chunk {
                    Ok(bytes) => {
                        st.push(&bytes);
                        st.drain_frames()
                    },
                    Err(e) => vec![Err(Error::Session(format!("event stream error: {e}")))],
                };
                futures::future::ready(Some(futures::stream::iter(frames)))
            })
            .flatten()
            .filter_map(|res| {
                futures::future::ready(match res {
                    Err(e) => Some(Err(e)),
                    Ok(frame) => parse_sse_frame(&frame)
                        .and_then(|payload| OpenCodeEvent::parse(&payload).map(Ok)),
                })
            }))
    }

    /// Status check without JSON parsing (SSE bodies are not JSON).
    async fn check_status_only(
        resp: reqwest::Response,
        path: &str,
    ) -> Result<reqwest::Response, Error> {
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Session(format!(
                "{path}: unauthorized; the service password in \
                 ~/.config/opencode/service.json does not match the running server"
            )));
        }
        if !status.is_success() {
            return Err(Error::Session(format!("{path}: HTTP {status}")));
        }
        Ok(resp)
    }
}

/// Splits an SSE byte stream into blank-line-separated frames across
/// chunk boundaries. Buffers at most [`SseSplitter::MAX_BUFFER`] bytes to
/// bound memory on hostile streams.
#[derive(Default)]
struct SseSplitter {
    buf: Vec<u8>,
}

impl SseSplitter {
    const MAX_BUFFER: usize = 1 << 20;

    fn push(&mut self, bytes: &[u8]) {
        if self.buf.len() + bytes.len() <= Self::MAX_BUFFER {
            self.buf.extend_from_slice(bytes);
        } else {
            // Oversized frame: drop it rather than grow unbounded.
            self.buf.clear();
        }
    }

    fn drain_frames(&mut self) -> Vec<Result<String, Error>> {
        // Frames end at a blank line (`\n\n`). CRLF servers leave a
        // trailing `\r` on the frame; `lines()` in `parse_sse_frame`
        // strips it, so only the LF LF boundary needs handling here.
        let mut frames = Vec::new();
        while let Some(pos) = self.buf.windows(2).position(|w| w == b"\n\n") {
            let frame = String::from_utf8_lossy(&self.buf[..pos]).into_owned();
            self.buf.drain(..pos + 2);
            frames.push(Ok(frame));
        }
        frames
    }
}

/// Percent-encode only what a query value must not contain.
fn urlencoding_lite(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            },
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session_json() -> serde_json::Value {
        serde_json::json!({
            "id": "ses_e6d0ac112ffe4ojI0s10vauY5g",
            "projectID": "c04e5b15553eb3f04e2e476b302c6cdaf3915c2c",
            "agent": "build",
            "model": {"id": "glm-5.3-flash", "providerID": "zai-coding-plan", "variant": "default"},
            "cost": 0,
            "tokens": {"input": 33525218, "output": 484069, "reasoning": 341354,
                       "cache": {"read": 635728448, "write": 0}},
            "outcome": "failed",
            "time": {"created": 1793466928678u64, "updated": 1794632790341u64,
                     "busy": 1794632634810u64, "idle": 1794627325097u64,
                     "viewed": 1794627325097u64},
            "title": "Pull and analyze repository changes",
            "location": {"directory": "/home/wyatt/dev/src/github.com/WyattAu/clawdius"},
            "revisions": [], "version": "2.3.0", "parentID": null,
            "shareUrl": null, "summary": {}
        })
    }

    #[test]
    fn session_summary_parses_live_shape() {
        let wire: WireSession = serde_json::from_value(sample_session_json()).expect("parse");
        let s = SessionSummary::from(wire);
        assert_eq!(s.id, "ses_e6d0ac112ffe4ojI0s10vauY5g");
        assert_eq!(s.outcome.as_deref(), Some("failed"));
        assert_eq!(s.provider_id.as_deref(), Some("zai-coding-plan"));
        assert_eq!(s.model_id.as_deref(), Some("glm-5.3-flash"));
        assert_eq!(s.input_tokens, 33_525_218);
        assert_eq!(s.cache_read_tokens, 635_728_448);
        assert_eq!(s.updated_ms, Some(1_794_632_790_341));
        assert_eq!(
            s.directory.as_deref(),
            Some("/home/wyatt/dev/src/github.com/WyattAu/clawdius")
        );
    }

    #[test]
    fn session_summary_tolerates_missing_fields() {
        let wire: WireSession =
            serde_json::from_value(serde_json::json!({ "id": "ses_x" })).expect("parse");
        let s = SessionSummary::from(wire);
        assert_eq!(s.id, "ses_x");
        assert_eq!(s.input_tokens, 0);
        assert!(s.title.is_none());
    }

    #[test]
    fn joined_text_skips_reasoning_parts() {
        let raw = serde_json::json!([
            {"type": "reasoning", "text": "thinking..."},
            {"type": "text", "text": "line one"},
            {"type": "text", "text": "line two"}
        ]);
        assert_eq!(joined_text(Some(&raw)), "line one\nline two");
        assert_eq!(joined_text(None), "");
    }

    #[test]
    fn data_envelope_is_unwrapped() {
        let env = serde_json::json!({"data": [1, 2, 3]});
        assert_eq!(unwrap_data(env), serde_json::json!([1, 2, 3]));
        let bare = serde_json::json!({"info": {"id": "m"}});
        assert_eq!(unwrap_data(bare), serde_json::json!({"info": {"id": "m"}}));
    }

    #[test]
    fn debug_redacts_password() {
        let client =
            OpenCodeClient::attach("http://127.0.0.1:49374", "hunter2-secret").expect("attach");
        let dbg = format!("{client:?}");
        assert!(
            !dbg.contains("hunter2-secret"),
            "password leaked in Debug: {dbg}"
        );
        assert!(dbg.contains("<redacted>"));
    }

    #[test]
    fn directory_query_encodes_spaces() {
        let q = OpenCodeClient::directory_query(Some("/home/wyatt/my repo"));
        assert_eq!(q, "?directory=/home/wyatt/my%20repo");
        assert_eq!(OpenCodeClient::directory_query(None), "");
    }

    #[test]
    fn attach_rejects_bad_urls() {
        assert!(OpenCodeClient::attach("not a url", "pw").is_err());
        assert!(OpenCodeClient::attach("http://127.0.0.1:1", "pw").is_ok());
    }

    /// One-shot HTTP mock: serves a canned response, records the last
    /// Authorization header. std-only (no tokio `net` feature needed).
    fn mock_server(
        status_line: &'static str,
        body: &'static str,
    ) -> (String, std::sync::Arc<std::sync::Mutex<String>>) {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;
        use std::sync::{Arc, Mutex};

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen_auth = Arc::new(Mutex::new(String::new()));
        let seen = seen_auth.clone();
        std::thread::spawn(move || {
            let Ok((sock, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(sock.try_clone().expect("clone"));
            let mut auth = String::new();
            let mut content_length = 0usize;
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if lower.starts_with("authorization:") {
                    auth = line.trim().to_string();
                }
                if let Some(v) = lower.strip_prefix("content-length:") {
                    content_length = v.trim().parse().unwrap_or(0);
                }
            }
            let mut sink = vec![0u8; content_length];
            let _ = reader.read_exact(&mut sink);
            *seen.lock().expect("lock") = auth;
            let mut sock = sock;
            let _ = write!(
                sock,
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
        });
        (format!("http://{addr}"), seen_auth)
    }

    #[tokio::test]
    async fn list_sessions_sends_basic_auth_and_parses() {
        let body = r#"{"data":[{"id":"ses_a","title":"T","outcome":"succeeded",
            "model":{"providerID":"p","modelID":"m"},
            "tokens":{"input":10,"output":2,"cache":{"read":5}}}]}"#;
        let (url, seen_auth) = mock_server("HTTP/1.1 200 OK", body);
        let client = OpenCodeClient::attach(url, "sekrit").expect("attach");
        let sessions = client.list_sessions().await.expect("list");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "ses_a");
        assert_eq!(sessions[0].output_tokens, 2);
        let auth = seen_auth.lock().expect("lock").clone();
        let expected = {
            use base64::Engine as _;
            format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode("opencode:sekrit")
            )
        };
        assert_eq!(
            auth.split_once(':')
                .map(|(k, v)| (k.to_ascii_lowercase(), v.trim_start().to_string())),
            Some(("authorization".to_string(), expected)),
            "header was: {auth}"
        );
    }

    #[tokio::test]
    async fn unauthorized_maps_to_a_readable_error() {
        let (url, _) = mock_server(
            "HTTP/1.1 401 Unauthorized",
            r#"{"_tag":"UnauthorizedError"}"#,
        );
        let client = OpenCodeClient::attach(url, "wrong").expect("attach");
        let err = client.list_sessions().await.expect_err("should 401");
        assert!(err.to_string().contains("unauthorized"), "{err}");
    }

    #[test]
    fn discover_parses_status_output_and_credentials() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin_dir = dir.path().join("bin");
        std::fs::create_dir(&bin_dir).expect("mkdir");
        let bin = bin_dir.join("opencode");
        std::fs::write(&bin, "#!/bin/sh\necho http://127.0.0.1:49374\n").expect("write bin");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        }
        let service_json = dir.path().join("service.json");
        std::fs::write(&service_json, r#"{"password":"pw-from-test"}"#).expect("write creds");

        let client = OpenCodeClient::discover_with(bin.to_str().expect("utf8"), &service_json)
            .expect("discover");
        let dbg = format!("{client:?}");
        assert!(dbg.contains("49374"));
        assert!(!dbg.contains("pw-from-test"));
    }

    #[test]
    fn discover_fails_cleanly_without_a_server() {
        let dir = tempfile::tempdir().expect("tempdir");
        // A binary that exits non-zero: no server running.
        let bin_dir = dir.path().join("bin");
        std::fs::create_dir(&bin_dir).expect("mkdir");
        let bin = bin_dir.join("opencode");
        std::fs::write(&bin, "#!/bin/sh\nexit 1\n").expect("write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        }
        let service_json = dir.path().join("service.json");
        std::fs::write(&service_json, r#"{"password":"x"}"#).expect("write");
        let err = OpenCodeClient::discover_with(bin.to_str().expect("utf8"), &service_json)
            .expect_err("should fail");
        assert!(err.to_string().contains("service start"), "{err}");
    }

    #[test]
    fn sse_frame_parses_data_lines_and_ignores_comments() {
        let frame = "event: session.idle\nid: 1\ndata: {\"type\":\"x\"}\n:keep-alive\n";
        assert_eq!(parse_sse_frame(frame), Some("{\"type\":\"x\"}".to_string()));
        assert_eq!(parse_sse_frame(": ping\n\n"), None);
        assert_eq!(parse_sse_frame("event: foo\n"), None);
    }

    #[test]
    fn sse_frame_joins_multi_line_data() {
        let frame = "data: {\"a\":\ndata: 1}\n\n";
        assert_eq!(parse_sse_frame(frame), Some("{\"a\":\n1}".to_string()));
    }

    #[test]
    fn splitter_reassembles_frames_across_chunk_boundaries() {
        let mut st = SseSplitter::default();
        assert!(st.drain_frames().is_empty(), "nothing before a boundary");
        st.push(b"data: {\"type\":\"session.id");
        assert!(st.drain_frames().is_empty());
        st.push(b"le\"} \n\ndata: {\"type\":\"other\"}\n\ntail");
        let frames: Vec<String> = st
            .drain_frames()
            .into_iter()
            .map(|r| r.expect("frame ok"))
            .collect();
        assert_eq!(
            frames,
            vec![
                "data: {\"type\":\"session.idle\"} ".to_string(),
                "data: {\"type\":\"other\"}".to_string(),
            ]
        );
    }

    #[test]
    fn splitter_caps_hostile_frames() {
        let mut st = SseSplitter::default();
        st.push(&vec![b'x'; SseSplitter::MAX_BUFFER + 1]);
        assert!(st.buf.is_empty(), "oversized buffer dropped");
    }

    #[test]
    fn events_parse_across_the_live_shapes() {
        let idle =
            OpenCodeEvent::parse(r#"{"type":"session.idle","properties":{"sessionID":"ses_a"}}"#)
                .expect("idle");
        assert_eq!(
            idle,
            OpenCodeEvent::SessionIdle {
                session_id: "ses_a".to_string()
            }
        );

        let err = OpenCodeEvent::parse(
            r#"{"type":"session.error","properties":{"sessionID":"ses_b","error":{"name":"X","message":"boom"}}}"#,
        )
        .expect("error");
        let OpenCodeEvent::SessionError { session_id, error } = err else {
            panic!("wrong variant");
        };
        assert_eq!(session_id, "ses_b");
        assert!(error.expect("error text").contains("boom"));

        let upd = OpenCodeEvent::parse(
            r#"{"type":"session.updated","properties":{"info":{"id":"ses_c"}}}"#,
        )
        .expect("updated");
        assert_eq!(
            upd,
            OpenCodeEvent::SessionUpdated {
                session_id: "ses_c".to_string()
            }
        );

        let other = OpenCodeEvent::parse(r#"{"type":"message.part.updated","properties":{}}"#)
            .expect("other");
        assert_eq!(
            other,
            OpenCodeEvent::Other {
                kind: "message.part.updated".to_string()
            }
        );

        assert!(OpenCodeEvent::parse("not json").is_none());
    }

    #[tokio::test]
    async fn events_stream_receives_parsed_events() {
        // Mock SSE server: two frames, then hold the connection open.
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            let Ok((sock, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(sock.try_clone().expect("clone"));
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut sock = sock;
            let body = "data: {\"type\":\"session.idle\",\"properties\":{\"sessionID\":\"ses_z\"}}\n\n: keep-alive\n\n";
            let _ = write!(
                sock,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
        });
        let client = OpenCodeClient::attach(format!("http://{addr}"), "pw").expect("attach");
        use futures::StreamExt;
        let mut stream = client.events().await.expect("subscribe");
        let first = stream.next().await.expect("one event").expect("ok");
        assert_eq!(
            first,
            OpenCodeEvent::SessionIdle {
                session_id: "ses_z".to_string()
            }
        );
    }

    /// Live check against the machine's real OpenCode server:
    /// `cargo test -p clawdius-core --lib fleet::opencode_client -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "requires a running opencode background server"]
    async fn live_lists_sessions() {
        let client = OpenCodeClient::discover().expect("discover");
        let sessions = client.list_sessions().await.expect("list");
        println!("live server: {} session(s)", sessions.len());
        for s in sessions.iter().take(5) {
            println!(
                "  {} | {:?} | in={} out={} | {:?}",
                s.id, s.outcome, s.input_tokens, s.output_tokens, s.title
            );
        }
        assert!(!sessions.is_empty());
        let first = sessions[0].clone();
        let one = client.session(&first.id).await.expect("detail");
        assert_eq!(one.expect("session").id, first.id);
        let msgs = client
            .messages(&first.id, first.directory.as_deref())
            .await
            .expect("messages");
        println!("  messages in first session: {}", msgs.len());
    }
}
