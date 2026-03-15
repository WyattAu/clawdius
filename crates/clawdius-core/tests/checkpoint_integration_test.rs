//! Integration tests for checkpoint system

use clawdius_core::checkpoint::{CheckpointManager, FileChange};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_full_checkpoint_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().to_path_buf();
    let db_path = workspace_root.join(".clawdius/checkpoints.db");

    let manager = CheckpointManager::new(&db_path, workspace_root.clone()).unwrap();

    let src_file = workspace_root.join("src/main.rs");
    fs::create_dir_all(src_file.parent().unwrap()).unwrap();
    fs::write(&src_file, "fn main() { println!(\"v1\"); }").unwrap();

    let cp1 = manager
        .create_checkpoint("test-session", "Initial version".to_string(), None)
        .await
        .unwrap();

    assert!(!cp1.id.is_empty());
    assert_eq!(cp1.description, "Initial version");
    assert!(!cp1.files.is_empty());

    fs::write(&src_file, "fn main() { println!(\"v2\"); }").unwrap();

    let lib_file = workspace_root.join("src/lib.rs");
    fs::write(&lib_file, "pub fn helper() {}").unwrap();

    let cp2 = manager
        .create_checkpoint("test-session", "Second version".to_string(), None)
        .await
        .unwrap();

    let checkpoints = manager.list_checkpoints("test-session").unwrap();
    assert_eq!(checkpoints.len(), 2);

    let diff = manager.compare_checkpoints(&cp1.id, &cp2.id).unwrap();
    assert!(!diff.file_diffs.is_empty());

    let mut has_modifications = false;
    for change in diff.file_diffs.values() {
        if matches!(change, FileChange::Modified(_)) {
            has_modifications = true;
            break;
        }
    }
    assert!(has_modifications);

    manager.restore_checkpoint(&cp1.id).await.unwrap();

    let restored_content = fs::read_to_string(&src_file).unwrap();
    assert_eq!(restored_content, "fn main() { println!(\"v1\"); }");

    assert!(!lib_file.exists());

    manager.delete_checkpoint(&cp2.id).unwrap();
    let remaining = manager.list_checkpoints("test-session").unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, cp1.id);
}

#[tokio::test]
async fn test_checkpoint_with_multiple_files() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().to_path_buf();
    let db_path = workspace_root.join(".clawdius/checkpoints.db");

    let manager = CheckpointManager::new(&db_path, workspace_root.clone()).unwrap();

    let files = vec![
        ("src/main.rs", "fn main() {}"),
        ("src/lib.rs", "pub mod utils;"),
        ("src/utils.rs", "pub fn helper() {}"),
        ("Cargo.toml", "[package]\nname = \"test\""),
    ];

    for (path, content) in &files {
        let full_path = workspace_root.join(path);
        fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        fs::write(&full_path, content).unwrap();
    }

    let checkpoint = manager
        .create_checkpoint("multi-file-session", "Multiple files".to_string(), None)
        .await
        .unwrap();

    assert!(checkpoint.files.len() >= files.len());

    for (path, _) in &files {
        let full_path = workspace_root.join(path);
        let snapshot = checkpoint.files.iter().find(|f| f.path == full_path);
        assert!(snapshot.is_some(), "File {path} should be in checkpoint");
    }
}

#[tokio::test]
async fn test_auto_checkpoint_feature() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().to_path_buf();
    let db_path = workspace_root.join(".clawdius/checkpoints.db");

    let manager = CheckpointManager::new(&db_path, workspace_root.clone()).unwrap();

    let test_file = workspace_root.join("test.rs");
    fs::write(&test_file, "original content").unwrap();

    let checkpoint = manager
        .auto_checkpoint("auto-session", &test_file)
        .await
        .unwrap();

    assert!(checkpoint.is_some());
    let checkpoint = checkpoint.unwrap();
    assert!(checkpoint.description.contains("Auto-checkpoint"));
    assert!(checkpoint.description.contains("test.rs"));
}

