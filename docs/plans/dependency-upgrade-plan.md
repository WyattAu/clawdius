# dependency-upgrade-plan

status: active
updated: 2024-06-18  rev: 1

## State
| # | Task | Status | Evidence | Next |
|---|------|--------|----------|------|
| 1 | Check available updates | done | `cargo update --dry-run` | Start updating dependencies |
| 2 | Update addr2line v0.25.1 -> v0.27.1 | pending | | Update first dependency |
| 3 | Update aes v0.9.3 -> v0.9.4 | pending | | Continue with next dependency |
| 4 | Update alloc-stdlib v0.2.2 -> v0.2.4 | pending | | Continue with next dependency |
| 5 | Update android_system_properties v0.1.5 -> v0.1.6 | pending | | Continue with next dependency |
| 6 | Update apple-native-keyring-store v1.0.0 -> v1.0.2 | pending | | Continue with next dependency |
| 7 | Update ar_archive_writer v0.5.2 -> v0.5.5 | pending | | Continue with next dependency |
| 8 | Update arbitrary v1.4.2 -> v1.5.0 | pending | | Continue with next dependency |
| 9 | Update arboard v3.6.1 -> v3.7.0 | pending | | Continue with next dependency |
| 10 | Update arc-swap v1.9.1 -> v1.9.5 | pending | | Continue with next dependency |

## Next action
Update addr2line from v0.25.1 to v0.27.1 in Cargo.toml and verify it compiles

## Blocked
- None yet

## Log
- 2024-06-18 Initial dependency check completed, 382 packages can be updated