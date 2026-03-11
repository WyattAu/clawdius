//! Tests for checkpoint manager

#[cfg(test)]
mod tests {
    use crate::checkpoint::{CheckpointManager, FileChange};
    use std::fs;
    use tempfile::TempDir;

    async fn create_test_manager() -> (CheckpointManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let db_path = workspace_root.join(".clawdius/checkpoints.db");
        
        let manager = CheckpointManager::new(&db_path, workspace_root.clone()).unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_create_checkpoint() {
        let (manager, temp_dir) = create_test_manager().await;
        
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn main() {}").unwrap();
        
        let checkpoint = manager.create_checkpoint(
            "test-session",
            "Test checkpoint".to_string(),
            None,
        ).await.unwrap();
        
        assert!(!checkpoint.id.is_empty());
        assert_eq!(checkpoint.description, "Test checkpoint");
        assert_eq!(checkpoint.session_id, "test-session");
        assert!(!checkpoint.files.is_empty());
    }

    #[tokio::test]
    async fn test_list_checkpoints() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        manager.create_checkpoint(
            "test-session",
            "Checkpoint 1".to_string(),
            None,
        ).await.unwrap();
        
        manager.create_checkpoint(
            "test-session",
            "Checkpoint 2".to_string(),
            None,
        ).await.unwrap();
        
        let checkpoints = manager.list_checkpoints("test-session").unwrap();
        assert_eq!(checkpoints.len(), 2);
    }

    #[tokio::test]
    async fn test_restore_checkpoint() {
        let (manager, temp_dir) = create_test_manager().await;
        
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "original content").unwrap();
        
        let checkpoint = manager.create_checkpoint(
            "test-session",
            "Before change".to_string(),
            None,
        ).await.unwrap();
        
        fs::write(&test_file, "modified content").unwrap();
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "modified content");
        
        manager.restore_checkpoint(&checkpoint.id).await.unwrap();
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "original content");
    }

    #[tokio::test]
    async fn test_compare_checkpoints() {
        let (manager, temp_dir) = create_test_manager().await;
        
        let test_file = temp_dir.path().join("test.rs");
        
        fs::write(&test_file, "line 1\nline 2\nline 3").unwrap();
        let cp1 = manager.create_checkpoint(
            "test-session",
            "First version".to_string(),
            None,
        ).await.unwrap();
        
        fs::write(&test_file, "line 1\nmodified line 2\nline 3\nline 4").unwrap();
        let cp2 = manager.create_checkpoint(
            "test-session",
            "Second version".to_string(),
            None,
        ).await.unwrap();
        
        let diff = manager.compare_checkpoints(&cp1.id, &cp2.id).unwrap();
        
        assert!(!diff.file_diffs.is_empty());
        
        let changes: Vec<_> = diff.file_diffs.iter().collect();
        assert!(!changes.is_empty());
    }

    #[tokio::test]
    async fn test_delete_checkpoint() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        let checkpoint = manager.create_checkpoint(
            "test-session",
            "To be deleted".to_string(),
            None,
        ).await.unwrap();
        
        let checkpoints_before = manager.list_checkpoints("test-session").unwrap();
        assert_eq!(checkpoints_before.len(), 1);
        
        manager.delete_checkpoint(&checkpoint.id).unwrap();
        
        let checkpoints_after = manager.list_checkpoints("test-session").unwrap();
        assert_eq!(checkpoints_after.len(), 0);
    }

    #[tokio::test]
    async fn test_get_checkpoint() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        let created = manager.create_checkpoint(
            "test-session",
            "Test".to_string(),
            Some("msg-123".to_string()),
        ).await.unwrap();
        
        let loaded = manager.get_checkpoint(&created.id).unwrap();
        assert!(loaded.is_some());
        
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, created.id);
        assert_eq!(loaded.description, "Test");
        assert_eq!(loaded.message_id, Some("msg-123".to_string()));
    }

    #[tokio::test]
    async fn test_file_hashing() {
        let (manager, temp_dir) = create_test_manager().await;
        
        let file1 = temp_dir.path().join("file1.rs");
        let file2 = temp_dir.path().join("file2.rs");
        
        fs::write(&file1, "same content").unwrap();
        fs::write(&file2, "same content").unwrap();
        
        let cp = manager.create_checkpoint(
            "test-session",
            "Test".to_string(),
            None,
        ).await.unwrap();
        
        let snapshot1 = cp.files.iter().find(|f| f.path == file1).unwrap();
        let snapshot2 = cp.files.iter().find(|f| f.path == file2).unwrap();
        
        assert_eq!(snapshot1.hash, snapshot2.hash);
    }

    #[tokio::test]
    async fn test_auto_checkpoint() {
        let (manager, temp_dir) = create_test_manager().await;
        
        let test_file = temp_dir.path().join("auto.rs");
        fs::write(&test_file, "content").unwrap();
        
        let checkpoint = manager.auto_checkpoint("test-session", &test_file).await.unwrap();
        
        assert!(checkpoint.is_some());
        let checkpoint = checkpoint.unwrap();
        assert!(checkpoint.description.contains("Auto-checkpoint"));
    }
}
