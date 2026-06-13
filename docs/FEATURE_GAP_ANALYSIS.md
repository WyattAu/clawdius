# Clawdius Feature Gap Analysis & Implementation Plan

> Competitive audit, requirements specification, and execution roadmap for closing
> feature gaps against 22 competitors. Based on empirical codebase audit conducted
> 2026-06-12.
>
> Clawdius version: v1.0.0 (post-session 6, 2,606 tests, 318 Lean4 theorems)

---

## Part I: Feature Reality Assessment

### Methodology

Every claimed feature was verified by reading actual source code, counting lines of
real implementation logic (excluding tests, comments, blank lines), and assessing
functional completeness. Features are classified as:

| Classification | Definition |
|---|---|
| **PRODUCTION** | Fully functional, tested, ready for user deployment |
| **FUNCTIONAL** | Works but has known limitations or missing edge cases |
| **STUB** | Types/scaffolding exist, no real logic |
| **NONEXISTENT** | Zero lines of implementation code |

### Results

| Feature | Claimed | Actual | Classification | Lines | Blocker |
|---|---|---|---|---|---|
| Graph-RAG code intelligence | Yes | Works | FUNCTIONAL | ~1,400 | Embedding model is toy (SimpleEmbedder) |
| Tree-sitter parsing (5+ langs) | 5 langs | 10 langs | PRODUCTION | ~1,000 | None |
| LSP server | Yes | Works | FUNCTIONAL | ~990 | Pattern-matching, not tree-sitter; no completions/code actions |
| MCP server | Yes | Works | PRODUCTION | ~1,600 | No MCP client |
| MCP client | Yes | No | NONEXISTENT | 0 | Not started |
| WASM plugin runtime | Yes | Works | FUNCTIONAL | ~698 | No memory limit enforcement, no timeout |
| gVisor sandbox backend | Planned | Stub | STUB | ~20 | Delegates to Docker |
| Firecracker sandbox backend | Planned | Stub | STUB | 0 | Delegates to Docker |
| Bubblewrap sandbox | Yes | Works | PRODUCTION | ~117 | None |
| Container sandbox (Docker) | Yes | Works | PRODUCTION | ~150 | None |
| Filtered command sandbox | Yes | Works | PRODUCTION | ~50 | None |
| sandbox-exec (macOS) | Yes | Works | PRODUCTION | ~80 | macOS only |
| SSO/SAML 2.0 | Yes | No | NONEXISTENT | 0 | Not started |
| SSO/OIDC | Yes | No | NONEXISTENT | 0 | Not started |
| Okta integration | Yes | No | NONEXISTENT | 0 | Not started |
| Azure AD integration | Yes | No | NONEXISTENT | 0 | Not started |
| GitHub SSO | Yes | No | NONEXISTENT | 0 | Not started |
| Audit logging (5 backends) | Yes | In-memory | STUB | ~340 | Only Vec<AuditEntry>, no persistence |
| Multi-tenant support | Yes | Works | FUNCTIONAL | ~3,500 | In-memory only, no DB persistence |
| Encryption at rest (AES-256-GCM) | Yes | Works | PRODUCTION | ~637 | None |
| Distributed LLM routing | Planned | Types only | STUB | ~808 | No network transport |
| OAuth2 proxy | No | No | NONEXISTENT | 0 | Not started |
| Cosign/container signing | No | No | NONEXISTENT | 0 | GPG only for archives |
| SBOM generation | Yes | Works | PRODUCTION | CycloneDX in CI | None |
| Web admin dashboard | Partial | REST API | FUNCTIONAL | ~665 | No web UI |
| Real-time collaboration | No | No | NONEXISTENT | 0 | Not started |
| Code completion | No | No | NONEXISTENT | 0 | Not started |
| Inline code suggestions | No | No | NONEXISTENT | 0 | Not started |
| Image/binary file support | No | No | NONEXISTENT | 0 | Not started |
| Voice input | No | No | NONEXISTENT | 0 | Not started |
| Multi-agent orchestration | Partial | Sprint engine | FUNCTIONAL | ~2,000 | No autonomous multi-agent |
| Continuous monitoring/alerting | No | No | NONEXISTENT | 0 | Not started |
| Rate limiting | Yes | Works | PRODUCTION | Per-user/platform | None |
| Feature flags | 15+ | Works | PRODUCTION | capability.rs | None |
| OS keyring storage | Yes | Works | PRODUCTION | keyring crate | None |
| Secret redaction | Yes | Works | PRODUCTION | mask_api_keys | None |
| Path traversal guard | Yes | Works | PRODUCTION | canonical paths | None |

---

## Part II: Updated Comparison Matrix (Gap-Focused)

### Features Where Clawdius LEADS (No Gap)

