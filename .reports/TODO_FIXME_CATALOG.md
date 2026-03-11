# TODO/FIXME Catalog

**Generated:** 2026-03-08
**Project:** Clawdius - High-Assurance AI Agentic Engine
**Version:** 0.8.0-alpha

## Summary

| Metric | Count |
|--------|-------|
| Total Items Found | 65 |
| Actionable TODOs | 31 |
| Template Strings (Intentional) | 34 |
| **Critical** | 0 |
| **High** | 6 |
| **Medium** | 25 |
| **Low** | 0 |

### Classification Overview

| Category | Count | Priority Distribution |
|----------|-------|----------------------|
| Infrastructure/Feature | 6 | 6 High |
| Test Implementation | 25 | 25 Medium |
| Template Strings | 34 | N/A (Intentional) |

---

## Items by Priority

### High Priority (Infrastructure/Features)

| ID | File | Line | Text | Category | Effort |
|----|------|------|------|----------|--------|
| TODO-001 | crates/clawdius-core/src/nexus/artifacts.rs | 144 | Add SQLite connection pool | Feature | 4h |
| TODO-002 | crates/clawdius-core/src/nexus/artifacts.rs | 146 | Add LRU cache | Feature | 3h |
| TODO-003 | crates/clawdius-core/src/nexus/artifacts.rs | 154 | Initialize SQLite database | Feature | 4h |
| TODO-004 | crates/clawdius-core/src/nexus/artifacts.rs | 155 | Create schema | Feature | 2h |
| TODO-005 | crates/clawdius-core/src/nexus/events.rs | 275 | Add metrics storage | Feature | 3h |
| TODO-006 | crates/clawdius-core/src/nexus/events.rs | 293 | Add database connection for audit storage | Feature | 3h |

### Medium Priority (Test Implementations)

| ID | File | Line | Text | Category | Effort |
|----|------|------|------|----------|--------|
| TODO-007 | crates/clawdius-core/src/nexus/tests.rs | 16 | Test complete lifecycle from Phase 0 to Phase 23 | Test | 4h |
| TODO-008 | crates/clawdius-core/src/nexus/tests.rs | 27 | Test that invalid transitions are rejected | Test | 2h |
| TODO-009 | crates/clawdius-core/src/nexus/tests.rs | 36 | Test artifact dependency tracking | Test | 3h |
| TODO-010 | crates/clawdius-core/src/nexus/tests.rs | 46 | Test quality gate system | Test | 3h |
| TODO-011 | crates/clawdius-core/src/nexus/tests.rs | 56 | Test event bus with real subscribers | Test | 2h |
| TODO-012 | crates/clawdius-core/src/nexus/tests.rs | 66 | Test transition rollback | Test | 3h |
| TODO-013 | crates/clawdius-core/src/nexus/tests.rs | 76 | Test thread safety | Test | 4h |
| TODO-014 | crates/clawdius-core/src/nexus/tests.rs | 85 | Test persistence | Test | 3h |
| TODO-015 | crates/clawdius-core/src/nexus/tests.rs | 158 | Test engine initialization | Test | 2h |
| TODO-016 | crates/clawdius-core/src/nexus/tests.rs | 164 | Test artifact operations | Test | 2h |
| TODO-017 | crates/clawdius-core/src/nexus/tests.rs | 170 | Test gate operations | Test | 2h |
| TODO-018 | crates/clawdius-core/src/nexus/tests.rs | 176 | Test event operations | Test | 2h |
| TODO-019 | crates/clawdius-core/src/nexus/tests.rs | 188 | Implement with proptest (artifact hash) | Test | 3h |
| TODO-020 | crates/clawdius-core/src/nexus/tests.rs | 195 | Implement with proptest (phase transitions) | Test | 3h |
| TODO-021 | crates/clawdius-core/src/nexus/tests.rs | 202 | Implement with proptest (artifact ID uniqueness) | Test | 2h |
| TODO-022 | crates/clawdius-core/src/nexus/artifacts.rs | 346 | Implement storage test with temp directory | Test | 2h |
| TODO-023 | crates/clawdius-core/src/nexus/artifacts.rs | 352 | Implement retrieval test | Test | 1h |
| TODO-024 | crates/clawdius-core/src/nexus/artifacts.rs | 358 | Implement dependency tracking test | Test | 2h |
| TODO-025 | crates/clawdius-core/src/nexus/events.rs | 404 | Implement metrics handler test | Test | 2h |
| TODO-026 | crates/clawdius-core/src/nexus/events.rs | 410 | Implement audit handler test | Test | 2h |
| TODO-027 | crates/clawdius-core/src/nexus/gates.rs | 318 | Implement full gate evaluation test | Test | 2h |
| TODO-028 | crates/clawdius-core/src/nexus/gates.rs | 324 | Implement phase-specific gate test | Test | 2h |
| TODO-029 | crates/clawdius-core/src/nexus/transition.rs | 202 | Implement full transition validation test | Test | 2h |
| TODO-030 | crates/clawdius-core/src/nexus/transition.rs | 208 | Implement rollback test | Test | 2h |
| TODO-031 | crates/clawdius-core/src/nexus/engine.rs | 274 | Implement full transition test | Test | 3h |

---

## Items by Category

### Feature (6 items)

