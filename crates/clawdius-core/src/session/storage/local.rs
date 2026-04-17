use super::{Vfs, VfsDirEntry, VfsError, VfsMetadata};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LocalFsBackend {
    root: PathBuf,
}

impl LocalFsBackend {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }

    fn validate_path(&self, path: &Path) -> std::result::Result<PathBuf, VfsError> {
        let resolved = self.resolve(path);

        let canonical_root = self.root.canonicalize().map_err(VfsError::Io)?;
        let canonical_path = match resolved.canonicalize() {
            Ok(p) => p,
            Err(_) => return Err(VfsError::NotFound(resolved)),
        };

        if !canonical_path.starts_with(&canonical_root) {
            return Err(VfsError::PathTraversal(canonical_path));
        }

        Ok(canonical_path)
    }

    /// Validate a path for write/create operations (path may not exist yet).
    /// Uses canonicalization when possible, lexical normalization as fallback.
    fn validate_new_path(&self, path: &Path) -> std::result::Result<PathBuf, VfsError> {
        let resolved = self.resolve(path);
        let canonical_root = self.root.canonicalize().map_err(VfsError::Io)?;

        // Try canonicalization first (handles symlinks in existing components)
        if let Ok(canonical) = resolved.canonicalize() {
            if !canonical.starts_with(&canonical_root) {
                return Err(VfsError::PathTraversal(canonical));
            }
            return Ok(canonical);
        }

        // Path doesn't exist — lexically normalize and validate
        let normalized = Self::lexical_normalize(&resolved);
        if !normalized.starts_with(&canonical_root) {
            return Err(VfsError::PathTraversal(normalized));
        }
        Ok(normalized)
    }

    /// Lexically normalize a path by resolving `.` and `..` components
    /// without touching the filesystem.
    fn lexical_normalize(path: &Path) -> PathBuf {
        let mut result = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::Prefix(p) => {
                    result.push(p.as_os_str());
                },
                std::path::Component::RootDir => {
                    result.push(component);
                },
                std::path::Component::CurDir => {},
                std::path::Component::ParentDir => {
                    result.pop();
                },
                std::path::Component::Normal(c) => result.push(c),
            }
        }
        result
    }

    fn to_vfs_metadata(meta: &fs::Metadata) -> VfsMetadata {
        VfsMetadata {
            is_file: meta.is_file(),
            is_dir: meta.is_dir(),
            is_symlink: meta.file_type().is_symlink(),
            size: meta.len(),
            modified: meta.modified().ok(),
            created: meta.created().ok(),
        }
    }
}

impl Vfs for LocalFsBackend {
    fn read_text(&self, path: &Path) -> std::result::Result<String, VfsError> {
        let resolved = self.validate_path(path)?;
        let bytes = fs::read(&resolved)?;
        String::from_utf8(bytes).map_err(|e| {
            VfsError::Other(format!(
                "File is not valid UTF-8 at {}: {e}",
                resolved.display()
            ))
        })
    }

    fn read_bytes(&self, path: &Path) -> std::result::Result<Vec<u8>, VfsError> {
        let resolved = self.validate_path(path)?;
        Ok(fs::read(&resolved)?)
    }

    fn metadata(&self, path: &Path) -> std::result::Result<VfsMetadata, VfsError> {
        let resolved = self.validate_path(path)?;
        let meta = fs::metadata(&resolved)?;
        Ok(Self::to_vfs_metadata(&meta))
    }

    fn exists(&self, path: &Path) -> bool {
        let resolved = self.resolve(path);
        resolved.exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        let resolved = self.resolve(path);
        resolved.is_dir()
    }

    fn is_file(&self, path: &Path) -> bool {
        let resolved = self.resolve(path);
        resolved.is_file()
    }

    fn canonicalize(&self, path: &Path) -> std::result::Result<PathBuf, VfsError> {
        self.validate_path(path)
    }