| Dimension | Clawdius | Nearest Competitor | Gap |
|---|---|---|---|
| Formal verification | 318 Lean4 theorems | None | INFINITE |
| Sandboxing | 3 functional + 2 planned | OpenHands (Docker only) | +2 backends |
| Messaging adapters | 9 platforms | None (all CLI/IDE) | INFINITE |
| Encryption at rest | AES-256-GCM + HKDF | None | INFINITE |
| LLM providers | 9+ | Aider/Claw Code (~8) | +1 |
| Tree-sitter langs | 10 | Zed (native) | Comparable |
| WASM plugins | Wasmtime runtime | None | INFINITE |
| Feature flags | 15+ | None | INFINITE |
| Cold boot | <20ms | Amp (~15ms) | Comparable |

### Features Where Clawdius LAGS (Gaps to Close)

| Dimension | Clawdius | Competitor Best | Gap Severity |
|---|---|---|---|
| SSO/Identity | NONEXISTENT | Devin/Cursor (SAML+OIDC+Okta+Azure AD) | CRITICAL |
| Audit logging | In-memory only | Devin (full audit trail) | HIGH |
| LSP completeness | documentSymbol only | Cursor/Zed (full LSP) | HIGH |
| MCP client | NONEXISTENT | Claude Code/Cline/Goose (client) | MEDIUM |
| Container signing | GPG archives only | Evergreen (cosign+SLSA+SBOM attestation) | MEDIUM |
| Web dashboard | REST API only | Devin/Replit (full cloud IDE) | MEDIUM |
| Code completion | NONEXISTENT | Copilot/Cursor/Tabnine | MEDIUM |
| Collaboration | NONEXISTENT | Zed (CRDT-based) | LOW |
| gVisor/Firecracker | Stub | Devin (cloud VM) | LOW |
| Distributed routing | Types only | Claw Code (multi-agent) | LOW |
| Image/multimodal input | NONEXISTENT | Claude Code/Cursor/GPT-4o | LOW |
| Voice input | NONEXISTENT | None significant | LOW |

### Competitor-Specific Gap Analysis

#### vs. Claude Code (Anthropic)

| Feature | Claude Code | Clawdius | Action Required |
|---|---|---|---|
| Single-provider optimization | Best Claude integration | Multi-provider | Advantage: Clawdius |
| IDE integration | VSCode + JetBrains native | LSP only | Add VSCode extension polish |
| MCP client | Built-in | None | Implement MCP client |
| Agentic depth | Deepest Claude reasoning | Comparable | No gap |
| Auto-compact context | Yes | Yes | No gap |
| Git workflow awareness | Yes | Yes | No gap |

#### vs. Cursor

| Feature | Cursor | Clawdius | Action Required |
|---|---|---|---|
| IDE-native experience | Best-in-class | LSP only | Add inline suggestions |
| Code completion | Real-time, multi-line | None | Implement completion provider |
| Cursor prediction | Patented | None | Not replicable |
| Multi-file editing | Real-time | Via agentic loop | Different paradigm |
| User base | Largest | Growing | Marketing gap |

#### vs. Devin

| Feature | Devin | Clawdius | Action Required |
|---|---|---|---|
| Fully autonomous | Cloud VM, no local install | Local only | Add web dashboard |
| SSO/Enterprise | SAML+OIDC+Okta | None | Implement SSO |
| Audit logging | Full trail | In-memory | Implement audit backends |
| Pricing | $500/mo | Free (bring key) | Advantage: Clawdius |
| Self-hosted | No | Yes | Advantage: Clawdius |

#### vs. Aider

| Feature | Aider | Clawdius | Action Required |
|---|---|---|---|
| Git-integrated workflow | Auto-commits per edit | Manual | Add auto-commit mode |
| Python ecosystem | Deep pip integration | None | N/A (Rust) |
| Model compatibility | ~8 providers | 9+ | Advantage: Clawdius |
| Star count | ~35K | ~500 | Marketing/community gap |
| Formal verification | None | 318 theorems | Advantage: Clawdius |

#### vs. Cline

| Feature | Cline | Clawdius | Action Required |
|---|---|---|---|
| VSCode extension | Native, polished | Scaffold only | Polish extension |
| MCP client | Built-in | None | Implement MCP client |
| Permission model | Per-action prompts | Per-action prompts | Comparable |
| Open source | Apache 2.0 | Apache 2.0 | Comparable |

#### vs. OpenHands

| Feature | OpenHands | Clawdius | Action Required |
|---|---|---|---|
| Docker sandbox | Yes | Yes (container backend) | No gap |
| Web UI | Yes (React) | REST API only | Add web dashboard |
| Star count | ~45K | ~500 | Community gap |
| Language | Python | Rust | Advantage: Clawdius (perf) |

