//! Loopback redirect capture for OAuth CLI flows.
//!
//! Binds `127.0.0.1:<port>`, accepts ONE request, parses the callback
//! query (`?code=...&state=...`), responds with a minimal HTML page, and
//! shuts down. Standard pattern for desktop OAuth clients (Claude Code,
//! Codex CLI do the same).

use std::io::{Read, Write};

/// Parsed callback query parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackParams {
    /// Authorization code from the provider.
    pub code: String,
    /// State echoed by the provider (CSRF binding — caller must match the
    /// sent value).
    pub state: String,
    /// Error parameter when the user denied consent.
    pub error: Option<String>,
}

/// Parse a callback URI/query string into its parameters.
#[must_use]
pub fn parse_callback(query_or_uri: &str) -> Option<CallbackParams> {
    let query = query_or_uri
        .split_once('?')
        .map(|(_, q)| q)
        .unwrap_or(query_or_uri);
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = urldecode(v);
        match k {
            "code" => code = Some(v),
            "state" => state = Some(v),
            "error" => error = Some(v),
            _ => {},
        }
    }
    code.map(|code| CallbackParams {
        code,
        state: state.unwrap_or_default(),
        error,
    })
}

fn urldecode(s: &str) -> String {
    // Percent-decode plus '+' as space (query-string convention).
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            },
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push(((h << 4) | l) as u8);
                        i += 3;
                    },
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    },
                }
            },
            b => {
                out.push(b);
                i += 1;
            },
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Bind `127.0.0.1:port` and capture the first callback request.
///
/// Waits up to `timeout_secs`. The bound socket is loopback-only and
/// closed immediately after capture — nothing else can connect to it in
/// the capture window (same-machine attacker aside, which the state
/// check covers).
///
/// # Errors
///
/// Bind/accept/read failures and timeout surface as strings.
pub async fn capture_code(
    port: u16,
    timeout_secs: u64,
) -> std::result::Result<CallbackParams, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("cannot bind loopback port {port}: {e}"))?;

    let accept = async {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| format!("accept failed: {e}"))?;
        let mut buf = [0u8; 4096];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|e| format!("read failed: {e}"))?;
        let request = String::from_utf8_lossy(&buf[..n]).into_owned();
        // First line: "GET /callback?code=..&state=.. HTTP/1.1"
        let target = request
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("")
            .to_string();
        let body = "<html><body><h1>clawdius: authorization received. \
                    You can close this tab.</h1></body></html>";
        let _ = stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await;
        Ok::<String, String>(target)
    };

    let target = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), accept)
        .await
        .map_err(|_| format!("no callback within {timeout_secs}s"))??;

    parse_callback(&target).ok_or_else(|| "callback request had no code parameter".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_callback_query() {
        let parsed = parse_callback("/callback?code=abc123&state=xyz789").unwrap();
        assert_eq!(parsed.code, "abc123");
        assert_eq!(parsed.state, "xyz789");
        assert!(parsed.error.is_none());
    }

    #[test]
    fn parses_full_uri_and_percent_encoding() {
        let parsed =
            parse_callback("http://localhost:54545/callback?code=a%20b%2Bc&state=s1&error=denied")
                .unwrap();
        assert_eq!(parsed.code, "a b+c");
        assert_eq!(parsed.error.as_deref(), Some("denied"));
    }

    #[test]
    fn missing_code_is_none() {
        assert!(parse_callback("/callback?state=only").is_none());
    }
}
