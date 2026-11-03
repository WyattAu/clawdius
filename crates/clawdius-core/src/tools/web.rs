//! Web fetch tool: pull a URL's text content for the agent.
//!
//! Claude Code parity row (`WebFetch`). Guardrails: http/https only, and
//! loopback/private/link-local targets are refused so a prompt-injected
//! fetch cannot probe the host network. Body size is capped; HTML is not
//! converted to markdown here (the compressor lives in
//! [`crate::tools::html_compressor`] and can be layered by callers).

use std::net::IpAddr;
use std::time::Duration;

/// Hard cap on the returned body (chars).
pub const FETCH_MAX_CHARS: usize = 20_000;
/// Hard cap on the downloaded body (bytes) before truncation.
const FETCH_MAX_BYTES: usize = 2 * 1024 * 1024;

/// Fetch errors beyond transport: scheme and target-address policy.
#[derive(Debug)]
pub enum FetchError {
    /// Non-http(s) scheme.
    UnsupportedScheme(String),
    /// The host resolved to a loopback/private/link-local address.
    ForbiddenTarget(String),
    /// Transport or body error.
    Transport(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedScheme(s) => write!(f, "unsupported scheme: {s}"),
            Self::ForbiddenTarget(h) => write!(f, "refused to fetch internal target: {h}"),
            Self::Transport(e) => write!(f, "fetch failed: {e}"),
        }
    }
}

/// Decide whether `host` is a target the agent must not fetch (loopback,
/// RFC1918, link-local, `.local`, `.internal`, plain hostnames without a
/// dot). Returns `Err(reason)` when forbidden.
pub fn check_target(host: &str) -> Result<(), FetchError> {
    let host = host.trim();
    let lowered = host.to_lowercase();
    if lowered == "localhost" || lowered.ends_with(".local") || lowered.ends_with(".internal") {
        return Err(FetchError::ForbiddenTarget(host.to_string()));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            return Err(FetchError::ForbiddenTarget(host.to_string()));
        }
    }
    // Bare single-label hostnames (no dot) resolve unpredictably; refuse.
    if !host.contains('.') && !host.parse::<IpAddr>().is_ok() {
        return Err(FetchError::ForbiddenTarget(host.to_string()));
    }
    Ok(())
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.octets()[0] == 100 && v4.octets()[1] == 64 // RFC 6598 CGNAT
        },
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unicast_link_local()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
        },
    }
}

/// Fetch `url` and return up to [`FETCH_MAX_CHARS`] chars of body text.
///
/// # Errors
///
/// Returns [`FetchError`] for policy refusals or transport failures.
pub async fn fetch_text(url: &str) -> Result<String, FetchError> {
    let parsed = url::Url::parse(url).map_err(|e| FetchError::Transport(e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => {},
        other => return Err(FetchError::UnsupportedScheme(other.to_string())),
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| FetchError::Transport("url has no host".to_string()))?;
    // Strip brackets from IPv6 literals before policy checks.
    check_target(host.trim_start_matches('[').trim_end_matches(']'))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| FetchError::Transport(e.to_string()))?;
    let response = client
        .get(url)
        .header("user-agent", "clawdius-fetch/1.0")
        .send()
        .await
        .map_err(|e| FetchError::Transport(e.to_string()))?;
    if !response.status().is_success() {
        return Err(FetchError::Transport(format!(
            "HTTP {} from {url}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| FetchError::Transport(e.to_string()))?;
    let capped = &bytes[..bytes.len().min(FETCH_MAX_BYTES)];
    let text = String::from_utf8_lossy(capped);
    Ok(text.chars().take(FETCH_MAX_CHARS).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_refuses_internal_targets() {
        assert!(check_target("localhost").is_err());
        assert!(check_target("printer.local").is_err());
        assert!(check_target("db.internal").is_err());
        assert!(check_target("127.0.0.1").is_err());
        assert!(check_target("10.0.0.5").is_err());
        assert!(check_target("192.168.1.1").is_err());
        assert!(check_target("172.16.0.9").is_err());
        assert!(check_target("169.254.1.1").is_err());
        assert!(check_target("::1").is_err());
        assert!(check_target("fe80::1").is_err());
        assert!(check_target("fd00::5").is_err());
        assert!(check_target("metadata").is_err(), "single-label refused");
    }

    #[test]
    fn policy_allows_public_targets() {
        assert!(check_target("example.com").is_ok());
        assert!(check_target("api.z.ai").is_ok());
        assert!(check_target("142.250.4.100").is_ok());
    }

    #[test]
    fn fetch_refuses_non_http_schemes() {
        // file:// is refused before any filesystem or network access.
        if let Err(FetchError::UnsupportedScheme(s)) = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(fetch_text("file:///etc/passwd"))
        {
            assert_eq!(s, "file");
        } else {
            panic!("file:// must be refused");
        }
    }
}