---

## Part III: Requirements Specification

### Priority Matrix

Requirements are ordered by competitive impact and implementation feasibility.
Each requirement has:
- **Priority**: P0 (blocks enterprise sales), P1 (blocks competitive parity), P2 (advantage)
- **Effort**: Small (<1 week), Medium (1-4 weeks), Large (1-3 months)
- **Impact**: Revenue, adoption, or differentiation gain

### P0 Requirements (Blocks Enterprise Adoption)

#### REQ-001: SSO/SAML 2.0 and OIDC Authentication

| Attribute | Value |
|---|---|
| Priority | P0 |
| Effort | Large |
| Impact | Unlocks enterprise sales |
| Competitors | Devin, Cursor, Windsurf, Augment (all have SSO) |
| Status | NONEXISTENT (0 lines) |

**Requirements:**

1. SAML 2.0 Service Provider implementation
   - Accept SAML assertions from IdP (Okta, Azure AD, OneLogin)
   - SP-initiated and IdP-initiated SSO flows
   - SAML metadata endpoint (`/saml/metadata`)
   - ACS endpoint (`/saml/acs`)
   - SLO (Single Logout) support
   - Signature verification (XML-DSig)

2. OIDC Relying Party implementation
   - Authorization Code flow with PKCE
   - Token validation (JWT verification, claims checking)
   - Discovery endpoint support (`/.well-known/openid-configuration`)
   - Support: Okta, Azure AD, Google Workspace, GitHub, GitLab

3. Session management
   - Secure session tokens (JWT or opaque)
   - Refresh token rotation
   - Session duration configurable per tenant
   - Concurrent session limits

4. User provisioning
   - SCIM 2.0 user provisioning (Okta, Azure AD)
   - JIT provisioning on first SSO login
   - Group/role mapping from IdP claims

5. MFA enforcement
   - Enforce IdP-side MFA (pass-through)
   - Optional TOTP fallback for local accounts

**Technical specification:**

```rust
// New crate: crates/clawdius-auth/
// Dependencies: openidconnect, saml-rs (or xmlsec), jsonwebtoken

pub trait IdentityProvider: Send + Sync {
    async fn authenticate(&self, credentials: AuthRequest) -> Result<AuthResult>;
    async fn validate_token(&self, token: &str) -> Result<UserInfo>;
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenPair>;
    async fn revoke_token(&self, token: &str) -> Result<()>;
}

pub struct SamlProvider { /* SP config, certificate, IdP metadata */ }
pub struct OidcProvider { /* client_id, client_secret, discovery_url */ }

pub struct AuthService {
    providers: HashMap<String, Arc<dyn IdentityProvider>>,
    session_store: Arc<dyn SessionStore>,
    user_store: Arc<dyn UserStore>,
}
```

**Files to create:**

| File | Purpose |
|---|---|
| `crates/clawdius-auth/Cargo.toml` | New crate definition |
| `crates/clawdius-auth/src/lib.rs` | Auth traits and types |
| `crates/clawdius-auth/src/saml.rs` | SAML 2.0 SP implementation |
| `crates/clawdius-auth/src/oidc.rs` | OIDC RP implementation |
| `crates/clawdius-auth/src/session.rs` | Session management |
| `crates/clawdius-auth/src/scim.rs` | SCIM 2.0 provisioning |
| `crates/clawdius-auth/src/middleware.rs` | Axum auth middleware |
| `crates/clawdius-auth/src/routes.rs` | Auth HTTP endpoints |

---

#### REQ-002: Audit Logging with Persistent Backends

| Attribute | Value |
|---|---|
| Priority | P0 |
| Effort | Medium |
| Impact | Required for SOC 2, HIPAA compliance |
| Competitors | Devin (full audit), Cursor (basic) |
| Status | STUB (in-memory only, ~340 lines) |

**Requirements:**

1. Audit event schema
   - Timestamp, actor, action, resource, outcome, IP, user-agent
   - Structured JSON format (ECS or custom)
   - Hash chain for tamper detection (already exists in AgentAuditLog)

2. Five backend implementations
   - **File**: JSON Lines append-only log with rotation (size + time)
   - **SQLite**: Dedicated audit database with indexes on timestamp, actor, action
   - **Elasticsearch/OpenSearch**: Bulk indexing with index lifecycle management
   - **Webhook**: HTTP POST to configurable endpoint with retry + backoff
   - **Syslog**: RFC 5424 format over TCP/TLS

3. Retention policies
   - Configurable per-backend retention period
   - Automatic rotation and cleanup
   - Compliance minimums (SOC 2: 90 days, HIPAA: 6 years)

4. Query API
   - REST endpoints for audit log retrieval
   - Filter by actor, action, resource, time range
   - Export to CSV/JSON

