use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VfsPath(PathBuf);

impl VfsPath {
    pub fn new(path: impl Into<PathBuf>) -> std::result::Result<Self, VfsPathError> {
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(VfsPathError::EmptyPath);
        }
        Self::validate_components(&path)?;
        Ok(Self(path))
    }

    pub fn join(&self, segment: impl AsRef<Path>) -> std::result::Result<Self, VfsPathError> {
        let joined = self.0.join(segment);
        Self::validate_components(&joined)?;
        Ok(Self(joined))
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    #[must_use]
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }

    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        self.0.parent().map(|p| Self(p.to_path_buf()))
    }

    #[must_use]
    pub fn file_name(&self) -> Option<&std::ffi::OsStr> {
        self.0.file_name()
    }

    #[must_use]
    pub fn extension(&self) -> Option<&std::ffi::OsStr> {
        self.0.extension()
    }

    #[must_use]
    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        self.0.to_string_lossy()
    }

    fn validate_components(path: &Path) -> std::result::Result<(), VfsPathError> {
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(VfsPathError::PathTraversal {
                        path: path.to_path_buf(),
                    });
                },
                std::path::Component::Prefix(p) => {
                    let verbatim = p.as_os_str().to_string_lossy();
                    if verbatim.starts_with(r"\\?\") {
                        return Err(VfsPathError::InvalidPrefix {
                            prefix: verbatim.into_owned(),
                        });
                    }
                },
                std::path::Component::Normal(s) => {
                    let name = s.to_string_lossy();
                    if name.is_empty() {
                        return Err(VfsPathError::EmptyComponent {
                            path: path.to_path_buf(),
                        });
                    }
                    if name.contains('\0') {
                        return Err(VfsPathError::NullByte {
                            path: path.to_path_buf(),
                        });
                    }
                },
                std::path::Component::CurDir | std::path::Component::RootDir => {},
            }
        }
        Ok(())
    }
}

impl AsRef<Path> for VfsPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl From<VfsPath> for PathBuf {
    fn from(v: VfsPath) -> Self {
        v.0
    }
}

impl std::fmt::Display for VfsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.display().fmt(f)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VfsPathError {
    #[error("Path is empty")]
    EmptyPath,
    #[error("Path traversal detected: {path}")]
    PathTraversal { path: PathBuf },
    #[error("Invalid path prefix: {prefix}")]
    InvalidPrefix { prefix: String },
    #[error("Empty path component in: {path}")]
    EmptyComponent { path: PathBuf },
    #[error("Null byte in path: {path}")]
    NullByte { path: PathBuf },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_path() {
        let p = VfsPath::new("foo/bar.txt").unwrap();
        assert_eq!(p.as_path(), Path::new("foo/bar.txt"));
    }

    #[test]
    fn test_parent_dir_rejected() {
        assert!(VfsPath::new("../etc/passwd").is_err());
        assert!(VfsPath::new("foo/../bar").is_err());
    }

    #[test]
    fn test_empty_path_rejected() {
        assert!(VfsPath::new("").is_err());
    }

    #[test]
    fn test_join_valid() {
        let base = VfsPath::new("foo").unwrap();
        let joined = base.join("bar.txt").unwrap();
        assert_eq!(joined.as_path(), Path::new("foo/bar.txt"));
    }

    #[test]
    fn test_join_traversal_rejected() {
        let base = VfsPath::new("foo").unwrap();
        assert!(base.join("../bar").is_err());
    }

    #[test]
    fn test_display() {
        let p = VfsPath::new("hello/world.rs").unwrap();
        assert_eq!(format!("{p}"), "hello/world.rs");
    }
}
