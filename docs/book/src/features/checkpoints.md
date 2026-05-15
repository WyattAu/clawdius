# Checkpoints and Timeline

Clawdius provides two complementary systems for tracking file changes and enabling rollback.

## Overview

- **Session Checkpoints**: Per-session snapshots of file state
- **Timeline**: Project-level version history with diffing and rollback

## Session Checkpoints

### Creating a Checkpoint

```bash
clawdius checkpoint create "before refactoring"
```

### Listing Checkpoints

```bash
clawdius checkpoint list
clawdius checkpoint list --verbose    # Show file details
clawdius checkpoint list --session abc123
```

### Restoring a Checkpoint

```bash
clawdius checkpoint restore <checkpoint-id>
```

### Comparing Checkpoints

```bash
clawdius checkpoint compare <id1> <id2>
```

### Deleting Checkpoints

```bash
clawdius checkpoint delete <checkpoint-id>
```

### Cleanup

Keep only the most recent checkpoints:

```bash
clawdius checkpoint cleanup --keep 10
clawdius checkpoint cleanup --session abc123 --keep 5
```

## Timeline

The timeline system provides project-level file version tracking independent of sessions.

### Creating Timeline Entries

```bash
clawdius timeline create "v1.0 release"
clawdius timeline create "pre-refactor" --description "Before major refactor"
```

### Listing Timeline Entries

```bash
clawdius timeline list
```

### Rollback

Restore your project to a previous state:

```bash
clawdius timeline rollback <checkpoint-id>
```

### Diffing

Compare two points in time:

```bash
clawdius timeline diff <from-id> <to-id>
```

### File History

View the change history for a specific file:

```bash
clawdius timeline history src/lib.rs
```

### Watch Mode

Automatically create checkpoints when files change:

```bash
clawdius timeline watch
clawdius timeline watch --debounce-secs 60 --max-per-hour 60
clawdius timeline watch --ignore "*.log" --ignore "node_modules"
```

### Cleanup

```bash
clawdius timeline cleanup --keep 100
```

## Timeline Data Structure

Each checkpoint stores:

| Field | Type | Description |
|-------|------|-------------|
| `id` | String | Unique checkpoint identifier |
| `name` | String | Human-readable name |
| `description` | Option\<String\> | Optional description |
| `tag` | Option\<String\> | Optional tag |
| `timestamp` | DateTime | Creation time |
| `files_changed` | usize | Number of files |
| `checksum` | String | Content hash (BLAKE3) |

## Diff Format

The diff between checkpoints includes:

| Field | Description |
|-------|-------------|
| `files_added` | Newly created files |
| `files_modified` | Changed files with additions/deletions |
| `files_deleted` | Removed files |
| `stats` | Aggregate change statistics |

## Typical Workflow

```bash
# 1. Create checkpoint before making changes
clawdius timeline create "before-auth-feature"

# 2. Make changes...
# 3. Create intermediate checkpoint
clawdius timeline create "auth-models-done"

# 4. Review what changed
clawdius timeline diff "before-auth-feature" "auth-models-done"

# 5. If something went wrong, rollback
clawdius timeline rollback "before-auth-feature"
```

## Storage

Timeline data is stored in SQLite. Metadata (timestamps, file lists, checksums) lives in the database, while file snapshots are stored on the filesystem.

```toml
[storage]
database_path = ".clawdius/graph/index.db"
```