**Technical specification:**

```rust
// Extend existing: crates/clawdius-core/src/audit/

pub trait AuditBackend: Send + Sync {
    async fn write(&self, entry: &AuditEntry) -> Result<()>;
    async fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEntry>>;
    async fn health_check(&self) -> Result<()>;
}

pub struct FileBackend { path: PathBuf, rotation: RotationPolicy }
pub struct SqliteBackend { pool: SqlitePool }
pub struct ElasticsearchBackend { client: ElasticClient, index_prefix: String }
pub struct WebhookBackend { url: Url, client: reqwest::Client }
pub struct SyslogBackend { target: SocketAddr, tls: bool }

pub struct AuditLogger {
    backends: Vec<Arc<dyn AuditBackend>>,
    chain: HashChain, // existing hash chain from AgentAuditLog
}
```

---

#### REQ-003: Container Image Signing and Supply Chain Security

| Attribute | Value |
|---|---|
| Priority | P0 |
| Effort | Small |
| Impact | Required for enterprise procurement |
| Competitors | Evergreen (full cosign+SLSA+SBOM) |
| Status | NONEXISTENT (GPG archive signing only) |

**Requirements:**

1. Cosign signing for all container images
   - Keyless signing with Sigstore (Fulcio + Rekor)
   - Or bring-your-own key (KMS-supported)
   - Sign on every Docker image push in CI

2. SLSA provenance attestation
   - SLSA Level 3 provenance for release artifacts
   - GitHub Actions native provenance

3. SBOM attestation
   - Attach CycloneDX SBOM to container images
   - Already generating SBOM via cargo-cyclonedx; need to attach to image

4. Verification tooling
   - `clawdius verify-image` command to check signatures
   - Policy engine to enforce signed-only images

**Technical specification:**

```yaml
# Add to .github/workflows/docker.yml
- name: Sign container image
  uses: sigstore/cosign-installer@v3
- run: cosign sign --yes ghcr.io/wyattau/clawdius:${{ github.sha }}

- name: Attach SBOM
  run: cosign attach sbom --sbom sbom.cdx.json ghcr.io/wyattau/clawdius:${{ github.sha }}

- name: SLSA provenance
  uses: slsa-framework/slsa-github-generator/.github/workflows/generator_container_slsa3.yml@v2
```

---

### P1 Requirements (Competitive Parity)

#### REQ-004: MCP Client Implementation

| Attribute | Value |
|---|---|
| Priority | P1 |
| Effort | Medium |
| Impact | Interoperability with MCP ecosystem |
| Competitors | Claude Code, Cline, Goose (all have MCP client) |
| Status | NONEXISTENT (server works, no client) |

**Requirements:**

1. MCP client transport
   - stdio transport (launch MCP server as subprocess)
   - HTTP/SSE transport (connect to remote MCP server)
   - Bidirectional message handling

2. Tool discovery and invocation
   - List tools from connected MCP servers
   - Invoke tools with schema validation
   - Stream tool results back to agent

3. Resource subscriptions
   - Subscribe to resource changes
   - Handle resource update notifications

4. Integration with agentic engine
   - MCP tools appear alongside native tools in tool_use dispatch
   - Permission model applies to MCP tools
   - Rate limiting per MCP server

**Technical specification:**

```rust
// New module: crates/clawdius-core/src/mcp/client.rs

pub struct McpClient {
    transport: Box<dyn McpTransport>,
    server_info: ServerInfo,
    tools: Vec<ToolDefinition>,
}

pub trait McpTransport: Send + Sync {
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse>;
    async fn send_notification(&self, notification: JsonRpcNotification) -> Result<()>;
}

pub struct StdioTransport { child: Child, stdin: BufWriter, stdout: BufReader }
pub struct HttpTransport { client: reqwest::Client, url: Url }

// Integration point: crates/clawdius-core/src/agentic/tool_executor.rs
// McpClient tools merged into the tool dispatch table
```

---

#### REQ-005: LSP Server Enhancement (Full Feature Set)

| Attribute | Value |
|---|---|
| Priority | P1 |
| Effort | Large |
| Impact | IDE integration quality |
| Competitors | Cursor, Zed, VSCode Copilot (full LSP) |
| Status | FUNCTIONAL but basic (~990 lines, pattern-matching) |

**Requirements:**

1. Replace pattern-matching with tree-sitter parsing
   - Use the existing 10-language parser from `workspace/indexer.rs`
   - Generate accurate `documentSymbol` responses from AST
   - Support nested symbols, ranges, selection ranges

2. Implement `textDocument/completion`
   - Symbol-based completions from index
   - Keyword completions per language
   - Snippet completions for common patterns

