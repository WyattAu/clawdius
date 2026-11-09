//! VCR capture/replay for deterministic HTTP in agent runs
//! (blueprint item 30 / convergent core).
//!
//! Cassette = JSONL of `{request_url, request_body, response_status,
//! response_body}` entries. Replay mode answers from the cassette with
//! zero network; record mode performs real requests via reqwest and
//! appends entries. Body matching is exact-request-URL + optional
//! body-substring.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// One recorded HTTP interaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CassetteEntry {
    /// Request URL.
    pub url: String,
    /// Request body (may be empty for GET).
    #[serde(default)]
    pub body: String,
    /// HTTP status returned.
    pub status: u16,
    /// Response body.
    pub response_body: String,
}

/// Load all entries from a cassette file; empty when absent.
#[must_use]
pub fn load_cassette(path: &Path) -> Vec<CassetteEntry> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect()
}

/// Append one entry to a cassette atomically (single-line JSONL).
///
/// # Errors
///
/// IO/serialization failures propagate.
pub fn record_entry(path: &Path, entry: &CassetteEntry) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(
        file,
        "{}",
        serde_json::to_string(entry).map_err(std::io::Error::other)?
    )
}

/// Find a recorded response for a URL (last matching entry wins).
#[must_use]
pub fn replay_for(
    cassette: &[CassetteEntry],
    url: &str,
    body_substring: Option<&str>,
) -> Option<(u16, String)> {
    cassette
        .iter()
        .rev()
        .find(|e| e.url == url && body_substring.is_none_or(|needle| e.body.contains(needle)))
        .map(|e| (e.status, e.response_body.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cassette_roundtrip_and_replay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cassette.jsonl");
        assert!(load_cassette(&path).is_empty());

        record_entry(
            &path,
            &CassetteEntry {
                url: "https://api.example.com/v1/data?x=1".into(),
                body: String::new(),
                status: 200,
                response_body: r#"{"result":"ok"}"#.into(),
            },
        )
        .unwrap();
        record_entry(
            &path,
            &CassetteEntry {
                url: "https://api.example.com/v1/data?x=2".into(),
                body: r#"{"q":"later"}"#.into(),
                status: 201,
                response_body: r#"{"created":true}"#.into(),
            },
        )
        .unwrap();

        let cassette = load_cassette(&path);
        assert_eq!(cassette.len(), 2);

        let (status, body) =
            replay_for(&cassette, "https://api.example.com/v1/data?x=1", None).unwrap();
        assert_eq!(status, 200);
        assert!(body.contains("ok"));
    }

    #[test]
    fn replay_last_match_wins_with_body_filter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        for body in [r#"{"q":"a"}"#, r#"{"q":"b"}"#] {
            record_entry(
                &path,
                &CassetteEntry {
                    url: "https://x.test/search".into(),
                    body: body.into(),
                    status: 200,
                    response_body: body.into(),
                },
            )
            .unwrap();
        }
        let cassette = load_cassette(&path);
        let (_, resp) = replay_for(&cassette, "https://x.test/search", Some(r#""q":"b""#)).unwrap();
        assert!(resp.contains('b'));
    }

    #[test]
    fn missing_url_yields_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        assert!(replay_for(&load_cassette(&path), "https://nope.test", None).is_none());
    }
}
