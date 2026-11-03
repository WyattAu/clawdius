//! Session todo list with Claude Code TodoWrite semantics.
//!
//! The agent maintains a visible plan: items are `pending`,
//! `in_progress`, or `completed`. The list is replaced wholesale by
//! `todo_write` and rendered by `todo_read`; keeping exactly one item
//! `in_progress` is recommended to the model but not enforced.

use serde::{Deserialize, Serialize};

/// One plan item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    /// What to do.
    pub content: String,
    /// Lifecycle: `pending`, `in_progress`, or `completed`.
    pub status: String,
}

impl TodoItem {
    /// Create a pending item.
    #[must_use]
    pub fn pending(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            status: "pending".to_string(),
        }
    }
}

/// Normalize a status string to the three known values.
#[must_use]
pub fn normalize_status(status: &str) -> String {
    match status.trim().to_lowercase().as_str() {
        "in_progress" | "inprogress" | "in-progress" | "active" => "in_progress".to_string(),
        "completed" | "complete" | "done" | "done " => "completed".to_string(),
        _ => "pending".to_string(),
    }
}

/// Parse a `todo_write` payload: a JSON array of `{content, status}`.
///
/// Malformed entries are skipped rather than failing the whole list.
#[must_use]
pub fn parse_todos(value: &serde_json::Value) -> Vec<TodoItem> {
    let Some(entries) = value.as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|e| {
            let content = e.get("content")?.as_str()?.trim().to_string();
            if content.is_empty() {
                return None;
            }
            Some(TodoItem {
                content,
                status: normalize_status(e.get("status").and_then(|s| s.as_str()).unwrap_or("")),
            })
        })
        .collect()
}

/// Render the list the way the model sees it after `todo_read`.
#[must_use]
pub fn render_todos(todos: &[TodoItem]) -> String {
    if todos.is_empty() {
        return "No todos. Use todo_write to create a plan.".to_string();
    }
    let mut out = String::from("Todo list:\n");
    for (idx, item) in todos.iter().enumerate() {
        let marker = match item.status.as_str() {
            "in_progress" => "▶",
            "completed" => "✓",
            _ => "·",
        };
        out.push_str(&format!("  {}. [{}] {}\n", idx + 1, marker, item.content));
    }
    let in_progress = todos.iter().filter(|t| t.status == "in_progress").count();
    let done = todos.iter().filter(|t| t.status == "completed").count();
    out.push_str(&format!("  ({done} completed, {in_progress} in progress)"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_skips_malformed_entries() {
        let todos = parse_todos(&json!([
            {"content": "fix tests", "status": "in_progress"},
            {"no_content": true},
            {"content": "  ", "status": "pending"},
            {"content": "write docs"},
            {"content": "ship", "status": "DONE"}
        ]));
        assert_eq!(todos.len(), 3);
        assert_eq!(todos[0].status, "in_progress");
        assert_eq!(todos[1].status, "pending");
        assert_eq!(todos[2].status, "completed");
    }

    #[test]
    fn render_shows_progress_and_markers() {
        let todos = vec![
            TodoItem {
                content: "a".into(),
                status: "completed".into(),
            },
            TodoItem {
                content: "b".into(),
                status: "in_progress".into(),
            },
            TodoItem {
                content: "c".into(),
                status: "pending".into(),
            },
        ];
        let rendered = render_todos(&todos);
        assert!(rendered.contains("✓"));
        assert!(rendered.contains("▶"));
        assert!(rendered.contains("1 completed"));
    }

    #[test]
    fn render_empty_list_prompts_for_plan() {
        assert!(render_todos(&[]).contains("todo_write"));
    }
}