3. Implement `textDocument/codeAction`
   - Quick fixes from LSP diagnostics
   - Refactoring actions (rename, extract)
   - Code generation actions

4. Implement `textDocument/diagnostics`
   - Pull diagnostics (LSP 3.17)
   - Architecture drift detection (from proof system)
   - Dead code detection
   - Complexity warnings

5. Implement `textDocument/rename`
   - Project-wide rename via symbol index
   - Preview before apply

6. Workspace operations
   - `workspace/symbol` -- project-wide symbol search
   - `workspace/executeCommand` -- custom Clawdius commands

**Technical specification:**

```rust
// Upgrade: crates/clawdius-lsp/src/symbol_index.rs
// Replace regex patterns with:

use crate::tree_sitter_parser::CodeParser; // from clawdius-core

pub struct TreeSitterIndex {
    parser: CodeParser,
    symbol_cache: DashMap<Url, Vec<DocumentSymbol>>,
}
```

---

#### REQ-006: Web Admin Dashboard

| Attribute | Value |
|---|---|
| Priority | P1 |
| Effort | Large |
| Impact | Enterprise self-service, competitive parity with Devin/Replit |
| Competitors | Devin (cloud IDE), OpenHands (React UI) |
| Status | REST API exists (~665 lines), no frontend |

**Requirements:**

1. Technology choice
   - Leptos (Rust WASM) for consistency with Rust ecosystem
   - Or React/Next.js for faster ecosystem access
   - Recommendation: Leptos 0.7 with server functions

2. Dashboard pages
   - **Overview**: System health, active sessions, token usage
   - **Tenants**: CRUD, subscription management, API keys
   - **Sessions**: Active sessions, history, playback
   - **Usage**: Token consumption charts, cost breakdown
   - **Security**: Audit log viewer, active threats, sandbox status
   - **Settings**: LLM provider config, feature flags, SSO config

3. Real-time updates
   - WebSocket for live session monitoring
   - SSE for usage metric streaming
   - Live log tailing

4. Responsive design
   - Mobile-friendly layout
   - WCAG 2.1 AA compliance
   - Spatial Materialism design language (consistent with landing page)

**Technical specification:**

```
crates/clawdius-dashboard/
  Cargo.toml
  src/
    lib.rs          # Leptos app root
    app.rs          # Router and layout
    pages/
      overview.rs   # Dashboard home
      tenants.rs    # Tenant management
      sessions.rs   # Session monitoring
      usage.rs      # Usage analytics
      security.rs   # Audit log viewer
      settings.rs   # Configuration
    components/
      chart.rs      # Usage charts
      table.rs      # Data tables
      nav.rs        # Navigation
    server/
      mod.rs        # Leptos server functions
      auth.rs       # Dashboard auth
```

---

#### REQ-007: gVisor and Firecracker Sandbox Backends

| Attribute | Value |
|---|---|
| Priority | P1 |
| Effort | Medium |
| Impact | Completes sandbox story for marketing |
| Competitors | Devin (cloud VM), OpenHands (Docker) |
| Status | STUB (~20 lines each) |

**Requirements:**

1. gVisor backend
   - Detect `runsc` runtime availability
   - Generate gVisor-specific seccomp profiles
   - Configure network sandboxing rules
   - Support custom OCI runtime flags
   - Fallback to container backend if `runsc` not available

2. Firecracker backend
   - Launch microVM via Firecracker binary
   - Configure jailer (seccomp, cgroups, namespaces)
   - MMDS for metadata service
   - Root filesystem preparation (ext4, minimal)
   - Configurable CPU/memory for microVM
   - Graceful shutdown and cleanup
   - Fallback to container backend if Firecracker not available

**Technical specification:**

```rust
// Upgrade: crates/clawdius-core/src/sandbox/backends/gvisor.rs
pub struct GvisorBackend {
    runsc_path: PathBuf,
    seccomp_profile: Option<PathBuf>,
    network_mode: GvisorNetwork,
}

// Upgrade: crates/clawdius-core/src/sandbox/backends/firecracker.rs
pub struct FirecrackerBackend {
    firecracker_bin: PathBuf,
    jailer_bin: PathBuf,
    default_cpus: u32,
    default_mem_mb: u32,
    rootfs_template: PathBuf,
}
```

---

### P2 Requirements (Competitive Advantage)

#### REQ-008: Distributed LLM Routing (Real Implementation)

| Attribute | Value |
|---|---|
| Priority | P2 |
| Effort | Large |
| Impact | Enterprise multi-region deployments |
| Competitors | None (unique) |
| Status | Types/scaffolding only (~808 lines) |

**Requirements:**

1. Network transport layer
   - gRPC-based inter-node communication
   - TLS mutual authentication
   - Service discovery via DNS or consul

