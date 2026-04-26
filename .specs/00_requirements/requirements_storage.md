# Requirements Specification: Storage Abstraction Layer

## Document Header

| Attribute | Value |
|-----------|-------|
| **Document ID** | REQ-STORAGE-001 |
| **Version** | 1.0.0 |
| **Status** | APPROVED |
| **Created** | 2026-04-26 |
| **Author** | Nexus |
| **Traceability** | ROADMAP-001 Phase A |

---

## 1. Functional Requirements

### 1.1 Storage Backend Trait

#### REQ-STOR-001: Pluggable Storage Backend
**EARS Pattern:** Ubiquitous

The system SHALL provide a `StorageBackend` trait that abstracts all persistent
storage operations. Concrete implementations SHALL exist for SQLite, PostgreSQL,
MariaDB, and InMemory (testing).

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-001 |
| **Priority** | MUST |
| **Verification** | Unit Test + Integration Test |

**Acceptance Criteria:**
- [ ] `StorageBackend` trait defined with all CRUD operations
- [ ] `SqliteBackend` passes all trait tests
- [ ] `PostgresBackend` passes all trait tests
- [ ] `MariaDbBackend` passes all trait tests
- [ ] `InMemoryBackend` passes all trait tests
- [ ] Backend selected via configuration (`.clawdius.toml`)
- [ ] Backend swap requires zero code changes to consumers

---

#### REQ-STOR-002: Schema Migrations
**EARS Pattern:** Ubiquitous

Each storage backend SHALL support automated schema migrations on startup.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-002 |
| **Priority** | MUST |
| **Verification** | Integration Test |

**Acceptance Criteria:**
- [ ] Fresh database: all tables created
- [ ] Existing database: only new tables/columns added
- [ ] Migration is idempotent (safe to run multiple times)
- [ ] Migration failures are rolled back
- [ ] Schema version tracked in metadata table

---

### 1.2 Session Storage

#### REQ-STOR-010: Session CRUD
**EARS Pattern:** Ubiquitous

The storage backend SHALL support full CRUD operations for sessions including
creation, retrieval, update, deletion, and filtered listing.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-010 |
| **Priority** | MUST |
| **Verification** | Unit Test |

**Acceptance Criteria:**
- [ ] Create session with all metadata fields
- [ ] Get session by ID
- [ ] List sessions filtered by date, tags, project
- [ ] Update session metadata (title, tags, token counts)
- [ ] Delete session cascades to messages and checkpoints
- [ ] Session listing supports pagination (offset/limit)

---

#### REQ-STOR-011: Message Persistence
**EARS Pattern:** Ubiquitous

All chat messages SHALL be persisted with session association, role, content,
token counts, and tool call metadata.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-011 |
| **Priority** | MUST |
| **Verification** | Unit Test |

**Acceptance Criteria:**
- [ ] Append message to session
- [ ] Retrieve messages in chronological order
- [ ] Retrieve messages with limit (for context window)
- [ ] Message content preserves tool call JSON
- [ ] Token counts stored per message

---

### 1.3 Workspace Storage

#### REQ-STOR-020: Workspace CRUD
**EARS Pattern:** Ubiquitous

The storage backend SHALL support workspace management including creation,
retrieval, listing, and project association.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-020 |
| **Priority** | MUST |
| **Verification** | Unit Test |

**Acceptance Criteria:**
- [ ] Create workspace with name and default project
- [ ] Get workspace by ID
- [ ] List all workspaces for a tenant
- [ ] Add project to workspace (N projects per workspace)
- [ ] Remove project from workspace
- [ ] Set default project for workspace

---

#### REQ-STOR-021: Project Metadata
**EARS Pattern:** Ubiquitous

Each project SHALL store its root path, detected language, and sandbox configuration.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-021 |
| **Priority** | MUST |
| **Verification** | Unit Test |

**Acceptance Criteria:**
- [ ] Project stores root_path as absolute path
- [ ] Project stores auto-detected language (via tree-sitter)
- [ ] Project stores sandbox configuration (JSON)
- [ ] Project path validated to exist on filesystem

---

### 1.4 Tenant Storage

#### REQ-STOR-030: Tenant Management
**EARS Pattern:** Ubiquitous

The storage backend SHALL support multi-tenant CRUD for SaaS deployments.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-030 |
| **Priority** | MUST |
| **Verification** | Unit Test |

**Acceptance Criteria:**
- [ ] Create tenant with tier (Free/Pro/Enterprise)
- [ ] Get tenant by ID
- [ ] Update tenant tier and limits
- [ ] Tenant isolation enforced (no cross-tenant data access)

---

#### REQ-STOR-031: Audit Logging
**EARS Pattern:** Ubiquitous

