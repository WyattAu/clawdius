//! Streaming output for real-time events

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

use super::OutputFormat;

/// Stream event types for real-time output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Session started
    Start {
        /// Session ID
        session_id: String,
        /// Model being used
        model: Option<String>,
        /// Timestamp
        timestamp: DateTime<Utc>,
    },

    /// Token generated (streaming)
    Token {
        /// Token content
        content: String,
    },

    /// Thinking/reasoning token
    Thinking {
        /// Thinking content
        content: String,
    },

    /// Tool call started
    ToolCall {
        /// Tool name
        name: String,
        /// Tool arguments
        arguments: serde_json::Value,
    },

    /// Tool call completed
    ToolResult {
        /// Tool name
        name: String,
        /// Tool result
        result: serde_json::Value,
        /// Success status
        success: bool,
    },

    /// File changed
    FileChange {
        /// File path
        path: String,
        /// Change type
        change_type: ChangeType,
    },

    /// Progress update
    Progress {
        /// Progress message
        message: String,
        /// Current step
        current: usize,
        /// Total steps
        total: usize,
    },

    /// Response completed
    Complete {
        /// Token usage
        usage: TokenUsageFinal,
        /// Duration in milliseconds
        duration_ms: u64,
    },

    /// Error occurred
    Error {
        /// Error message
        message: String,
        /// Error code
        code: String,
        /// Whether the error is recoverable
        recoverable: bool,
    },

    /// Checkpoint created
    Checkpoint {
        /// Checkpoint ID
        id: String,
        /// Description
        description: Option<String>,
    },

    /// Context added
    ContextAdded {
        /// Context type
        context_type: String,
        /// Source
        source: String,
        /// Token count
        tokens: usize,
    },
}

/// Change type for file changes
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    /// File created
    Created,
    /// File modified
    Modified,
    /// File deleted
    Deleted,
    /// File renamed
    Renamed,
}

/// Final token usage
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsageFinal {
    /// Input tokens
    pub input: usize,
    /// Output tokens
    pub output: usize,
    /// Total tokens
    pub total: usize,
}

impl StreamEvent {
    /// Create a start event
    pub fn start(session_id: impl Into<String>, model: Option<String>) -> Self {
        Self::Start {
            session_id: session_id.into(),
            model,
            timestamp: Utc::now(),
        }
    }

    /// Create a token event
    pub fn token(content: impl Into<String>) -> Self {
        Self::Token {
            content: content.into(),
        }
    }

    /// Create a thinking event
    pub fn thinking(content: impl Into<String>) -> Self {
        Self::Thinking {
            content: content.into(),
        }
    }

    /// Create a tool call event
    pub fn tool_call(name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self::ToolCall {
            name: name.into(),
            arguments,
        }
    }

    /// Create a tool result event
    pub fn tool_result(name: impl Into<String>, result: serde_json::Value, success: bool) -> Self {
        Self::ToolResult {
            name: name.into(),
            result,
            success,
        }
    }

    /// Create a file change event
    pub fn file_change(path: impl Into<String>, change_type: ChangeType) -> Self {
        Self::FileChange {
            path: path.into(),
            change_type,
        }
    }

    /// Create a progress event
    pub fn progress(message: impl Into<String>, current: usize, total: usize) -> Self {
        Self::Progress {
            message: message.into(),
            current,
            total,
        }
    }

    /// Create a complete event
    #[must_use]
    pub fn complete(input: usize, output: usize, duration_ms: u64) -> Self {
        Self::Complete {
            usage: TokenUsageFinal {
                input,
                output,
                total: input + output,
            },
            duration_ms,
        }
    }

    /// Create an error event
    pub fn error(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            code: code.into(),
            recoverable: false,
        }
    }

    /// Create a recoverable error event
    pub fn recoverable_error(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            code: code.into(),
            recoverable: true,
        }
    }

    /// Convert to JSON string (newline-delimited)
    pub fn to_json_line(&self) -> crate::Result<String> {
        let mut json = serde_json::to_string(self).map_err(crate::Error::Serialization)?;
        json.push('\n');
        Ok(json)
    }
}