2. Load balancing strategies
   - Round-robin (exists as type)
   - Least-connections (exists as type)
   - Latency-aware (exists as type)
   - Cost-aware (new: route to cheapest provider)
   - Fallback chain (primary -> secondary -> emergency)

3. Health checking
   - Periodic health probes
   - Circuit breaker per provider
   - Automatic failover

4. Integration with existing LLM layer
   - Transparent routing (agent doesn't know it's distributed)
   - Token usage aggregation across nodes
   - Rate limit coordination

---

#### REQ-009: Continuous Monitoring and Alerting

| Attribute | Value |
|---|---|
| Priority | P2 |
| Effort | Medium |
| Impact | Operations maturity |
| Competitors | None (unique for self-hosted) |
| Status | NONEXISTENT |

**Requirements:**

1. Metrics collection
   - Prometheus-compatible metrics endpoint
   - Request latency histograms
   - Token usage gauges
   - Error rate counters
   - Sandbox execution metrics

2. Alerting rules
   - Token usage threshold alerts
   - Error rate spike detection
   - Sandbox escape attempt detection
   - Provider rate limit approaching
   - Memory/CPU resource alerts

3. Health dashboard
   - Grafana dashboard JSON template
   - Pre-configured panels for common metrics

---

#### REQ-010: Image and Multimodal Input

| Attribute | Value |
|---|---|
| Priority | P2 |
| Effort | Medium |
| Impact | Feature parity with Claude Code/Cursor |
| Competitors | Claude Code, Cursor, GPT-4o agents |
| Status | NONEXISTENT |

**Requirements:**

1. Image understanding
   - Accept image files as context
   - Base64 encoding for API submission
   - Support: PNG, JPEG, GIF, WebP, SVG
   - Models: Claude 3.5+ (vision), GPT-4o (vision), Gemini Pro Vision

2. Screenshot analysis
   - Capture terminal/output screenshots
   - UI/UX feedback from screenshots

3. Diagram interpretation
   - Architecture diagram understanding
   - Flow chart to code translation

---

## Part IV: Implementation Plan

### Phase Overview

```
Phase A: Enterprise Security Foundation        [Weeks 1-6]   P0
Phase B: Competitive Feature Parity            [Weeks 5-12]  P1
Phase C: Advantage Features                    [Weeks 10-18] P2
Phase D: Infrastructure and Deployment         [Weeks 1-18]  Continuous
```

### Phase A: Enterprise Security Foundation (P0)

#### Week 1-2: Audit Logging Backends (REQ-002)

| Day | Task | Deliverable |
|---|---|---|
| 1-2 | Design audit event schema + backend trait | `audit::AuditBackend` trait |
| 3 | Implement FileBackend | JSON Lines with rotation |
| 4 | Implement SqliteBackend | Dedicated audit DB |
| 5 | Implement WebhookBackend | HTTP POST with retry |
| 6 | Implement SyslogBackend | RFC 5424 over TCP |
| 7-8 | Implement ElasticsearchBackend | Bulk indexing |
| 9 | Wire backends into AuditLogger | Config-driven selection |
| 10 | Add query API to gateway | REST endpoints for audit retrieval |
| 11-12 | Write tests | Target: 60+ tests across backends |
| 13 | Update admin.rs to expose audit endpoints | `/api/v1/audit/*` |

**Verification:** All 5 backends pass integration tests. Audit entries persist across restarts.

#### Week 3-4: SSO/SAML 2.0 and OIDC (REQ-001)

| Day | Task | Deliverable |
|---|---|---|
| 1-2 | Create `clawdius-auth` crate scaffold | Cargo.toml + lib.rs + traits |
| 3-5 | Implement OIDC RP (easier, more common) | `OidcProvider` with PKCE |
| 6-8 | Implement SAML 2.0 SP | `SamlProvider` with XML-DSig |
| 9 | Session management | JWT + refresh token rotation |
| 10 | Auth middleware for Axum | `AuthMiddleware` layer |
| 11 | User provisioning (JIT) | Auto-create users on first SSO |
| 12 | SCIM 2.0 endpoints | `/scim/v2/Users`, `/scim/v2/Groups` |
| 13-14 | Integration tests with mock IdP | Test with Dex or Keycloak |
| 15 | Documentation | SSO configuration guide |

**Dependencies:** `openidconnect` crate (Rust), `xmlsec` or pure-Rust SAML library.

**Verification:** SSO works with Keycloak (OIDC) and testshib.org (SAML). Users auto-provisioned.

#### Week 5: Container Signing (REQ-003)

