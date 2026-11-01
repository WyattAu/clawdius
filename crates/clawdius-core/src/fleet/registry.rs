//! Repository discovery and the active/inactive repo registry.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// One repository known to the fleet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoEntry {
    /// Short name (directory name of the repo).
    pub name: String,
    /// Absolute path of the repository root.
    pub path: String,
    /// Whether the central agent may route dev-cycle work to this repo.
    pub active: bool,
}

impl RepoEntry {
    /// Create an entry; new repos start active.
    #[must_use]
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            active: true,
        }
    }
}

/// The set of repositories the fleet supervises.
///
/// Keyed by repo name so upserts from repeated discovery scans are stable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoRegistry {
    /// Repos by name.
    pub entries: BTreeMap<String, RepoEntry>,
}

impl RepoRegistry {
    /// Create an empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Insert or update a repo; discovery upserts never flip `active` back on
    /// for repos the user deactivated (reactivation is explicit, via
    /// [`RepoRegistry::set_active`]).
    pub fn upsert(&mut self, entry: RepoEntry) {
        let active = self
            .entries
            .get(&entry.name)
            .map_or(entry.active, |existing| existing.active);
        self.entries
            .insert(entry.name.clone(), RepoEntry { active, ..entry });
    }

    /// Toggle a repo's active flag; returns false if the repo is unknown.
    pub fn set_active(&mut self, name: &str, active: bool) -> bool {
        match self.entries.get_mut(name) {
            Some(entry) => {
                entry.active = active;
                true
            },
            None => false,
        }
    }

    /// Names of repos the central agent may route work to.
    #[must_use]
    pub fn active_names(&self) -> Vec<String> {
        self.entries
            .values()
            .filter(|e| e.active)
            .map(|e| e.name.clone())
            .collect()
    }

    /// Look up a repo by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&RepoEntry> {
        self.entries.get(name)
    }

    /// Discover repositories under `root` (depth ≤ 3) and upsert them.
    ///
    /// A directory counts as a repository when it directly contains `.git`.
    /// Hidden directories and common non-repo dirs (`node_modules`, `target`,
    /// `vendor`) are skipped.
    pub fn scan(&mut self, root: &Path) {
        let mut stack: Vec<(usize, std::path::PathBuf)> = vec![(0, root.to_path_buf())];
        while let Some((depth, dir)) = stack.pop() {
            let Ok(read) = std::fs::read_dir(&dir) else {
                continue;
            };
            if dir.join(".git").exists() {
                let name = dir.file_name().map_or_else(
                    || "<root>".to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                self.upsert(RepoEntry::new(name, dir.to_string_lossy().into_owned()));
            }
            if depth >= 3 {
                continue;
            }
            for entry in read.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if name.starts_with('.') || SKIP_DIRS.contains(&name) {
                    continue;
                }
                stack.push((depth + 1, path));
            }
        }
    }
}

/// Directories never descended into during discovery.
const SKIP_DIRS: [&str; 6] = ["node_modules", "target", "vendor", "dist", "build", ".venv"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_preserves_user_deactivation() {
        let mut reg = RepoRegistry::new();
        reg.upsert(RepoEntry::new("a", "/a"));
        assert!(reg.set_active("a", false));
        reg.upsert(RepoEntry::new("a", "/a"));
        assert!(
            !reg.get("a").unwrap().active,
            "discovery must not reactivate"
        );
    }

    #[test]
    fn scan_finds_git_dirs_at_depth() {
        let tmp = tempfile::tempdir().unwrap();
        let deep = tmp.path().join("org").join("repo-one");
        std::fs::create_dir_all(deep.join(".git")).unwrap();
        std::fs::create_dir_all(tmp.path().join("node_modules").join("skip")).unwrap();
        let mut reg = RepoRegistry::new();
        reg.scan(tmp.path());
        assert_eq!(reg.entries.len(), 1);
        assert!(reg.get("repo-one").is_some());
    }
}