All state-changing operations SHALL be logged to an audit trail.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-031 |
| **Priority** | MUST |
| **Verification** | Integration Test |

**Acceptance Criteria:**
- [ ] Audit entries include timestamp, actor, action, resource, result
- [ ] Audit log queryable by tenant, user, time range, action type
- [ ] Audit log append-only (no deletion or modification)
- [ ] Audit log supports export (JSON, CSV)

---

## 2. Non-Functional Requirements

### 2.1 Performance

#### REQ-STOR-040: Latency Targets
**EARS Pattern:** Ubiquitous

| Operation | Target | Measurement |
|-----------|--------|-------------|
| Session create | < 5ms | Benchmark |
| Session get | < 1ms | Benchmark |
| Message append | < 2ms | Benchmark |
| Message list (100) | < 5ms | Benchmark |
| Workspace list | < 5ms | Benchmark |

---

### 2.2 Compatibility

#### REQ-STOR-050: SQLite Compatibility
**EARS Pattern:** Ubiquitous

SQLite backend SHALL use rusqlite and support SQLite 3.39+.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-050 |
| **Priority** | MUST |
| **Verification** | Integration Test |

**Acceptance Criteria:**
- [ ] Works with system SQLite (3.39+)
- [ ] Works with bundled SQLite (via rusqlite features)
- [ ] WAL mode enabled for concurrent reads
- [ ] Single-file database (no external server)

---

#### REQ-STOR-051: PostgreSQL Compatibility
**EARS Pattern:** Ubiquitous

PostgreSQL backend SHALL use sqlx and support PostgreSQL 15+.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-051 |
| **Priority** | MUST |
| **Verification** | Integration Test |

**Acceptance Criteria:**
- [ ] Connection pooling via sqlx Pool
- [ ] Supports both TCP and Unix socket connections
- [ ] SSL/TLS connections supported
- [ ] Migrations use sqlx query builder (no external migration tool required)

---

#### REQ-STOR-052: MariaDB Compatibility
**EARS Pattern:** Ubiquitous

MariaDB backend SHALL use sqlx and support MariaDB 10.11+ and MySQL 8.0+.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-052 |
| **Priority** | MUST |
| **Verification** | Integration Test |

**Acceptance Criteria:**
- [ ] Connection pooling via sqlx Pool
- [ ] SQL dialect compatibility (LIMIT vs FETCH FIRST handled)
- [ ] SSL/TLS connections supported

---

## 3. Configuration

#### REQ-STOR-060: Backend Selection
**EARS Pattern:** State-driven

When the user configures `.clawdius.toml`, the system SHALL select the appropriate
storage backend based on the `database.backend` field.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-STOR-060 |
| **Priority** | MUST |
| **Verification** | Unit Test |

**Acceptance Criteria:**
- [ ] `backend = "sqlite"` creates `SqliteBackend`
- [ ] `backend = "postgresql"` creates `PostgresBackend`
- [ ] `backend = "mariadb"` creates `MariaDbBackend`
- [ ] Missing or invalid backend falls back to SQLite with warning
- [ ] Default (no config) uses SQLite at `~/.local/share/clawdius/clawdius.db`

---

## 4. Traceability Matrix

| Requirement ID | Component | Test Case | Standard |
|----------------|-----------|-----------|----------|
| REQ-STOR-001 | StorageBackend trait | TC-STOR-001 | IEEE 1016 |
| REQ-STOR-002 | All backends | TC-STOR-002 | ISO/IEC 12207 |
| REQ-STOR-010 | Session operations | TC-STOR-010 | IEEE 1016 |
| REQ-STOR-011 | Message operations | TC-STOR-011 | IEEE 1016 |
| REQ-STOR-020 | Workspace operations | TC-STOR-020 | IEEE 1016 |
| REQ-STOR-021 | Project operations | TC-STOR-021 | IEEE 1016 |
| REQ-STOR-030 | Tenant operations | TC-STOR-030 | IEEE 1016 |
| REQ-STOR-031 | Audit operations | TC-STOR-031 | ISO/IEC 27001 |
| REQ-STOR-040 | All backends | TC-STOR-040 | IEEE 1016 |
| REQ-STOR-050 | SqliteBackend | TC-STOR-050 | IEEE 1016 |
| REQ-STOR-051 | PostgresBackend | TC-STOR-051 | IEEE 1016 |
| REQ-STOR-052 | MariaDbBackend | TC-STOR-052 | IEEE 1016 |
| REQ-STOR-060 | Config loading | TC-STOR-060 | IEEE 1016 |

---

## 5. Document Status

| Quality Gate | Status |
|---------------|--------|
| Requirements Complete | ✅ |
| Acceptance Criteria Defined | ✅ |
| Traceability Established | ✅ |
| Stakeholder Review | ⏳ Pending |