| Day | Task | Deliverable |
|---|---|---|
| 1 | Add cosign to CI workflow | `docker.yml` signing step |
| 2 | Add SLSA provenance | SLSA3 generator workflow |
| 3 | Attach SBOM to images | `cosign attach sbom` |
| 4 | Add `clawdius verify-image` CLI command | Signature verification |
| 5 | Documentation | Supply chain security guide |

**Verification:** `cosign verify` succeeds on published images.

#### Week 6: Integration Testing and Documentation

| Day | Task | Deliverable |
|---|---|---|
| 1-2 | End-to-end SSO test suite | Okta + Azure AD test plans |
| 3 | Audit log compliance check | SOC 2 / HIPAA retention verified |
| 4 | Security whitepaper update | Add SSO + audit sections |
| 5 | Comparison matrix update | Reflect new capabilities |

### Phase B: Competitive Feature Parity (P1)

#### Week 5-7: MCP Client (REQ-004)

| Day | Task | Deliverable |
|---|---|---|
| 1-2 | Design MCP client transport trait | `McpTransport`, `StdioTransport` |
| 3-4 | Implement stdio transport | Subprocess management |
| 5-6 | Implement HTTP/SSE transport | Remote MCP connections |
| 7-8 | Tool discovery and invocation | Merge with native tools |
| 9-10 | Integration with agentic engine | Tool dispatch table extension |
| 11-12 | Permission model for MCP tools | Apply existing permission system |
| 13-14 | Test with existing MCP servers | Test against clawdius-mcp (self) |
| 15 | Documentation | MCP client configuration guide |

**Verification:** Clawdius can connect to external MCP servers (filesystem, GitHub) and invoke tools.

#### Week 7-10: LSP Enhancement (REQ-005)

| Day | Task | Deliverable |
|---|---|---|
| 1-3 | Replace symbol_index pattern matching with tree-sitter | Use CodeParser from core |
| 4-5 | Implement `textDocument/completion` | Symbol + keyword completions |
| 6-7 | Implement `textDocument/diagnostics` | Pull diagnostics |
| 8-9 | Implement `textDocument/codeAction` | Quick fixes + refactorings |
| 10 | Implement `textDocument/rename` | Project-wide rename |
| 11 | Implement `workspace/symbol` | Full-project search |
| 12-13 | Leptos or native code lens support | Code lens integration |
| 14-15 | Comprehensive test suite | Target: 50+ LSP tests |

**Verification:** VSCode extension connects and provides completions, diagnostics, go-to-definition, rename.

#### Week 8-12: Web Dashboard (REQ-006)

| Day | Task | Deliverable |
|---|---|---|
| 1-2 | Scaffold Leptos dashboard crate | `clawdius-dashboard` |
| 3-5 | Layout, navigation, auth gate | Shell with login |
| 6-8 | Overview page (health, sessions, usage) | Real-time metrics |
| 9-11 | Tenant management page | CRUD + API keys |
| 12-14 | Session monitoring page | Live session view |
| 15-17 | Usage analytics page | Charts and tables |
| 18-19 | Audit log viewer page | Query and filter |
| 20-21 | Settings page | Provider config, SSO, feature flags |
| 22 | WebSocket integration | Real-time updates |
| 23-24 | Build dashboard into Docker image | Serve from gateway |

**Verification:** Admin can manage tenants, view sessions, and monitor usage through web UI.

#### Week 10-11: gVisor and Firecracker Backends (REQ-007)

| Day | Task | Deliverable |
|---|---|---|
| 1-3 | Implement real gVisor backend | `runsc` detection, OCI config, seccomp |
| 4-6 | Implement real Firecracker backend | microVM launch, jailer, MMDS |
| 7-8 | Root filesystem preparation | Minimal ext4 for Firecracker |
| 9-10 | Integration tests | Requires `runsc` and `firecracker` binaries |
| 11 | Fallback logic | Graceful degradation to container backend |
| 12 | Documentation | Sandbox backend comparison guide |

**Verification:** Commands execute inside gVisor and Firecracker sandboxes with proper isolation.

### Phase C: Advantage Features (P2)

#### Week 10-14: Distributed LLM Routing (REQ-008)

| Day | Task | Deliverable |
|---|---|---|
| 1-3 | gRPC transport layer | Inter-node communication |
| 4-5 | Service discovery | DNS-based + static config |
| 6-8 | Load balancing strategies | Wire existing types to real transport |
| 9-10 | Health checking + circuit breaker | Per-provider health |
| 11-12 | Integration with LLM layer | Transparent routing |
| 13-14 | Token usage aggregation | Cross-node accounting |
| 15 | Rate limit coordination | Distributed rate limiting |

**Verification:** Two-node cluster routes LLM requests with failover.

#### Week 14-16: Monitoring and Alerting (REQ-009)

