# Checkpoint System

The checkpoint system provides file timeline and restoration capabilities for Clawdius.

## Overview

Checkpoints allow you to:
- Save the current state of your workspace
- Restore to a previous checkpoint
- Compare differences between checkpoints
- Track file changes over time

## CLI Commands

### Create a Checkpoint

Create a checkpoint of the current workspace state:

```bash
clawdius checkpoint create "Before refactoring"
```

With a specific session:

```bash
clawdius checkpoint create "Before refactoring" --session my-session
```

### List Checkpoints

List all checkpoints for the current session:

```bash
clawdius checkpoint list
```

With verbose output showing file details:

```bash
clawdius checkpoint list --verbose
```

For a specific session:

```bash
clawdius checkpoint list --session my-session
```

### Restore a Checkpoint

Restore workspace to a specific checkpoint:

```bash
clawdius checkpoint restore <checkpoint-id>
```

### Compare Checkpoints

Compare two checkpoints to see differences:

```bash
clawdius checkpoint compare <checkpoint-id-1> <checkpoint-id-2>
```

### Delete a Checkpoint

Delete a checkpoint:

```bash
clawdius checkpoint delete <checkpoint-id>
```

## Features

### Automatic File Tracking

The checkpoint system automatically tracks:
- Source code files (`.rs`, `.py`, `.js`, `.ts`, `.go`, etc.)
- Configuration files (`.toml`, `.json`, `.yaml`, etc.)
- Any non-binary files in the workspace

### File Hashing

Files are hashed using SHA3-256 for integrity verification and efficient comparison.

### Smart Restoration

When restoring a checkpoint:
- Files are restored to their exact state at checkpoint creation
- Directory structure is preserved
- Empty directories are created as needed

### Diff Comparison

The comparison feature shows:
- Added files (+)
- Deleted files (-)
- Modified files (~) with inline diffs

## Programmatic Usage

```rust
use clawdius_core::checkpoint::CheckpointManager;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = std::env::current_dir()?;
    let db_path = workspace_root.join(".clawdius/checkpoints.db");
    
    let manager = CheckpointManager::new(&db_path, workspace_root)?;
    
    // Create a checkpoint
    let checkpoint = manager.create_checkpoint(
        "my-session",
        "Before major changes".to_string(),
        None,
    ).await?;
    
    println!("Created checkpoint: {}", checkpoint.id);
    
    // List checkpoints
    let checkpoints = manager.list_checkpoints("my-session")?;
    for cp in checkpoints {
        println!("- {} ({})", cp.description, cp.timestamp);
    }
    
    // Restore to checkpoint
    manager.restore_checkpoint(&checkpoint.id).await?;
    
    // Compare checkpoints
    let diff = manager.compare_checkpoints("cp-id-1", "cp-id-2")?;
    for (path, change) in &diff.file_diffs {
        println!("{}: {:?}", path.display(), change);
    }
    
    Ok(())
}
```

## Database Schema

Checkpoints are stored in SQLite with the following schema:

```sql
CREATE TABLE checkpoints (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    message_id TEXT,
    description TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    file_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE checkpoint_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    hash TEXT NOT NULL,
    snapshot_file TEXT NOT NULL,
    UNIQUE(checkpoint_id, path)
);
```

## Integration with Sessions

Checkpoints can be associated with session messages:

```rust
// Create checkpoint after LLM response
let message_id = "msg-123";
let checkpoint = manager.create_checkpoint(
    &session.id,
    "After AI suggestion".to_string(),
    Some(message_id.to_string()),
).await?;
```

## Best Practices

1. **Create checkpoints before major changes**: This allows easy rollback
2. **Use descriptive names**: Help identify the purpose of each checkpoint
3. **Regular cleanup**: Delete old checkpoints to save disk space
4. **Session association**: Link checkpoints to session messages for context

## Limitations

- Binary files are not tracked
- Large files may slow down checkpoint creation
- File permissions are not preserved
- Symbolic links are not followed

## Future Enhancements

- [ ] Incremental snapshots for large files
- [ ] Compression of snapshot data
- [ ] Remote backup integration
- [ ] File permission tracking
- [ ] Binary file diff support