    fn write(&self, path: &Path, content: &[u8]) -> std::result::Result<(), VfsError> {
        let resolved = self.validate_new_path(path)?;
        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&resolved, content)?;
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> std::result::Result<(), VfsError> {
        let resolved = self.validate_new_path(path)?;
        fs::create_dir_all(&resolved)?;
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> std::result::Result<(), VfsError> {
        let resolved = self.validate_path(path)?;
        fs::remove_file(&resolved)?;
        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> std::result::Result<(), VfsError> {
        let resolved = self.validate_path(path)?;
        fs::remove_dir_all(&resolved)?;
        Ok(())
    }

    fn read_dir(&self, path: &Path) -> std::result::Result<Vec<VfsDirEntry>, VfsError> {
        let resolved = self.validate_path(path)?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&resolved)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            entries.push(VfsDirEntry {
                path: entry.path(),
                metadata: Self::to_vfs_metadata(&meta),
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(entries)
    }

    fn base_path(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_backend() -> (LocalFsBackend, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let backend = LocalFsBackend::new(tmp.path());
        (backend, tmp)
    }

    #[test]
    fn test_read_write_text() {
        let (vfs, _tmp) = make_backend();
        vfs.write_text(Path::new("hello.txt"), "hello world")
            .unwrap();
        let content = vfs.read_text(Path::new("hello.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_read_write_bytes() {
        let (vfs, _tmp) = make_backend();
        let data = vec![0u8, 1, 2, 255, 128];
        vfs.write(Path::new("binary.bin"), &data).unwrap();
        let read = vfs.read_bytes(Path::new("binary.bin")).unwrap();
        assert_eq!(read, data);
    }

    #[test]
    fn test_create_dir_all() {
        let (vfs, _tmp) = make_backend();
        vfs.create_dir_all(Path::new("a/b/c")).unwrap();
        assert!(vfs.is_dir(Path::new("a/b/c")));
    }

    #[test]
    fn test_exists() {
        let (vfs, _tmp) = make_backend();
        assert!(!vfs.exists(Path::new("missing.txt")));
        vfs.write_text(Path::new("present.txt"), "data").unwrap();
        assert!(vfs.exists(Path::new("present.txt")));
        vfs.create_dir_all(Path::new("adir")).unwrap();
        assert!(vfs.exists(Path::new("adir")));
    }

    #[test]
    fn test_is_file_is_dir() {
        let (vfs, _tmp) = make_backend();
        vfs.write_text(Path::new("file.txt"), "x").unwrap();
        vfs.create_dir_all(Path::new("dir")).unwrap();

        assert!(vfs.is_file(Path::new("file.txt")));
        assert!(!vfs.is_dir(Path::new("file.txt")));
        assert!(vfs.is_dir(Path::new("dir")));
        assert!(!vfs.is_file(Path::new("dir")));
    }

    #[test]
    fn test_remove_file() {
        let (vfs, _tmp) = make_backend();
        vfs.write_text(Path::new("to_delete.txt"), "bye").unwrap();
        assert!(vfs.exists(Path::new("to_delete.txt")));
        vfs.remove_file(Path::new("to_delete.txt")).unwrap();
        assert!(!vfs.exists(Path::new("to_delete.txt")));
    }

    #[test]
    fn test_read_dir() {
        let (vfs, _tmp) = make_backend();
        vfs.write_text(Path::new("a.txt"), "").unwrap();
        vfs.write_text(Path::new("b.txt"), "").unwrap();
        vfs.create_dir_all(Path::new("sub")).unwrap();

        let entries = vfs.read_dir(Path::new(".")).unwrap();
        assert_eq!(entries.len(), 3);
        let names: Vec<&str> = entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
        assert!(names.contains(&"sub"));
    }

    #[test]
    fn test_metadata() {
        let (vfs, _tmp) = make_backend();
        vfs.write_text(Path::new("info.txt"), "12345").unwrap();
        let meta = vfs.metadata(Path::new("info.txt")).unwrap();
        assert!(meta.is_file);
        assert!(!meta.is_dir);
        assert_eq!(meta.size, 5);
        assert!(meta.modified.is_some());
    }

    #[test]
    fn test_path_traversal_blocked() {
        let (vfs, _tmp) = make_backend();
        let result = vfs.read_text(Path::new("../../etc/passwd"));
        assert!(result.is_err());
        match result.unwrap_err() {
            VfsError::PathTraversal(_) => {},
            VfsError::NotFound(_) => {},
            other => panic!("Expected PathTraversal or NotFound, got: {other}"),
        }
    }

    #[test]
    fn test_absolute_path_outside_root_blocked() {
        let (vfs, _tmp) = make_backend();
        let result = vfs.read_text(Path::new("/etc/passwd"));
        assert!(result.is_err());
        match result.unwrap_err() {
            VfsError::PathTraversal(_) | VfsError::NotFound(_) => {},
            other => panic!("Expected PathTraversal or NotFound, got: {other}"),
        }
    }

    #[test]
    fn test_nonexistent_file_read() {
        let (vfs, _tmp) = make_backend();
        let result = vfs.read_text(Path::new("nope.txt"));
        assert!(result.is_err());
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_write_creates_parent_dirs() {
        let (vfs, _tmp) = make_backend();
        vfs.write_text(Path::new("deep/nested/file.txt"), "content")
            .unwrap();
        assert!(vfs.is_file(Path::new("deep/nested/file.txt")));
        let content = vfs.read_text(Path::new("deep/nested/file.txt")).unwrap();
        assert_eq!(content, "content");
    }

    #[test]
    fn test_remove_dir_all() {
        let (vfs, _tmp) = make_backend();
        vfs.write_text(Path::new("tree/leaf.txt"), "").unwrap();
        vfs.remove_dir_all(Path::new("tree")).unwrap();
        assert!(!vfs.exists(Path::new("tree")));
    }

    #[test]
    fn test_base_path() {
        let tmp = tempfile::tempdir().unwrap();
        let vfs = LocalFsBackend::new(tmp.path());
        assert_eq!(vfs.base_path(), tmp.path());
    }

    #[test]
    fn test_canonicalize() {
        let (vfs, _tmp) = make_backend();
        vfs.write_text(Path::new("real.txt"), "data").unwrap();
        let canon = vfs.canonicalize(Path::new("real.txt")).unwrap();
        assert!(canon.ends_with("real.txt"));
    }

    #[test]
    fn test_read_dir_empty() {
        let (vfs, _tmp) = make_backend();
        vfs.create_dir_all(Path::new("empty")).unwrap();
        let entries = vfs.read_dir(Path::new("empty")).unwrap();
        assert!(entries.is_empty());
    }

    // === Security: write() and create_dir_all() path traversal ===

    #[test]
    fn test_write_blocks_path_traversal() {
        let (vfs, _tmp) = make_backend();
        let result = vfs.write(Path::new("../../etc/evil.txt"), b"pwned");
        assert!(result.is_err());
        match result.unwrap_err() {
            VfsError::PathTraversal(_) => {},
            other => panic!("Expected PathTraversal, got: {other}"),
        }
    }

    #[test]
    fn test_write_blocks_absolute_path_outside_root() {
        let (vfs, _tmp) = make_backend();
        let result = vfs.write(Path::new("/etc/evil.txt"), b"pwned");
        assert!(result.is_err());
        match result.unwrap_err() {
            VfsError::PathTraversal(_) => {},
            other => panic!("Expected PathTraversal, got: {other}"),
        }
    }

    #[test]
    fn test_create_dir_blocks_path_traversal() {
        let (vfs, _tmp) = make_backend();
        let result = vfs.create_dir_all(Path::new("../../tmp/evil_dir"));
        assert!(result.is_err());
        match result.unwrap_err() {
            VfsError::PathTraversal(_) => {},
            other => panic!("Expected PathTraversal, got: {other}"),
        }
    }

    #[test]
    fn test_write_nested_traversal_blocked() {
        let (vfs, _tmp) = make_backend();
        // Create a legit directory first
        vfs.create_dir_all(Path::new("safe/nested")).unwrap();
        // Now try to escape via sibling traversal
        let result = vfs.write(Path::new("safe/nested/../../../etc/evil.txt"), b"pwned");
        assert!(result.is_err());
        match result.unwrap_err() {
            VfsError::PathTraversal(_) => {},
            other => panic!("Expected PathTraversal, got: {other}"),
        }
    }

    #[test]
    fn test_lexical_normalize() {
        // Basic . and .. handling
        assert_eq!(
            LocalFsBackend::lexical_normalize(Path::new("/a/b/../c")),
            PathBuf::from("/a/c")
        );
        assert_eq!(
            LocalFsBackend::lexical_normalize(Path::new("/a/./b/./c")),
            PathBuf::from("/a/b/c")
        );
        // .. at root stays at root
        assert_eq!(
            LocalFsBackend::lexical_normalize(Path::new("/../../etc")),
            PathBuf::from("/etc")
        );
        // Multiple ..
        assert_eq!(
            LocalFsBackend::lexical_normalize(Path::new("/a/b/c/../../d")),
            PathBuf::from("/a/d")
        );
    }
}
