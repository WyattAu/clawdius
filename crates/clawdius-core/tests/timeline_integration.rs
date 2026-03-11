//! Integration tests for File Timeline system
//!
//! Tests checkpoint creation, rollback functionality, diff generation,
//! file history tracking, and cleanup operations.

use clawdius_core::timeline::{
    ChangeKind, CheckpointId, FileWatcher, TimelineManager, WatcherConfig,
};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

fn create_test_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&path, content).unwrap();
    path
}

#[tokio::test]
async fn test_create_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    create_test_file(&temp_dir, "src/main.rs", "fn main() {}");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    let checkpoint_id = manager.create_checkpoint("initial").await.unwrap();

    assert!(!checkpoint_id.0.is_empty());
}

#[tokio::test]
async fn test_create_checkpoint_with_description() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    create_test_file(&temp_dir, "src/main.rs", "fn main() {}");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    let checkpoint_id = manager
        .create_checkpoint_with_description("initial", "Initial checkpoint")
        .await
        .unwrap();

    assert!(!checkpoint_id.0.is_empty());

    let checkpoint = manager.get_checkpoint(&checkpoint_id).unwrap();
    assert!(checkpoint.is_some());
    let checkpoint = checkpoint.unwrap();
    assert_eq!(
        checkpoint.description,
        Some("Initial checkpoint".to_string())
    );
}

#[tokio::test]
async fn test_list_checkpoints() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    create_test_file(&temp_dir, "src/main.rs", "fn main() {}");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    manager.create_checkpoint("checkpoint1").await.unwrap();
    manager.create_checkpoint("checkpoint2").await.unwrap();
    manager.create_checkpoint("checkpoint3").await.unwrap();

    let checkpoints = manager.list_checkpoints().unwrap();

    assert_eq!(checkpoints.len(), 3);
}

#[tokio::test]
async fn test_get_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    create_test_file(&temp_dir, "src/main.rs", "fn main() {}");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    let checkpoint_id = manager.create_checkpoint("test_checkpoint").await.unwrap();

    let checkpoint = manager.get_checkpoint(&checkpoint_id).unwrap();

    assert!(checkpoint.is_some());
    let checkpoint = checkpoint.unwrap();
    assert_eq!(checkpoint.name, "test_checkpoint");
}

#[tokio::test]
async fn test_get_nonexistent_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    let manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    let fake_id = CheckpointId::from_string("nonexistent-id".to_string());
    let checkpoint = manager.get_checkpoint(&fake_id).unwrap();

    assert!(checkpoint.is_none());
}

#[tokio::test]
async fn test_rollback_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    let test_file = create_test_file(&temp_dir, "src/main.rs", "original content");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    let checkpoint_id = manager.create_checkpoint("before_change").await.unwrap();

    fs::write(&test_file, "modified content").unwrap();

    let content_after = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content_after, "modified content");

    manager.rollback(&checkpoint_id).await.unwrap();

    let content_restored = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content_restored, "original content");
}

#[tokio::test]
async fn test_diff_checkpoints() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    create_test_file(&temp_dir, "src/main.rs", "version 1");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    let checkpoint1 = manager.create_checkpoint("v1").await.unwrap();

    create_test_file(&temp_dir, "src/main.rs", "version 2");

    let checkpoint2 = manager.create_checkpoint("v2").await.unwrap();

    let diff = manager.diff(&checkpoint1, &checkpoint2).unwrap();

    assert_eq!(diff.from, checkpoint1);
    assert_eq!(diff.to, checkpoint2);
}

#[tokio::test]
async fn test_file_history() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    let test_file = create_test_file(&temp_dir, "src/main.rs", "version 1");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    manager.create_checkpoint("v1").await.unwrap();

    fs::write(&test_file, "version 2").unwrap();
    manager.create_checkpoint("v2").await.unwrap();

    fs::write(&test_file, "version 3").unwrap();
    manager.create_checkpoint("v3").await.unwrap();

    let history = manager.get_file_history(&test_file).unwrap();

    assert!(!history.is_empty());
}

