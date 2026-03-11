# GitHub Issues Created During TODO Cleanup

**Date**: 2026-03-06  
**Purpose**: Track GitHub issues created as part of TODO cleanup initiative

## Issues Created

### Issue #1: Implement snapshot creation functionality
- **URL**: https://github.com/WyattAu/clawdius/issues/1
- **Priority**: Medium
- **Labels**: enhancement
- **Source**: `crates/clawdius-core/src/checkpoint/snapshot.rs:43`
- **Description**: The `SnapshotManager::create` method currently returns an empty snapshot without actually capturing the workspace state. Implementation should scan workspace, create file snapshots with content hashes, and return populated Snapshot.
- **Status**: Open

### Issue #2: Implement LLM integration in interactive mode
- **URL**: https://github.com/WyattAu/clawdius/issues/2
- **Priority**: High
- **Labels**: enhancement
- **Source**: `crates/clawdius/src/cli.rs:1307`
- **Description**: The interactive chat mode currently echoes user input instead of sending to LLM. Implementation should send input and resolved context to LLM API, process response, and display to user.
- **Status**: Open

## Summary

- **Total Issues Created**: 2
- **High Priority**: 1
- **Medium Priority**: 1
- **All Issues**: Enhancement type

## Related Documentation

- Full cleanup report: `.reports/TODO_CLEANUP_REPORT.md`
- Implementation status: `.reports/IMPLEMENTATION_STATUS_v0.7.0.md`

## Next Steps

1. Prioritize Issue #2 (LLM integration) as it's high priority
2. Schedule Issue #1 (snapshot creation) for next sprint
3. Consider breaking down issues into smaller subtasks if needed
