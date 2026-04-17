use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct VfsMetadata {
    pub is_file: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
    pub created: Option<std::time::SystemTime>,
}

#[derive(Debug, thiserror::Error)]
pub enum VfsError {
    #[error("Path not found: {0}")]
    NotFound(PathBuf),
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),
    #[error("Already exists: {0}")]
    AlreadyExists(PathBuf),
    #[error("Not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("Not a file: {0}")]
    NotAFile(PathBuf),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Path traversal attempt: {0}")]
    PathTraversal(PathBuf),
    #[error("{0}")]
    Other(String),
}

impl VfsError {
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }

    #[must_use]
    pub const fn is_permission_denied(&self) -> bool {
        matches!(self, Self::PermissionDenied(_))
    }
}

#[derive(Debug, Clone)]
pub struct VfsDirEntry {
    pub path: PathBuf,
    pub metadata: VfsMetadata,
}

pub trait Vfs: Send + Sync + std::fmt::Debug {
    fn read_text(&self, path: &Path) -> std::result::Result<String, VfsError>;
    fn read_bytes(&self, path: &Path) -> std::result::Result<Vec<u8>, VfsError>;
    fn metadata(&self, path: &Path) -> std::result::Result<VfsMetadata, VfsError>;
    fn exists(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
    fn canonicalize(&self, path: &Path) -> std::result::Result<PathBuf, VfsError>;

    fn write(&self, path: &Path, content: &[u8]) -> std::result::Result<(), VfsError>;
    fn write_text(&self, path: &Path, content: &str) -> std::result::Result<(), VfsError> {
        self.write(path, content.as_bytes())
    }
    fn create_dir_all(&self, path: &Path) -> std::result::Result<(), VfsError>;
    fn remove_file(&self, path: &Path) -> std::result::Result<(), VfsError>;
    fn remove_dir_all(&self, path: &Path) -> std::result::Result<(), VfsError>;

    fn read_dir(&self, path: &Path) -> std::result::Result<Vec<VfsDirEntry>, VfsError>;

    fn base_path(&self) -> &Path;
}

/// Recursively walk a directory tree using a VFS backend.
/// `filter_entry` is called for each entry; return `false` to skip a directory
/// (and all its children) entirely.
pub fn walk_dir(
    vfs: &dyn Vfs,
    root: &Path,
    filter_entry: Option<&dyn Fn(&VfsDirEntry) -> bool>,
) -> std::result::Result<Vec<VfsDirEntry>, VfsError> {
    let mut results = Vec::new();
    walk_dir_recursive(vfs, root, filter_entry, &mut results)?;
    Ok(results)
}

fn walk_dir_recursive(
    vfs: &dyn Vfs,
    dir: &Path,
    filter_entry: Option<&dyn Fn(&VfsDirEntry) -> bool>,
    out: &mut Vec<VfsDirEntry>,
) -> std::result::Result<(), VfsError> {
    let entries = vfs.read_dir(dir)?;
    for entry in &entries {
        if let Some(filter) = filter_entry {
            if !filter(entry) {
                // If it's a directory we're skipping, don't recurse.
                continue;
            }
        }
        out.push(entry.clone());
        if entry.metadata.is_dir {
            walk_dir_recursive(vfs, &entry.path, filter_entry, out)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory VFS implementation for testing VFS abstraction boundaries.
    /// Stores files and directories in hashmaps, no filesystem access.
    #[derive(Debug)]
    struct InMemoryVfs {
        root: PathBuf,
        files: Mutex<HashMap<String, Vec<u8>>>,
        dirs: Mutex<std::collections::HashSet<String>>,
    }

    impl InMemoryVfs {
        fn new(root: impl Into<PathBuf>) -> Self {
            Self {
                root: root.into(),
                files: Mutex::new(HashMap::new()),
                dirs: Mutex::new(std::collections::HashSet::new()),
            }
        }

        fn key(&self, path: &Path) -> String {
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                self.root.join(path)
            };
            resolved.to_string_lossy().to_string()
        }
    }

    impl Vfs for InMemoryVfs {
        fn read_text(&self, path: &Path) -> std::result::Result<String, VfsError> {
            let key = self.key(path);
            let files = self.files.lock().unwrap();
            let data = files
                .get(&key)
                .ok_or_else(|| VfsError::NotFound(path.to_path_buf()))?;
            String::from_utf8(data.clone())
                .map_err(|e| VfsError::Other(format!("Invalid UTF-8: {e}")))
        }

        fn read_bytes(&self, path: &Path) -> std::result::Result<Vec<u8>, VfsError> {
            let key = self.key(path);
            let files = self.files.lock().unwrap();
            files
                .get(&key)
                .cloned()
                .ok_or_else(|| VfsError::NotFound(path.to_path_buf()))
        }

        fn metadata(&self, path: &Path) -> std::result::Result<VfsMetadata, VfsError> {
            let key = self.key(path);
            let files = self.files.lock().unwrap();
            let dirs = self.dirs.lock().unwrap();
            if let Some(data) = files.get(&key) {
                Ok(VfsMetadata {
                    is_file: true,
                    is_dir: false,
                    is_symlink: false,
                    size: data.len() as u64,
                    modified: None,
                    created: None,
                })
            } else if dirs.contains(&key) {
                Ok(VfsMetadata {
                    is_file: false,
                    is_dir: true,
                    is_symlink: false,
                    size: 0,
                    modified: None,
                    created: None,
                })
            } else {
                Err(VfsError::NotFound(path.to_path_buf()))
            }
        }

        fn exists(&self, path: &Path) -> bool {
            let key = self.key(path);
            let files = self.files.lock().unwrap();
            let dirs = self.dirs.lock().unwrap();
            files.contains_key(&key) || dirs.contains(&key)
        }

        fn is_dir(&self, path: &Path) -> bool {
            let key = self.key(path);
            self.dirs.lock().unwrap().contains(&key)
        }

        fn is_file(&self, path: &Path) -> bool {
            let key = self.key(path);
            self.files.lock().unwrap().contains_key(&key)
        }

        fn canonicalize(&self, path: &Path) -> std::result::Result<PathBuf, VfsError> {
            if self.exists(path) {
                let resolved = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    self.root.join(path)
                };
                Ok(resolved)
            } else {
                Err(VfsError::NotFound(path.to_path_buf()))
            }
        }

        fn write(&self, path: &Path, content: &[u8]) -> std::result::Result<(), VfsError> {
            let key = self.key(path);
            // Auto-create parent directories (like LocalFsBackend)
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                self.root.join(path)
            };
            if let Some(parent) = resolved.parent() {
                let parent_str = parent.to_string_lossy().to_string();
                if !parent_str.is_empty() {
                    self.create_dir_all(parent).ok(); // Ignore if already exists
                }
            }
            self.files.lock().unwrap().insert(key, content.to_vec());
            Ok(())
        }

        fn create_dir_all(&self, path: &Path) -> std::result::Result<(), VfsError> {
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                self.root.join(path)
            };
            let key = resolved.to_string_lossy().to_string();
            let mut dirs = self.dirs.lock().unwrap();
            // Add this dir and all ancestors
            let mut current = key.clone();
            loop {
                dirs.insert(current.clone());
                if let Some(parent) = PathBuf::from(&current).parent() {
                    let parent_str = parent.to_string_lossy().to_string();
                    if parent_str.is_empty() || parent_str == current {
                        break;
                    }
                    current = parent_str;
                } else {
                    break;
                }
            }
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> std::result::Result<(), VfsError> {
            let key = self.key(path);
            let mut files = self.files.lock().unwrap();
            if files.remove(&key).is_some() {
                Ok(())
            } else {
                Err(VfsError::NotFound(path.to_path_buf()))
            }
        }

        fn remove_dir_all(&self, path: &Path) -> std::result::Result<(), VfsError> {
            let key = self.key(path);
            let prefix = format!("{key}/");
            let mut files = self.files.lock().unwrap();
            let mut dirs = self.dirs.lock().unwrap();
            files.retain(|k, _| !k.starts_with(&prefix) && k != &key);
            dirs.retain(|d| d != &key && !d.starts_with(&prefix));
            Ok(())
        }

        fn read_dir(&self, path: &Path) -> std::result::Result<Vec<VfsDirEntry>, VfsError> {
            let key = self.key(path);
            if !self.is_dir(path) {
                return Err(VfsError::NotADirectory(path.to_path_buf()));
            }
            let prefix = format!("{key}/");
            let files = self.files.lock().unwrap();
            let dirs = self.dirs.lock().unwrap();

            let mut entries = Vec::new();
            // Collect direct children (files)
            for (fkey, data) in files.iter() {
                if let Some(rest) = fkey.strip_prefix(&prefix) {
                    if !rest.contains('/') {
                        entries.push(VfsDirEntry {
                            path: PathBuf::from(fkey),
                            metadata: VfsMetadata {
                                is_file: true,
                                is_dir: false,
                                is_symlink: false,
                                size: data.len() as u64,
                                modified: None,
                                created: None,
                            },
                        });
                    }
                }
            }
            // Collect direct children (dirs)
            for dkey in dirs.iter() {
                if dkey != &key {
                    if let Some(rest) = dkey.strip_prefix(&prefix) {
                        if !rest.contains('/') {
                            entries.push(VfsDirEntry {
                                path: PathBuf::from(dkey),
                                metadata: VfsMetadata {
                                    is_file: false,
                                    is_dir: true,
                                    is_symlink: false,
                                    size: 0,
                                    modified: None,
                                    created: None,
                                },
                            });
                        }
                    }
                }
            }
            entries.sort_by(|a, b| a.path.cmp(&b.path));
            Ok(entries)
        }

        fn base_path(&self) -> &Path {
            &self.root
        }
    }