| ID | File | Line | Text | Priority | Effort |
|----|------|------|------|----------|--------|
| TODO-001 | artifacts.rs | 144 | Add SQLite connection pool | High | 4h |
| TODO-002 | artifacts.rs | 146 | Add LRU cache | High | 3h |
| TODO-003 | artifacts.rs | 154 | Initialize SQLite database | High | 4h |
| TODO-004 | artifacts.rs | 155 | Create schema | High | 2h |
| TODO-005 | events.rs | 275 | Add metrics storage | High | 3h |
| TODO-006 | events.rs | 293 | Add database connection for audit storage | High | 3h |

### Test (25 items)

All test-related TODOs are in the Medium priority section above.

---

## Intentional Template Strings (Not Actionable)

These TODO comments are part of code generation templates and should remain as-is:

### cli.rs - Test Generation Templates (9 items)
| Lines | Purpose |
|-------|---------|
| 1049, 1054, 1059 | Rust test template placeholders |
| 1064, 1068, 1072 | TypeScript/JavaScript test template placeholders |
| 1079, 1083, 1087 | Python test template placeholders |

### actions/tests.rs - Test Generation Templates (9 items)
| Lines | Purpose |
|-------|---------|
| 392, 397, 402 | Rust test template placeholders |
| 409, 413, 417 | TypeScript/JavaScript test template placeholders |
| 426, 430, 434 | Python test template placeholders |

### completion.rs - Code Completion Snippets (9 items)
| Lines | Purpose |
|-------|---------|
| 204, 206, 209 | Rust completion placeholders |
| 227, 229, 231 | Python completion placeholders |
| 242, 244 | JavaScript/TypeScript completion placeholders |
| 254 | Go completion placeholder |

### completion_handler_integration.rs - Test Assertions (7 items)
Lines 49, 61, 85, 133, 145, 169, 304 contain assertions checking for "TODO" in generated output - these are test validation, not actual TODOs.

---

## Effort Estimation

| Priority | Total Items | Estimated Hours |
|----------|-------------|-----------------|
| High | 6 | 19h |
| Medium | 25 | 56h |
| **Total** | **31** | **75h** |

---

## Recommended Resolution Order

### Phase 1: Core Infrastructure (High Priority)
Resolve these first to enable the Nexus FSM to function properly:

1. **TODO-003** - Initialize SQLite database (prerequisite for TODO-001, TODO-004)
2. **TODO-004** - Create schema (depends on TODO-003)
3. **TODO-001** - Add SQLite connection pool (depends on TODO-003, TODO-004)
4. **TODO-002** - Add LRU cache (independent)
5. **TODO-005** - Add metrics storage (independent)
6. **TODO-006** - Add database connection for audit storage (independent)

### Phase 2: Unit Tests (Medium Priority)
Implement tests for completed infrastructure:

7. **TODO-022** - Implement storage test with temp directory
8. **TODO-023** - Implement retrieval test
9. **TODO-024** - Implement dependency tracking test
10. **TODO-025** - Implement metrics handler test
11. **TODO-026** - Implement audit handler test
12. **TODO-027** - Implement full gate evaluation test
13. **TODO-028** - Implement phase-specific gate test
14. **TODO-029** - Implement full transition validation test
15. **TODO-030** - Implement rollback test
16. **TODO-031** - Implement full transition test

### Phase 3: Integration Tests (Medium Priority)
Comprehensive testing of the complete system:

17. **TODO-015** - Test engine initialization
18. **TODO-016** - Test artifact operations
19. **TODO-017** - Test gate operations
20. **TODO-018** - Test event operations
21. **TODO-007** - Test complete lifecycle from Phase 0 to Phase 23
22. **TODO-008** - Test that invalid transitions are rejected
23. **TODO-009** - Test artifact dependency tracking
24. **TODO-010** - Test quality gate system
25. **TODO-011** - Test event bus with real subscribers
26. **TODO-012** - Test transition rollback
27. **TODO-013** - Test thread safety
28. **TODO-014** - Test persistence

### Phase 4: Property-Based Tests (Medium Priority)
Add property-based testing for robustness:

29. **TODO-019** - Implement proptest for artifact hash consistency
30. **TODO-020** - Implement proptest for phase transition validity
31. **TODO-021** - Implement proptest for artifact ID uniqueness

---

## Notes

1. **No Critical Items**: All TODOs are either infrastructure features or test implementations. No blocking bugs or security issues were found.

2. **Template Strings**: The 34 template string TODOs are intentional and should NOT be removed. They serve as placeholders in generated code templates.

3. **Dependencies**: The artifact storage TODOs (TODO-001 through TODO-004) have dependencies and should be resolved in order.

4. **Testing Strategy**: Tests are marked with `#[ignore]` attribute, indicating they are skeleton tests waiting for implementation. The infrastructure should be completed first.

5. **Related `todo!()` Macros**: Several files also contain `todo!()` macro calls that will panic at runtime:
   - `artifacts.rs:162` - "Implement artifact storage"
   - `artifacts.rs:165` - "Implement artifact retrieval"
   - `artifacts.rs:168` - "Implement artifact deletion"
   - `events.rs:286` - "Implement metrics collection"
   
   These are tracked separately from TODO comments but represent the same work items.

---

## Related Reports

- [TODO Cleanup Report](.reports/TODO_CLEANUP_REPORT.md)
- [TODO Cleanup Final Summary](.reports/TODO_CLEANUP_FINAL_SUMMARY.md)
- [Complete Status v0.8.0-alpha](.reports/COMPLETE_STATUS_v0.8.0-alpha.md)