#[tokio::test]
async fn test_delete_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    create_test_file(&temp_dir, "src/main.rs", "content");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    let checkpoint_id = manager.create_checkpoint("to_delete").await.unwrap();

    let checkpoints_before = manager.list_checkpoints().unwrap();
    assert_eq!(checkpoints_before.len(), 1);

    manager.delete_checkpoint(&checkpoint_id).unwrap();

    let checkpoints_after = manager.list_checkpoints().unwrap();
    assert_eq!(checkpoints_after.len(), 0);
}

#[tokio::test]
async fn test_cleanup_old_checkpoints() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    create_test_file(&temp_dir, "src/main.rs", "content");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    for i in 0..10 {
        manager
            .create_checkpoint(&format!("checkpoint_{}", i))
            .await
            .unwrap();
    }

    let checkpoints_before = manager.list_checkpoints().unwrap();
    assert_eq!(checkpoints_before.len(), 10);

    let deleted = manager.cleanup_old_checkpoints(5).unwrap();
    assert_eq!(deleted, 5);

    let checkpoints_after = manager.list_checkpoints().unwrap();
    assert_eq!(checkpoints_after.len(), 5);
}

#[tokio::test]
async fn test_cleanup_with_fewer_checkpoints() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    create_test_file(&temp_dir, "src/main.rs", "content");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    manager.create_checkpoint("checkpoint1").await.unwrap();
    manager.create_checkpoint("checkpoint2").await.unwrap();

    let deleted = manager.cleanup_old_checkpoints(5).unwrap();
    assert_eq!(deleted, 0);

    let checkpoints = manager.list_checkpoints().unwrap();
    assert_eq!(checkpoints.len(), 2);
}

#[tokio::test]
async fn test_track_file() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    let test_file = create_test_file(&temp_dir, "src/tracked.rs", "tracked content");

    let manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    manager.track_file(&test_file).unwrap();
}

#[tokio::test]
async fn test_checkpoint_id_creation() {
    let id1 = CheckpointId::new();
    let id2 = CheckpointId::new();

    assert_ne!(id1.0, id2.0);
    assert!(!id1.0.is_empty());
}

#[tokio::test]
async fn test_checkpoint_id_from_string() {
    let original = "test-checkpoint-id".to_string();
    let id = CheckpointId::from_string(original.clone());

    assert_eq!(id.0, original);
}

#[tokio::test]
async fn test_multiple_files_in_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    create_test_file(&temp_dir, "src/main.rs", "fn main() {}");
    create_test_file(&temp_dir, "src/lib.rs", "pub fn lib() {}");
    create_test_file(&temp_dir, "src/utils.rs", "pub fn util() {}");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    let checkpoint_id = manager.create_checkpoint("multi_file").await.unwrap();

    let checkpoint = manager.get_checkpoint(&checkpoint_id).unwrap().unwrap();
    assert!(checkpoint.files_count > 0);
}

#[tokio::test]
async fn test_timeline_manager_with_watcher() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    create_test_file(&temp_dir, "src/main.rs", "fn main() {}");

    let config = WatcherConfig::default();
    let manager =
        TimelineManager::with_watcher(&db_path, temp_dir.path().to_path_buf(), config).unwrap();

    manager.start_watching().unwrap();
    manager.stop_watching().unwrap();
}

#[tokio::test]
async fn test_timeline_manager_create_watcher() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    let manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    let config = WatcherConfig {
        debounce_interval: Duration::from_secs(10),
        max_checkpoints_per_hour: 60,
        ..Default::default()
    };

    let _watcher = manager.create_watcher(config);
}

// Watcher tests
mod watcher_tests {
    use super::*;

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();