/// Stream writer for outputting events
pub struct StreamWriter<W: Write> {
    writer: W,
    format: OutputFormat,
}

impl<W: Write> StreamWriter<W> {
    /// Create a new stream writer
    pub fn new(writer: W, format: OutputFormat) -> Self {
        Self { writer, format }
    }

    /// Write an event
    pub fn write_event(&mut self, event: &StreamEvent) -> crate::Result<()> {
        match self.format {
            OutputFormat::StreamJson => {
                let line = event.to_json_line()?;
                self.writer.write_all(line.as_bytes())?;
                self.writer.flush()?;
            }
            OutputFormat::Json => {
                // For single JSON, accumulate and write at end
            }
            OutputFormat::Text => {
                // For text, format nicely
                let text = self.format_event_text(event);
                self.writer.write_all(text.as_bytes())?;
                self.writer.flush()?;
            }
        }
        Ok(())
    }

    fn format_event_text(&self, event: &StreamEvent) -> String {
        match event {
            StreamEvent::Start { session_id, .. } => {
                format!("Session: {session_id}\n")
            }
            StreamEvent::Token { content } => content.clone(),
            StreamEvent::Thinking { content } => {
                format!("[thinking] {content}\n")
            }
            StreamEvent::ToolCall { name, arguments } => {
                format!("\n🔧 Tool: {name} ({arguments})\n")
            }
            StreamEvent::ToolResult { name, success, .. } => {
                let status = if *success { "✓" } else { "✗" };
                format!("{status} Tool result: {name}\n")
            }
            StreamEvent::FileChange { path, change_type } => {
                let action = match change_type {
                    ChangeType::Created => "created",
                    ChangeType::Modified => "modified",
                    ChangeType::Deleted => "deleted",
                    ChangeType::Renamed => "renamed",
                };
                format!("📄 File {action}: {path}\n")
            }
            StreamEvent::Progress {
                message,
                current,
                total,
            } => {
                format!("[{current}/{total}] {message}\n")
            }
            StreamEvent::Complete { usage, duration_ms } => {
                format!(
                    "\n✓ Complete: {} tokens in {}ms\n",
                    usage.total, duration_ms
                )
            }
            StreamEvent::Error { message, code, .. } => {
                format!("✗ Error ({code}): {message}\n")
            }
            StreamEvent::Checkpoint { id, description } => {
                let desc = description.as_deref().unwrap_or("no description");
                format!("📍 Checkpoint: {id} - {desc}\n")
            }
            StreamEvent::ContextAdded {
                context_type,
                source,
                tokens,
            } => {
                format!("📎 Added {context_type} from {source} ({tokens} tokens)\n")
            }
        }
    }

    /// Flush the writer
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl StreamWriter<Vec<u8>> {
    /// Create a new in-memory stream writer
    #[must_use]
    pub fn in_memory(format: OutputFormat) -> Self {
        Self::new(Vec::new(), format)
    }

    /// Get the written content as a string
    #[must_use]
    pub fn into_string(self) -> String {
        String::from_utf8_lossy(&self.writer).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_json() {
        let event = StreamEvent::token("Hello");
        let json = event.to_json_line().unwrap();
        assert!(json.contains(r#""type":"token"#));
        assert!(json.contains(r#""content":"Hello"#));
    }

    #[test]
    fn test_stream_writer() {
        let mut writer = StreamWriter::in_memory(OutputFormat::StreamJson);

        writer
            .write_event(&StreamEvent::start("session-123", None))
            .unwrap();
        writer.write_event(&StreamEvent::token("Hello")).unwrap();
        writer
            .write_event(&StreamEvent::complete(10, 5, 1000))
            .unwrap();

        let output = writer.into_string();
        assert!(output.contains("session-123"));
        assert!(output.contains("Hello"));
    }
}
