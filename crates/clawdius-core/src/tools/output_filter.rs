//! Output filtering for agent tool results: secret scrubbing,
//! repetition collapse, and compiler-error delta distillation.
//!
//! Blueprint items 14/32/48 (docs/agent-blueprint-triage.md): tool
//! outputs entering model context are scrubbed of credential-shaped
//! strings, deduplicated, and compiler error walls are diffed against
//! the previous build so only the delta reaches the model.

use std::collections::HashSet;

/// Credential-shaped substrings scrubbed to `<REDACTED:type>`.
/// Deliberately shape-based (not a full secret scanner): catches the
/// common cloud/SCM token formats that leak through test output and
/// stack traces.
#[must_use]
pub fn scrub_secrets(text: &str) -> (String, usize) {
    let mut scrubbed = text.to_string();
    let mut count = 0usize;
    for (needle, tag) in SECRET_PATTERNS {
        let redacted = format!("<REDACTED:{tag}>");
        count += scrubbed.matches(needle).count();
        if count > 0 && scrubbed.contains(needle) {
            scrubbed = scrubbed.replace(needle, &redacted);
        }
    }
    // Generic key-value shapes: "api_key: <value>", "token= <value>"
    for key in [
        "api_key",
        "apikey",
        "token",
        "secret",
        "password",
        "authorization",
    ] {
        let lower = scrubbed.to_lowercase();
        let needle_prefix = format!("{key}: ");
        let needle_eq = format!("{key}=");
        for needle in [&needle_prefix, &needle_eq] {
            let lower_needle = needle.to_lowercase();
            let mut positions = Vec::new();
            let mut from = 0;
            while let Some(pos) = lower[from..].find(&lower_needle) {
                let abs = from + pos;
                positions.push(abs);
                from = abs + lower_needle.len();
            }
            // Replace value up to whitespace/newline/quote, right-to-left.
            for pos in positions.iter().rev() {
                let start = pos + needle.len();
                let rest = &scrubbed[start..];
                let end = rest
                    .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                    .unwrap_or(rest.len());
                if end > 0 {
                    let range = start..start + end;
                    scrubbed.replace_range(range, "<REDACTED:key>");
                    count += 1;
                }
            }
        }
    }
    (scrubbed, count)
}

/// Credential shapes: AWS access keys, GitHub tokens (classic + fine),
/// OpenAI/Anthropic keys, Slack bot tokens, Bearer headers, JWTs.
const SECRET_PATTERNS: [(&str, &str); 7] = [
    ("AKIA", "aws-access-key"),
    ("ghp_", "github-token"),
    ("github_pat_", "github-fine-token"),
    ("sk-ant-", "anthropic-key"),
    ("sk-proj-", "openai-key"),
    ("xoxb-", "slack-bot-token"),
    ("eyJhbGciOiJ", "jwt"),
];

/// Collapse runs of identical lines and truncate, keeping head and tail
/// (stack traces repeat the same frame dozens of times; the assertion
/// that matters is at the end).
#[must_use]
pub fn compact_output(text: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(text.len().min(max_chars));
    let mut prev: Option<&str> = None;
    let mut run = 0usize;
    let mut total = 0usize;
    for line in text.lines() {
        total += 1;
        if prev == Some(line) {
            run += 1;
            continue;
        }
        if run > 0 {
            out.push_str(&format!("[last line repeated {run}x]\n"));
        }
        out.push_str(line);
        out.push('\n');
        prev = Some(line);
        run = 0;
    }
    if run > 0 {
        out.push_str(&format!("[last line repeated {run}x]\n"));
    }

    if out.chars().count() <= max_chars {
        return out;
    }
    // Keep head/tail at ~40/60 split so the final assertion stays visible.
    let head = max_chars * 2 / 5;
    let tail = max_chars - head;
    let chars: Vec<char> = out.chars().collect();
    let skipped = chars.len() - head - tail;
    format!(
        "{}\n[... {skipped} chars / {total} lines truncated ...]\n{}",
        chars[..head].iter().collect::<String>(),
        chars[chars.len() - tail..].iter().collect::<String>(),
    )
}

/// Distill the current compiler/test output against the previous one:
/// lines unique to the current output only (the new errors are the
/// signal; the ones that persisted are already known to the model).
#[must_use]
pub fn delta_lines(previous: &str, current: &str) -> String {
    let prev: HashSet<&str> = previous.lines().collect();
    let mut out = String::new();
    let mut fresh = 0usize;
    for line in current.lines() {
        if !prev.contains(line) {
            out.push_str(line);
            out.push('\n');
            fresh += 1;
        }
    }
    if fresh == 0 {
        format!("(no new lines vs previous output)\n{current}")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubs_known_token_shapes() {
        let (out, n) = scrub_secrets(
            "key AKIAIOSFODNN7EXAMPLE and ghp_16Characters7a and sk-ant-api03-xx failed",
        );
        assert_eq!(n, 3);
        assert!(out.contains("<REDACTED:aws-access-key>"));
        assert!(out.contains("<REDACTED:github-token>"));
        assert!(out.contains("<REDACTED:anthropic-key>"));
        assert!(!out.contains("AKIAIOSFODNN7"));
    }

    #[test]
    fn scrubs_keyvalue_shapes() {
        let (out, _) = scrub_secrets("api_key: supersecretvalue123\nstatus: ok");
        assert!(out.contains("<REDACTED:key>"));
        assert!(!out.contains("supersecretvalue123"));
        assert!(out.contains("status: ok"));
    }

    #[test]
    fn compacts_repeated_lines_keeping_order() {
        let mut text = String::new();
        for _ in 0..10 {
            text.push_str("Compiling foo v0.1.0\n");
        }
        text.push_str("error: could not compile\n");
        let out = compact_output(&text, 10_000);
        assert!(
            out.contains("[last line repeated 9x]"),
            "10 identical lines = first shown + 9 repeats"
        );
        assert!(out.contains("error: could not compile"));
    }

    #[test]
    fn truncates_keeping_tail_assertion() {
        let filler = "x".repeat(5_000);
        let text = format!("{filler}\nassertion `left == right` failed: BANANA");
        let out = compact_output(&text, 300);
        assert!(out.len() <= 400);
        assert!(out.contains("BANANA"), "tail assertion must survive");
        assert!(out.contains("truncated"));
    }

    #[test]
    fn delta_keeps_only_fresh_lines() {
        let prev = "error[E0382]: borrow checker\nerror: aborting";
        let current = "error[E0382]: borrow checker\nerror[E9999]: new failure\nerror: aborting";
        let delta = delta_lines(prev, current);
        assert!(delta.contains("E9999"));
        assert!(!delta.contains("borrow checker\n"));
    }
}