#[tokio::test]
async fn test_checkpoint_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().to_path_buf();
    let db_path = workspace_root.join(".clawdius/checkpoints.db");

    let manager = CheckpointManager::new(&db_path, workspace_root.clone()).unwrap();

    let file = workspace_root.join("test.rs");
    fs::write(&file, "content").unwrap();

    let cp1 = manager
        .create_checkpoint("session-1", "S1".to_string(), None)
        .await
        .unwrap();
    let cp2 = manager
        .create_checkpoint("session-2", "S2".to_string(), None)
        .await
        .unwrap();

    let s1_checkpoints = manager.list_checkpoints("session-1").unwrap();
    assert_eq!(s1_checkpoints.len(), 1);
    assert_eq!(s1_checkpoints[0].id, cp1.id);

    let s2_checkpoints = manager.list_checkpoints("session-2").unwrap();
    assert_eq!(s2_checkpoints.len(), 1);
    assert_eq!(s2_checkpoints[0].id, cp2.id);
}

#[tokio::test]
async fn test_timeline() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().to_path_buf();
    let db_path = workspace_root.join(".clawdius/checkpoints.db");

    let manager = CheckpointManager::new(&db_path, workspace_root.clone()).unwrap();

    let file = workspace_root.join("test.rs");
    fs::write(&file, "content").unwrap();

    let cp1 = manager
        .create_checkpoint("timeline-session", "First".to_string(), None)
        .await
        .unwrap();
    let cp2 = manager
        .create_checkpoint("timeline-session", "Second".to_string(), None)
        .await
        .unwrap();
    let cp3 = manager
        .create_checkpoint("timeline-session", "Third".to_string(), None)
        .await
        .unwrap();

    let timeline = manager.get_timeline("timeline-session").unwrap();

    assert_eq!(timeline.session_id, "timeline-session");
    assert_eq!(timeline.checkpoints.len(), 3);
    assert_eq!(timeline.current_index, Some(2));

    assert_eq!(timeline.checkpoints[0].id, cp1.id);
    assert_eq!(timeline.checkpoints[0].description, "First");
    assert_eq!(timeline.checkpoints[1].id, cp2.id);
    assert_eq!(timeline.checkpoints[1].description, "Second");
    assert_eq!(timeline.checkpoints[2].id, cp3.id);
    assert_eq!(timeline.checkpoints[2].description, "Third");
}

#[tokio::test]
async fn test_checkpoint_summary() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().to_path_buf();
    let db_path = workspace_root.join(".clawdius/checkpoints.db");

    let manager = CheckpointManager::new(&db_path, workspace_root.clone()).unwrap();

    let file = workspace_root.join("test.rs");
    fs::write(&file, "content").unwrap();

    let checkpoint = manager
        .create_checkpoint(
            "summary-session",
            "Test summary".to_string(),
            Some("msg-123".to_string()),
        )
        .await
        .unwrap();

    let summary = manager.get_checkpoint_summary(&checkpoint.id).unwrap();

    assert!(summary.is_some());
    let summary = summary.unwrap();
    assert_eq!(summary.id, checkpoint.id);
    assert_eq!(summary.description, "Test summary");
    assert_eq!(summary.message_id, Some("msg-123".to_string()));
    assert!(summary.file_count > 0);
}

#[tokio::test]
async fn test_cleanup_old_checkpoints() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().to_path_buf();
    let db_path = workspace_root.join(".clawdius/checkpoints.db");

    let manager = CheckpointManager::new(&db_path, workspace_root.clone()).unwrap();

    let file = workspace_root.join("test.rs");
    fs::write(&file, "content").unwrap();

    for i in 1..=5 {
        manager
            .create_checkpoint("cleanup-session", format!("CP {i}"), None)
            .await
            .unwrap();
    }

    let before = manager.list_checkpoints("cleanup-session").unwrap();
    assert_eq!(before.len(), 5);

    let deleted = manager
        .cleanup_old_checkpoints("cleanup-session", 3)
        .unwrap();
    assert_eq!(deleted, 2);

    let after = manager.list_checkpoints("cleanup-session").unwrap();
    assert_eq!(after.len(), 3);
}