    // === walk_dir tests ===

    #[test]
    fn test_walk_dir_flat() {
        let vfs = InMemoryVfs::new("/workspace");
        vfs.create_dir_all(Path::new("/workspace")).unwrap();
        vfs.write_text(Path::new("/workspace/a.txt"), "a").unwrap();
        vfs.write_text(Path::new("/workspace/b.txt"), "b").unwrap();

        let entries = walk_dir(&vfs, Path::new("/workspace"), None).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_walk_dir_recursive() {
        let vfs = InMemoryVfs::new("/workspace");
        vfs.create_dir_all(Path::new("/workspace/src/utils"))
            .unwrap();
        vfs.write_text(Path::new("/workspace/Cargo.toml"), "")
            .unwrap();
        vfs.write_text(Path::new("/workspace/src/main.rs"), "")
            .unwrap();
        vfs.write_text(Path::new("/workspace/src/utils/helper.rs"), "")
            .unwrap();

        let entries = walk_dir(&vfs, Path::new("/workspace"), None).unwrap();
        // walk_dir returns: Cargo.toml, src (dir), src/main.rs, src/utils (dir), src/utils/helper.rs
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn test_walk_dir_with_filter() {
        let vfs = InMemoryVfs::new("/workspace");
        vfs.create_dir_all(Path::new("/workspace/src")).unwrap();
        vfs.create_dir_all(Path::new("/workspace/target")).unwrap();
        vfs.write_text(Path::new("/workspace/src/main.rs"), "")
            .unwrap();
        vfs.write_text(Path::new("/workspace/target/debug/app"), "")
            .unwrap();

        // Filter: skip "target" directory
        let entries = walk_dir(
            &vfs,
            Path::new("/workspace"),
            Some(&|entry: &VfsDirEntry| !entry.path.to_str().unwrap().contains("target")),
        )
        .unwrap();

        assert_eq!(entries.len(), 2); // src (dir), src/main.rs
    }

    #[test]
    fn test_walk_dir_empty_directory() {
        let vfs = InMemoryVfs::new("/workspace");
        vfs.create_dir_all(Path::new("/workspace/empty")).unwrap();

        let entries = walk_dir(&vfs, Path::new("/workspace/empty"), None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_walk_dir_nonexistent_directory() {
        let vfs = InMemoryVfs::new("/workspace");
        let result = walk_dir(&vfs, Path::new("/workspace/nope"), None);
        assert!(result.is_err());
        // InMemoryVfs returns NotADirectory for non-existent dirs
        // (since the parent /workspace may have been auto-created by other tests)
        let err = result.unwrap_err();
        assert!(err.is_not_found() || matches!(err, VfsError::NotADirectory(_)));
    }

    // === InMemoryVfs basic tests ===

    #[test]
    fn test_inmemory_read_write_text() {
        let vfs = InMemoryVfs::new("/workspace");
        vfs.write_text(Path::new("/workspace/hello.txt"), "hello world")
            .unwrap();
        let content = vfs.read_text(Path::new("/workspace/hello.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_inmemory_read_nonexistent() {
        let vfs = InMemoryVfs::new("/workspace");
        let result = vfs.read_text(Path::new("/workspace/nope.txt"));
        assert!(result.is_err());
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_inmemory_exists() {
        let vfs = InMemoryVfs::new("/workspace");
        assert!(!vfs.exists(Path::new("/workspace/file.txt")));
        vfs.write_text(Path::new("/workspace/file.txt"), "x")
            .unwrap();
        assert!(vfs.exists(Path::new("/workspace/file.txt")));
    }

    #[test]
    fn test_inmemory_is_file_is_dir() {
        let vfs = InMemoryVfs::new("/workspace");
        vfs.create_dir_all(Path::new("/workspace/dir")).unwrap();
        vfs.write_text(Path::new("/workspace/file.txt"), "x")
            .unwrap();
        assert!(vfs.is_dir(Path::new("/workspace/dir")));
        assert!(!vfs.is_file(Path::new("/workspace/dir")));
        assert!(vfs.is_file(Path::new("/workspace/file.txt")));
        assert!(!vfs.is_dir(Path::new("/workspace/file.txt")));
    }

    #[test]
    fn test_inmemory_remove_file() {
        let vfs = InMemoryVfs::new("/workspace");
        vfs.write_text(Path::new("/workspace/del.txt"), "bye")
            .unwrap();
        assert!(vfs.exists(Path::new("/workspace/del.txt")));
        vfs.remove_file(Path::new("/workspace/del.txt")).unwrap();
        assert!(!vfs.exists(Path::new("/workspace/del.txt")));
    }

    #[test]
    fn test_inmemory_remove_dir_all() {
        let vfs = InMemoryVfs::new("/workspace");
        vfs.create_dir_all(Path::new("/workspace/tree/sub"))
            .unwrap();
        vfs.write_text(Path::new("/workspace/tree/file.txt"), "")
            .unwrap();
        vfs.remove_dir_all(Path::new("/workspace/tree")).unwrap();
        assert!(!vfs.exists(Path::new("/workspace/tree")));
        assert!(!vfs.exists(Path::new("/workspace/tree/file.txt")));
    }

    #[test]
    fn test_inmemory_read_dir() {
        let vfs = InMemoryVfs::new("/workspace");
        vfs.create_dir_all(Path::new("/workspace")).unwrap();
        vfs.write_text(Path::new("/workspace/a.txt"), "").unwrap();
        vfs.write_text(Path::new("/workspace/b.txt"), "").unwrap();
        vfs.create_dir_all(Path::new("/workspace/sub")).unwrap();

        let entries = vfs.read_dir(Path::new("/workspace")).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_inmemory_metadata() {
        let vfs = InMemoryVfs::new("/workspace");
        vfs.write_text(Path::new("/workspace/info.txt"), "12345")
            .unwrap();
        let meta = vfs.metadata(Path::new("/workspace/info.txt")).unwrap();
        assert!(meta.is_file);
        assert!(!meta.is_dir);
        assert_eq!(meta.size, 5);
    }

    #[test]
    fn test_inmemory_base_path() {
        let vfs = InMemoryVfs::new("/custom/root");
        assert_eq!(vfs.base_path(), Path::new("/custom/root"));
    }

    // === VfsError tests ===

    #[test]
    fn test_vfs_error_is_not_found() {
        let err = VfsError::NotFound(PathBuf::from("/nope"));
        assert!(err.is_not_found());
        assert!(!err.is_permission_denied());
    }

    #[test]
    fn test_vfs_error_is_permission_denied() {
        let err = VfsError::PermissionDenied(PathBuf::from("/secret"));
        assert!(err.is_permission_denied());
        assert!(!err.is_not_found());
    }
}
