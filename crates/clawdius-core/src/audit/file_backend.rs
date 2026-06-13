use super::logger::{AuditBackend, AuditEntry};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct FileBackend {
    directory: PathBuf,
    max_file_size_bytes: u64,
    max_files: usize,
}

impl FileBackend {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            max_file_size_bytes: 50 * 1024 * 1024,
            max_files: 100,
        }
    }

    fn current_file_path(&self) -> PathBuf {
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.directory.join(format!("audit-{}.jsonl", date))
    }

    fn should_rotate(&self, path: &Path) -> bool {
        fs::metadata(path)
            .map(|m| m.len() >= self.max_file_size_bytes)
            .unwrap_or(false)
    }

    fn rotate_if_needed(&self) -> Result<()> {
        let current = self.current_file_path();
        if self.should_rotate(&current) {
            let ts = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
            let rotated = self.directory.join(format!("audit-{}.jsonl.rotated", ts));
            fs::rename(&current, &rotated)?;
            self.cleanup_old_files()?;
        }
        Ok(())
    }

    fn cleanup_old_files(&self) -> Result<()> {
        let mut files: Vec<_> = fs::read_dir(&self.directory)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().and_then(|x| x.to_str()) == Some("jsonl")
                    || e.file_name().to_string_lossy().contains(".rotated")
            })
            .collect();

        if files.len() > self.max_files {
            files.sort_by_key(|e| e.file_name());
            let to_delete = files.len() - self.max_files;
            for f in files.iter().take(to_delete) {
                let _ = fs::remove_file(f.path());
            }
        }
        Ok(())
    }
}

#[async_trait]
impl AuditBackend for FileBackend {
    async fn write(&self, entry: &AuditEntry) -> Result<()> {
        let _ = fs::create_dir_all(&self.directory);
        self.rotate_if_needed()?;

        let path = self.current_file_path();
        let line = serde_json::to_string(entry)? + "\n";

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("Failed to open audit file: {}", path.display()))?;

        file.write_all(line.as_bytes())?;
        file.flush()?;
        Ok(())
    }

    async fn query(&self, event_type: Option<&str>, limit: usize) -> Result<Vec<AuditEntry>> {
        let mut results = Vec::new();

        if !self.directory.exists() {
            return Ok(results);
        }

        let mut files: Vec<_> = fs::read_dir(&self.directory)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jsonl"))
            .collect();

        files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

        for file_entry in files {
            let content = fs::read_to_string(file_entry.path())?;
            for line in content.lines().rev() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(entry) = serde_json::from_str::<AuditEntry>(line) {
                    if event_type.map_or(true, |t| entry.event_type == t) {
                        results.push(entry);
                        if results.len() >= limit {
                            results.reverse();
                            return Ok(results);
                        }
                    }
                }
            }
        }

        results.reverse();
        Ok(results)
    }

    async fn delete_before(&self, timestamp: u64) -> Result<usize> {
        let mut deleted = 0;

        if !self.directory.exists() {
            return Ok(deleted);
        }

        let entries = fs::read_dir(&self.directory)?;

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("jsonl") {
                continue;
            }

            let content = fs::read_to_string(&path).unwrap_or_default();
            let all_older = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<AuditEntry>(l).ok())
                .all(|e| e.timestamp < timestamp);

            if all_older && content.lines().any(|l| !l.trim().is_empty()) {
                deleted += content.lines().filter(|l| !l.trim().is_empty()).count();
                fs::remove_file(&path)?;
            }
        }

        Ok(deleted)
    }

    fn backend_name(&self) -> &'static str {
        "file"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_entry(event_type: &str, ts: u64) -> AuditEntry {
        AuditEntry {
            timestamp: ts,
            event_type: event_type.to_string(),
            user_id: None,
            session_id: None,
            action: "test".to_string(),
            resource: None,
            details: serde_json::json!({}),
            ip_address: None,
            user_agent: None,
        }
    }

    #[tokio::test]
    async fn test_file_write_and_query() {
        let dir = tempfile::tempdir().unwrap();
        let backend = FileBackend::new(dir.path());

        backend.write(&sample_entry("auth", 100)).await.unwrap();
        backend.write(&sample_entry("auth", 200)).await.unwrap();
        backend.write(&sample_entry("llm", 300)).await.unwrap();

        let all = backend.query(None, 10).await.unwrap();
        assert_eq!(all.len(), 3);

        let auth = backend.query(Some("auth"), 10).await.unwrap();
        assert_eq!(auth.len(), 2);
    }

    #[tokio::test]
    async fn test_file_query_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let backend = FileBackend::new(dir.path());

        let results = backend.query(None, 10).await.unwrap();
        assert!(results.is_empty());
    }
}
