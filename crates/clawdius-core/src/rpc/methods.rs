//! RPC method definitions

use serde::{Deserialize, Serialize};

/// RPC methods exposed by Clawdius
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    // === Session Management ===
    /// Create a new session
    SessionCreate,
    /// Load a session by ID
    SessionLoad,
    /// Save current session
    SessionSave,
    /// List all sessions
    SessionList,
    /// Delete a session
    SessionDelete,

    // === Chat Operations ===
    /// Send a chat message
    ChatSend,
    /// Stream chat response
    ChatStream,
    /// Cancel current chat
    ChatCancel,

    // === Context Management ===
    /// Add context item
    ContextAdd,
    /// Remove context item
    ContextRemove,
    /// List context items
    ContextList,
    /// Compact context
    ContextCompact,

    // === File Operations ===
    /// Read a file
    FileRead,
    /// Write a file
    FileWrite,
    /// Edit a file
    FileEdit,
    /// Get file diff
    FileDiff,

    // === Tools ===
    /// List available tools
    ToolList,
    /// Execute a tool
    ToolExecute,

    // === State Management ===
    /// Get current state
    StateGet,
    /// Create checkpoint
    StateCheckpoint,
    /// Restore to checkpoint
    StateRestore,
    /// List checkpoints
    StateList,

    // === Browser ===
    /// Navigate browser
    BrowserNavigate,
    /// Click element
    BrowserClick,
    /// Type text
    BrowserType,
    /// Take screenshot
    BrowserScreenshot,
    /// Evaluate JavaScript
    BrowserEvaluate,
    /// Close browser
    BrowserClose,

    // === Git ===
    /// Get git status
    GitStatus,
    /// Get git diff
    GitDiff,
    /// Get git log
    GitLog,

    // === Completion ===
    /// Get inline completion
    CompletionInline,

    // === Configuration ===
    /// Get configuration
    ConfigGet,
    /// Set configuration
    ConfigSet,

    // === System ===
    /// Shutdown server
    Shutdown,
}

impl Method {
    /// Parse method from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            // Session
            "session/create" | "sessionCreate" => Some(Self::SessionCreate),
            "session/load" | "sessionLoad" => Some(Self::SessionLoad),
            "session/save" | "sessionSave" => Some(Self::SessionSave),
            "session/list" | "sessionList" => Some(Self::SessionList),
            "session/delete" | "sessionDelete" => Some(Self::SessionDelete),

            // Chat
            "chat/send" | "chatSend" => Some(Self::ChatSend),
            "chat/stream" | "chatStream" => Some(Self::ChatStream),
            "chat/cancel" | "chatCancel" => Some(Self::ChatCancel),

            // Context
            "context/add" | "contextAdd" => Some(Self::ContextAdd),
            "context/remove" | "contextRemove" => Some(Self::ContextRemove),
            "context/list" | "contextList" => Some(Self::ContextList),
            "context/compact" | "contextCompact" => Some(Self::ContextCompact),

            // File
            "file/read" | "fileRead" => Some(Self::FileRead),
            "file/write" | "fileWrite" => Some(Self::FileWrite),
            "file/edit" | "fileEdit" => Some(Self::FileEdit),
            "file/diff" | "fileDiff" => Some(Self::FileDiff),

            // Tools
            "tool/list" | "toolList" => Some(Self::ToolList),
            "tool/execute" | "toolExecute" => Some(Self::ToolExecute),

            // State
            "state/get" | "stateGet" => Some(Self::StateGet),
            "state/checkpoint" | "stateCheckpoint" => Some(Self::StateCheckpoint),
            "state/restore" | "stateRestore" => Some(Self::StateRestore),
            "state/list" | "stateList" => Some(Self::StateList),

            // Browser
            "browser/navigate" | "browserNavigate" => Some(Self::BrowserNavigate),
            "browser/click" | "browserClick" => Some(Self::BrowserClick),
            "browser/type" | "browserType" => Some(Self::BrowserType),
            "browser/screenshot" | "browserScreenshot" => Some(Self::BrowserScreenshot),
            "browser/evaluate" | "browserEvaluate" => Some(Self::BrowserEvaluate),
            "browser/close" | "browserClose" => Some(Self::BrowserClose),

            // Git
            "git/status" | "gitStatus" => Some(Self::GitStatus),
            "git/diff" | "gitDiff" => Some(Self::GitDiff),
            "git/log" | "gitLog" => Some(Self::GitLog),

            // Completion
            "completion/inline" | "completionInline" => Some(Self::CompletionInline),

            // Config
            "config/get" | "configGet" => Some(Self::ConfigGet),
            "config/set" | "configSet" => Some(Self::ConfigSet),

            // System
            "shutdown" => Some(Self::Shutdown),

            _ => None,
        }
    }

    /// Convert to method string (slash format)
    pub fn to_method(&self) -> &'static str {
        match self {
            Self::SessionCreate => "session/create",
            Self::SessionLoad => "session/load",
            Self::SessionSave => "session/save",
            Self::SessionList => "session/list",
            Self::SessionDelete => "session/delete",
            Self::ChatSend => "chat/send",
            Self::ChatStream => "chat/stream",
            Self::ChatCancel => "chat/cancel",
            Self::ContextAdd => "context/add",
            Self::ContextRemove => "context/remove",
            Self::ContextList => "context/list",
            Self::ContextCompact => "context/compact",
            Self::FileRead => "file/read",
            Self::FileWrite => "file/write",
            Self::FileEdit => "file/edit",
            Self::FileDiff => "file/diff",
            Self::ToolList => "tool/list",
            Self::ToolExecute => "tool/execute",
            Self::StateGet => "state/get",
            Self::StateCheckpoint => "state/checkpoint",
            Self::StateRestore => "state/restore",
            Self::StateList => "state/list",
            Self::BrowserNavigate => "browser/navigate",
            Self::BrowserClick => "browser/click",
            Self::BrowserType => "browser/type",
            Self::BrowserScreenshot => "browser/screenshot",
            Self::BrowserEvaluate => "browser/evaluate",
            Self::BrowserClose => "browser/close",
            Self::GitStatus => "git/status",
            Self::GitDiff => "git/diff",
            Self::GitLog => "git/log",
            Self::CompletionInline => "completion/inline",
            Self::ConfigGet => "config/get",
            Self::ConfigSet => "config/set",
            Self::Shutdown => "shutdown",
        }
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_method())
    }
}

impl std::str::FromStr for Method {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s).ok_or_else(|| format!("Unknown method: {}", s))
    }
}
