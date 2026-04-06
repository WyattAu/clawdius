//! File operations tool

use serde::{Deserialize, Serialize};
use std::fs;

/// Validate that a path is safe (no traversal, no symlinks outside workspace)
/// Returns the canonicalized path if safe, or an error if traversal is detected.
fn validate_path(
    path: &str,
    workspace_root: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let path = std::path::Path::new(path);

    // Reject paths with null bytes
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::Normal(s) if s.to_string_lossy().contains('\0')))
    {
        return Err("Path contains null bytes".to_string());
    }

    // Canonicalize the path (resolves .., symlinks, etc.)
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // Path doesn't exist yet — build it relative to current dir
            // For new files, check the parent directory
            let parent = path.parent().unwrap_or(std::path::Path::new("."));
            match parent.canonicalize() {
                Ok(p) => p.join(path.file_name().unwrap_or(std::ffi::OsStr::new(""))),
                Err(_) => return Err(format!("Cannot resolve path: {}", path.display())),
            }
        },
    };

    // Check canonical path starts with workspace root
    let canonical_workspace = match workspace_root.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return Err(format!(
                "Cannot resolve workspace root: {}",
                workspace_root.display()
            ))
        },
    };

    if !canonical.starts_with(&canonical_workspace) {
        return Err(format!(
            "Path '{}' resolves outside workspace '{}'",
            path.display(),
            workspace_root.display()
        ));
    }

    Ok(canonical)
}

/// File read parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadParams {
    /// File path
    pub path: String,
    /// Optional offset to start reading from
    #[serde(default)]
    pub offset: Option<usize>,
    /// Optional limit on number of lines
    #[serde(default)]
    pub limit: Option<usize>,
}

/// File write parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteParams {
    /// File path
    pub path: String,
    /// Content to write
    pub content: String,
}

/// File edit parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditParams {
    /// File path
    pub path: String,
    /// Old string to find
    pub old_string: String,
    /// New string to replace with
    pub new_string: String,
    /// Replace all occurrences
    #[serde(default)]
    pub replace_all: bool,
}

/// File list parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListParams {
    /// Directory path
    pub path: String,
}

/// File tool implementation
pub struct FileTool;

impl FileTool {
    #[must_use]
    pub fn new() -> Self {
        FileTool
    }

    pub fn read(&self, params: FileReadParams) -> crate::Result<String> {
        let workspace_root = std::env::current_dir().unwrap_or_default();
        let safe_path = validate_path(&params.path, &workspace_root)
            .map_err(|e| crate::Error::Tool(format!("Path validation failed: {}", e)))?;
        let path = &safe_path;

        if !path.exists() {
            return Err(crate::Error::Tool(format!(
                "File not found: {}",
                params.path
            )));
        }

        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();

        let start = params.offset.unwrap_or(0);
        if start >= lines.len() {
            return Ok(String::new());
        }

        let end = params
            .limit
            .map_or(lines.len(), |l| (start + l).min(lines.len()));

        Ok(lines[start..end].join("\n"))
    }

    pub fn write(&self, params: FileWriteParams) -> crate::Result<()> {
        let workspace_root = std::env::current_dir().unwrap_or_default();
        let safe_path = validate_path(&params.path, &workspace_root)
            .map_err(|e| crate::Error::Tool(format!("Path validation failed: {}", e)))?;
        let path = &safe_path;

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(path, &params.content)?;
        Ok(())
    }

    pub fn edit(&self, params: FileEditParams) -> crate::Result<bool> {
        let workspace_root = std::env::current_dir().unwrap_or_default();
        let safe_path = validate_path(&params.path, &workspace_root)
            .map_err(|e| crate::Error::Tool(format!("Path validation failed: {}", e)))?;
        let path = &safe_path;

        if !path.exists() {
            return Err(crate::Error::Tool(format!(
                "File not found: {}",
                params.path
            )));
        }

        let content = fs::read_to_string(path)?;

        if !content.contains(&params.old_string) {
            return Ok(false);
        }

        let new_content = if params.replace_all {
            content.replace(&params.old_string, &params.new_string)
        } else {
            let mut replaced = false;
            let mut result = String::new();
            let mut remaining = content.as_str();

            while let Some(pos) = remaining.find(&params.old_string) {
                if replaced {
                    result.push_str(&remaining[..pos]);
                    result.push_str(&params.new_string);
                    remaining = &remaining[pos + params.old_string.len()..];
                } else {
                    result.push_str(&remaining[..pos]);
                    result.push_str(&params.new_string);
                    remaining = &remaining[pos + params.old_string.len()..];
                    #[allow(unused_assignments)]
                    {
                        replaced = true;
                    }
                    break;
                }
            }
            result.push_str(remaining);
            result
        };

        fs::write(path, new_content)?;
        Ok(true)
    }

    pub fn list(&self, params: FileListParams) -> crate::Result<Vec<String>> {
        let workspace_root = std::env::current_dir().unwrap_or_default();
        let safe_path = validate_path(&params.path, &workspace_root)
            .map_err(|e| crate::Error::Tool(format!("Path validation failed: {}", e)))?;
        let path = &safe_path;

        if !path.exists() {
            return Err(crate::Error::Tool(format!(
                "Directory not found: {}",
                params.path
            )));
        }

        if !path.is_dir() {
            return Err(crate::Error::Tool(format!(
                "Not a directory: {}",
                params.path
            )));
        }

        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push(name);
        }

        entries.sort();
        Ok(entries)
    }
}

impl Default for FileTool {
    fn default() -> Self {
        Self::new()
    }
}