| Day | Task | Deliverable |
|---|---|---|
| 1-3 | Prometheus metrics endpoint | `/metrics` with standard + custom metrics |
| 4-6 | Grafana dashboard template | Pre-built JSON |
| 7-9 | Alerting rules | Prometheus alertmanager config |
| 10 | Sandbox escape detection | Security event alerting |
| 11-12 | Documentation | Monitoring setup guide |

**Verification:** Grafana dashboard shows live metrics from running Clawdius instance.

#### Week 16-18: Multimodal Input (REQ-010)

| Day | Task | Deliverable |
|---|---|---|
| 1-3 | Image file handling | Accept images as context |
| 4-6 | API integration (Claude Vision, GPT-4o) | Model-specific image handling |
| 7-8 | CLI interface for image input | `clawdius chat --image diagram.png` |
| 9-10 | Gateway adapter image support | Images from messaging platforms |
| 11-12 | Screenshot capture and analysis | Terminal/output screenshots |

**Verification:** User can attach images to chat and receive analysis.

### Phase D: Infrastructure (Continuous)

#### Hardened Docker Image (Evergreen Compliant)

Build a production-hardened container image following the Evergreen Image Registry
5-pillar standard:

**Dockerfile design:**

```dockerfile
# Build stage
FROM rust:1.92-bookworm AS builder
# ... (existing multi-stage build)

# Final stage: wolfi-base (NOT debian-slim)
FROM ghcr.io/wyattau/evergreenimageregistry/wolfi-base:latest AS runtime

# Non-root user (UID 65532 per Evergreen standard)
USER 65532:65532

# HEALTHCHECK mandatory
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/health-shim", "--port", "8080"]

# SBOM: auto-generated by CI
# Signing: cosign keyless via Sigstore
```

**Deployment target:** `wyatt@192.168.1.191`

---

## Part V: MiMo v2.5 Integration Testing

### Objective

Validate that Clawdius works correctly with the MiMo v2.5 model (via Z.AI API)
as an additional LLM provider option.

### Test Plan

1. **API Compatibility**: Send standard chat completion requests to MiMo v2.5 endpoint
2. **Tool Calling**: Verify tool_use format compatibility
3. **Streaming**: Test SSE streaming response handling
4. **Context Window**: Validate context window management with MiMo's limits
5. **Error Handling**: Test rate limit, timeout, and error response handling

### Integration Path

```toml
# In config.toml
[llm.providers.mimo]
type = "openai_compatible"
base_url = "https://api.zai.com/v1"
api_key_env = "ZAI_API_KEY"
default_model = "mimo-v2.5"
```

### Verification Criteria

- [ ] Chat completion returns valid responses
- [ ] Tool calls are correctly formatted and parsed
- [ ] Streaming tokens are properly concatenated
- [ ] Context compaction works within MiMo's token limits
- [ ] Rate limit errors are handled with retry logic

---

## Appendix A: Dependency Map

```
REQ-001 (SSO) ──────────> REQ-006 (Dashboard auth)
REQ-002 (Audit) ────────> REQ-009 (Monitoring)
REQ-003 (Signing) ──────> Evergreen Image
REQ-004 (MCP Client) ───> REQ-005 (LSP integration)
REQ-005 (LSP) ──────────> VSCode Extension
REQ-006 (Dashboard) ────> REQ-001 (SSO), REQ-002 (Audit viewer)
REQ-007 (gVisor/FC) ────> REQ-009 (Sandbox monitoring)
REQ-008 (Distributed) ──> REQ-009 (Health monitoring)
REQ-009 (Monitoring) ───> Grafana deployment
REQ-010 (Multimodal) ───> LLM provider updates
```

## Appendix B: Resource Estimates

| Phase | Calendar Weeks | Engineer-Weeks | New Lines of Rust |
|---|---|---|---|
| Phase A (P0) | 6 | 6 | ~8,000 |
| Phase B (P1) | 8 | 12 | ~15,000 |
| Phase C (P2) | 8 | 10 | ~8,000 |
| Phase D (Infra) | 18 (continuous) | 4 | ~1,000 |
| **Total** | **18 weeks** | **32 E-W** | **~32,000** |

## Appendix C: Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| SAML library maturity (Rust) | Medium | High | Evaluate `saml-rs`, fall back to FFI with xmlsec |
| Leptos learning curve | Medium | Medium | Consider React if team velocity drops |
| Firecracker binary availability | Low | Low | Container backend fallback exists |
| gVisor `runsc` compatibility | Low | Medium | Test on target kernel versions early |
| MiMo API compatibility | Medium | Low | OpenAI-compatible layer abstracts differences |
| Distributed consensus complexity | High | Medium | Start with leader election only, defer Raft |

---

*Last updated: 2026-06-12 | Based on empirical audit of v1.0.0 post-session 6*