        assert_eq!(config.debounce_interval, Duration::from_secs(30));
        assert!(config.auto_checkpoint);
        assert_eq!(config.max_checkpoints_per_hour, 120);
        assert!(!config.ignore_patterns.is_empty());
    }

    #[test]
    fn test_watcher_ignore_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let config = WatcherConfig::default();
        let watcher = FileWatcher::new(temp_dir.path().to_path_buf(), config);

        assert!(watcher.should_ignore(&temp_dir.path().join(".git/config")));
        assert!(watcher.should_ignore(&temp_dir.path().join("target/debug/test")));
        assert!(watcher.should_ignore(&temp_dir.path().join("node_modules/package")));
        assert!(watcher.should_ignore(&temp_dir.path().join("test.swp")));
        assert!(watcher.should_ignore(&temp_dir.path().join("file.swo")));
        assert!(watcher.should_ignore(&temp_dir.path().join("file~")));
        assert!(watcher.should_ignore(&temp_dir.path().join(".clawdius/config.toml")));
        assert!(watcher.should_ignore(&temp_dir.path().join("Cargo.lock")));

        assert!(!watcher.should_ignore(&temp_dir.path().join("src/main.rs")));
        assert!(!watcher.should_ignore(&temp_dir.path().join("lib/test.rs")));
    }

    #[test]
    fn test_watcher_custom_ignore_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let config = WatcherConfig {
            ignore_patterns: vec!["custom_dir/".to_string(), "*.log".to_string()],
            ..Default::default()
        };
        let watcher = FileWatcher::new(temp_dir.path().to_path_buf(), config);

        assert!(watcher.should_ignore(&temp_dir.path().join("custom_dir/file.txt")));
        assert!(watcher.should_ignore(&temp_dir.path().join("test.log")));

        assert!(!watcher.should_ignore(&temp_dir.path().join("src/main.rs")));
    }

    #[tokio::test]
    async fn test_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WatcherConfig::default();
        let watcher = FileWatcher::new(temp_dir.path().to_path_buf(), config);

        assert!(watcher.is_running().await);
    }

    #[tokio::test]
    async fn test_watcher_stop() {
        let temp_dir = TempDir::new().unwrap();
        let config = WatcherConfig::default();
        let watcher = FileWatcher::new(temp_dir.path().to_path_buf(), config);

        watcher.stop().await;
        assert!(!watcher.is_running().await);
    }

    #[test]
    fn test_change_kind_conversion() {
        use notify::EventKind as NotifyEventKind;

        assert!(matches!(
            ChangeKind::from(NotifyEventKind::Create(notify::event::CreateKind::Any)),
            ChangeKind::Created
        ));
        assert!(matches!(
            ChangeKind::from(NotifyEventKind::Modify(notify::event::ModifyKind::Any)),
            ChangeKind::Modified
        ));
        assert!(matches!(
            ChangeKind::from(NotifyEventKind::Remove(notify::event::RemoveKind::Any)),
            ChangeKind::Deleted
        ));
        assert!(matches!(
            ChangeKind::from(NotifyEventKind::Any),
            ChangeKind::Any
        ));
    }

    #[tokio::test]
    async fn test_watcher_with_manager() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join(".clawdius/timeline.db");

        create_test_file(&temp_dir, "src/main.rs", "fn main() {}");

        let manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let config = WatcherConfig {
            debounce_interval: Duration::from_secs(5),
            ..Default::default()
        };

        let watcher = manager.create_watcher(config);

        let checkpoint_created = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let checkpoint_created_clone = checkpoint_created.clone();

        let handle = tokio::spawn(async move {
            watcher
                .watch(move |_paths, _kind| {
                    let checkpoint_created = checkpoint_created_clone.clone();
                    async move {
                        checkpoint_created.store(true, std::sync::atomic::Ordering::SeqCst);
                        Ok(())
                    }
                })
                .await
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        create_test_file(&temp_dir, "src/new_file.rs", "// test");

        tokio::time::sleep(Duration::from_millis(500)).await;

        handle.abort();
    }
}

#[tokio::test]
async fn test_empty_workspace_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join(".clawdius/timeline.db");

    let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

    let checkpoint_id = manager.create_checkpoint("empty").await.unwrap();

    let checkpoint = manager.get_checkpoint(&checkpoint_id).unwrap().unwrap();
    assert_eq!(checkpoint.files_count, 0);
}
