# Supply Chain Security Review: Clawdius High-Assurance Engineering Engine

**Document ID:** SCS-CLAWDIUS-001  
**Version:** 1.0.0  
**Phase:** 3 (Security Engineering - Red Phase)  
**Status:** APPROVED  
**Created:** 2026-03-01  
**Classification:** Supply Chain Security

---

## 1. Executive Summary

This document establishes the supply chain security framework for Clawdius, building on the Phase 1.5 SBOM and defining dependency management, cargo-vet requirements, and update policies.

### 1.1 Supply Chain Security Metrics

| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Dependencies with cargo-vet audit | 0/20 | 20/20 | ⏳ Pending |
| Dependencies with unsafe code audited | 0/8 | 8/8 | ⏳ Pending |
| CVEs in dependencies | 0 | 0 | ✅ |
| License compliance | 100% | 100% | ✅ |
| Dependency pinning | 100% | 100% | ✅ |

### 1.2 Risk Summary

| Risk Level | Count | Description |
|------------|-------|-------------|
| Critical | 0 | No critical supply chain risks |
| High | 3 | Direct dependencies with unsafe code |
| Medium | 4 | Transitive unmaintained dependencies |
| Low | 2 | Optional dependencies with known issues |

---

## 2. Dependency Inventory

### 2.1 Direct Dependencies (Tier 1)

*Reference: `.clawdius/specs/01_5_supply_chain/sbom.spdx`*

| Package | Version | License | Unsafe | Audit Status |
|---------|---------|---------|--------|--------------|
| monoio | 0.2.4 | Apache-2.0 | Yes (io_uring) | ⏳ Required |
| wasmtime | 42.0.1 | Apache-2.0 | Yes (JIT) | ⏳ Required |
| rusqlite | 0.38.0 | MIT | Yes (FFI) | ⏳ Required |
| lancedb | 0.26.2 | Apache-2.0 | Unknown | ⏳ Required |
| genai | 0.5.3 | Apache-2.0 | No | ✅ Safe |
| tree-sitter | 0.26.0 | MIT | Yes (C FFI) | ⏳ Required |
| ratatui | 0.30.0 | MIT | No | ✅ Safe |
| serde | 1.0.228 | Apache-2.0/MIT | No | ✅ Safe |
| serde_json | 1.0.149 | Apache-2.0/MIT | No | ✅ Safe |
| toml | 1.0.3 | Apache-2.0/MIT | No | ✅ Safe |
| rkyv | 0.8.10 | MIT | Yes (zero-copy) | ⏳ Required |
| async-openai | 0.33.0 | Apache-2.0 | No | ✅ Safe |
| crossterm | 0.29.0 | Apache-2.0 | No | ✅ Safe |
| keyring | 3.6.2 | Apache-2.0/MIT | Yes (FFI) | ⏳ Required |
| uuid | 1.21.0 | Apache-2.0/MIT | No | ✅ Safe |
| thiserror | 2.0.18 | Apache-2.0/MIT | No | ✅ Safe |
| tracing | 0.1.44 | MIT | No | ✅ Safe |
| tracing-subscriber | 0.3.20 | MIT | No | ✅ Safe |
| syntect | 5.3.0 | Apache-2.0 | Yes (C FFI) | ⏳ Required |
| mimalloc | 0.1.46 | MIT | Yes (C FFI) | ⏳ Required |

### 2.2 Unsafe Code Audit Requirements

| Package | Unsafe Reason | Audit Priority | Auditor |
|---------|---------------|----------------|---------|
| monoio | io_uring syscalls | Critical | Internal |
| wasmtime | JIT compilation | Critical | External |
| rusqlite | SQLite FFI | High | External |
| tree-sitter | Parser C FFI | High | External |
| rkyv | Zero-copy casting | Medium | Internal |
| keyring | Platform keyring FFI | Medium | Internal |
| syntect | Oniguruma FFI | Medium | External |
| mimalloc | Allocator FFI | Medium | External |

---

## 3. cargo-vet Configuration

### 3.1 Initialization

```bash
# Initialize cargo-vet
cargo vet init

# Configure for Clawdius
cat > supply-chain/audits.toml << 'EOF'
# Clawdius Supply Chain Audits

[audits]
# Audits will be added here as dependencies are reviewed
EOF
```

### 3.2 Audit Policy

```toml
# supply-chain/config.toml

[policy]
# Require audits for all dependencies with unsafe code
audit-as-crates-io = true

# Criteria for audits
[criteria.safe-to-run]
description = "Safe to run in development and testing environments"
implies = "safe-to-deploy"

[criteria.safe-to-deploy]
description = "Safe to deploy in production environments"
implies = "safe-to-run"

# Import audits from trusted sources
[imports.google]
url = "https://raw.githubusercontent.com/google/rust-crate-audits/main/audits.toml"

[imports.mozilla]
url = "https://raw.githubusercontent.com/mozilla/supply-chain/main/audits.toml"

# Exemptions for dependencies that need audit (temporary)
[[exemptions.monoio]]
version = "0.2.4"
criteria = "safe-to-run"
note = "Pending internal audit - io_uring code review required"

[[exemptions.wasmtime]]
version = "42.0.1"
criteria = "safe-to-run"
note = "Pending external audit - Bytecode Alliance maintained"
```

### 3.3 Audit Workflow

```
┌─────────────────────────────────────────────────────────────────┐
│                    cargo-vet Workflow                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐                                                │
│  │ New Version │                                                │
│  │ Detected    │                                                │
│  └──────┬──────┘                                                │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────┐     ┌─────────────┐                           │
│  │ cargo vet   │────►│ Check       │                           │
│  │ check       │     │ Imports     │                           │
│  └─────────────┘     └──────┬──────┘                           │
│                             │                                    │
│                             ▼                                    │
│                      ┌─────────────┐                            │
│                      │ Already     │──Yes──► PASS               │
│                      │ Audited?    │                            │
│                      └──────┬──────┘                            │
│                             │ No                                 │
│                             ▼                                    │
│                      ┌─────────────┐                            │
│                      │ Has Unsafe? │                            │
│                      └──────┬──────┘                            │
│                             │                                    │
│              ┌──────────────┼──────────────┐                    │
│              │ Yes          │              │ No                 │
│              ▼              │              ▼                    │
│      ┌─────────────┐       │       ┌─────────────┐             │
│      │ Require     │       │       │ Allow       │             │
│      │ Full Audit  │       │       │ (low risk)  │             │
│      └──────┬──────┘       │       └──────┬──────┘             │
│             │              │              │                      │
│             ▼              │              ▼                      │
│      ┌─────────────┐       │       ┌─────────────┐             │
│      │ Review Code │       │       │ Log &       │             │
│      │ + Document  │       │       │ Proceed     │             │
│      └──────┬──────┘       │       └─────────────┘             │
│             │              │                                    │
│             ▼              │                                    │
│      ┌─────────────┐       │                                    │
│      │ Add Audit   │       │                                    │
│      │ Entry       │       │                                    │
│      └─────────────┘       │                                    │
│                           │                                    │
└───────────────────────────┼────────────────────────────────────┘
                            │
                            ▼
                     ┌─────────────┐
                     │ CI Gate     │
                     │ (cargo vet  │
                     │  check)     │
                     └─────────────┘
```

### 3.4 Audit Template

```toml
# supply-chain/audits.toml

[[audits.monoio]]
version = "0.2.4"
criteria = "safe-to-deploy"
who = "Clawdius Security Team <security@clawdius.dev>"
notes = """
Reviewed [date]. Findings:
1. io_uring implementation uses safe wrappers around syscalls
2. No memory safety issues found in unsafe blocks
3. Thread-safety verified through atomic operations
4. No network calls or data exfiltration
5. License: Apache-2.0 (compatible)

Recommendation: APPROVED for production deployment.
"""
```

---

## 4. Dependency Update Policy

### 4.1 Update Categories

| Category | Frequency | Process | Automation |
|----------|-----------|---------|------------|
| Security patches | Immediate | Emergency review + merge | Dependabot |
| Minor version updates | Weekly | Automated PR + CI check | Renovate |
| Major version updates | Monthly | Manual review required | Renovate (manual) |
| Pre-release versions | Never | Blocked | cargo-deny |

### 4.2 Update Workflow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Dependency Update Workflow                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────┐                                            │
│  │ Update Detected │                                            │
│  │ (Renovate/Dep.) │                                            │
│  └────────┬────────┘                                            │
│           │                                                      │
│           ▼                                                      │
│  ┌─────────────────┐     ┌─────────────────────────────────┐   │
│  │ Is Security     │──Yes──►│ Emergency Process              │   │
│  │ Advisory?       │        │ 1. Review advisory             │   │
│  └────────┬────────┘        │ 2. Assess impact               │   │
│           │ No              │ 3. Update + test               │   │
│           ▼                 │ 4. Fast-track merge            │   │
│  ┌─────────────────┐        └─────────────────────────────────┘   │
│  │ Is Major        │                                        │
│  │ Version?        │                                        │
│  └────────┬────────┘                                        │
│           │                                                    │
│           ├──Yes──► Manual Review Required                     │
│           │            - Breaking changes assessment           │
│           │            - Migration plan                        │
│           │            - Team approval                         │
│           │                                                    │
│           │ No                                                 │
│           ▼                                                    │
│  ┌─────────────────┐                                            │
│  │ Automated PR    │                                            │
│  │ Created         │                                            │
│  └────────┬────────┘                                            │
│           │                                                      │
│           ▼                                                      │
│  ┌─────────────────┐                                            │
│  │ CI Pipeline     │                                            │
│  │ ├─ cargo check  │                                            │
│  │ ├─ cargo test   │                                            │
│  │ ├─ cargo clippy │                                            │
│  │ ├─ cargo deny   │                                            │
│  │ └─ cargo vet    │                                            │
│  └────────┬────────┘                                            │
│           │                                                      │
│           ├──Fail──► Block merge, notify team                   │
│           │                                                    │
│           │ Pass                                               │
│           ▼                                                    │
│  ┌─────────────────┐                                            │
│  │ Auto-merge      │                                            │
│  │ (if configured) │                                            │
│  └─────────────────┘                                            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4.3 Renovate Configuration

```json
// renovate.json
{
  "$schema": "https://docs.renovatebot.com/renovate-schema.json",
  "extends": [
    "config:base"
  ],
  "timezone": "UTC",
  "schedule": ["every weekend"],
  "rangeStrategy": "pin",
  "lockFileMaintenance": {
    "enabled": true,
    "schedule": ["before 5am on Monday"]
  },
  "packageRules": [
    {
      "matchPackagePatterns": ["*"],
      "semanticCommitType": "deps",
      "semanticCommitScope": "cargo"
    },
    {
      "matchUpdateTypes": ["major"],
      "reviewers": ["team:security"],
      "labels": ["major-update", "needs-review"],
      "automerge": false
    },
    {
      "matchUpdateTypes": ["minor", "patch"],
      "automerge": true,
      "automergeType": "pr"
    },
    {
      "matchPackageNames": ["wasmtime", "monoio", "rusqlite"],
      "reviewers": ["team:security"],
      "labels": ["security-sensitive"],
      "automerge": false
    }
  ],
  "vulnerabilityAlerts": {
    "enabled": true,
    "labels": ["security"],
    "reviewers": ["team:security"],
    "assignees": ["team:security"]
  }
}
```

---

## 5. Build System Security

### 5.1 Reproducible Builds

```toml
# Cargo.toml

[profile.release]
# Reproducible build settings
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true

# Deterministic settings
[profile.release.package."*"]
opt-level = 3

# Build configuration
[unstable]
# Enable reproducible archive
reproducible-artifact = ["bin"]
```

### 5.2 Nix Flake Configuration

```nix
# flake.nix (relevant security sections)
{
  description = "Clawdius - High-Assurance Engineering Engine";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        
        # Pinned Rust version for reproducibility
        rustToolchain = pkgs.rust-bin.stable."1.85.0".default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo-nextest
            cargo-deny
            cargo-vet
            cargo-mutants
            bubblewrap
            podman
            protobuf  # for lancedb
          ];
        };
      }
    );
}
```

### 5.3 Build Verification

```bash
#!/bin/bash
# scripts/verify-build.sh

set -euo pipefail

echo "Verifying reproducible build..."

# Build twice with different source paths
cp -r . /tmp/clawdius-build1
cp -r . /tmp/clawdius-build2

cd /tmp/clawdius-build1
cargo build --release --locked
HASH1=$(sha256sum target/release/clawdius | cut -d' ' -f1)

cd /tmp/clawdius-build2
cargo build --release --locked
HASH2=$(sha256sum target/release/clawdius | cut -d' ' -f1)

if [ "$HASH1" = "$HASH2" ]; then
    echo "✅ Build is reproducible: $HASH1"
    exit 0
else
    echo "❌ Build is NOT reproducible"
    echo "Build 1: $HASH1"
    echo "Build 2: $HASH2"
    exit 1
fi
```

---

## 6. Container Security

### 6.1 Container Image Policy

| Policy | Requirement |
|--------|-------------|
| Base image | distroless/static-debian12 |
| Root user | Forbidden (run as non-root) |
| Secrets | No secrets in images |
| SBOM | Required (sbom.spdx) |
| Signature | Required (cosign) |

### 6.2 Containerfile

```dockerfile
# Containerfile
FROM rust:1.85-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked

FROM gcr.io/distroless/static-debian12:latest

COPY --from=builder /app/target/release/clawdius /clawdius

# Run as non-root (distroless default)
USER nonroot:nonroot

ENTRYPOINT ["/clawdius"]
```

### 6.3 Container Signing

```bash
# Sign container image with cosign
cosign sign --key cosign.key quay.io/clawdius/clawdius:$VERSION

# Verify signature
cosign verify --key cosign.pub quay.io/clawdius/clawdius:$VERSION
```

---

## 7. Vulnerability Management

### 7.1 CVE Response Process

```
┌─────────────────────────────────────────────────────────────────┐
│                    CVE Response Process                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────┐                                            │
│  │ CVE Reported    │                                            │
│  │ (cargo-audit)   │                                            │
│  └────────┬────────┘                                            │
│           │                                                      │
│           ▼                                                      │
│  ┌─────────────────┐                                            │
│  │ Assess Severity │                                            │
│  │ (CVSS Score)    │                                            │
│  └────────┬────────┘                                            │
│           │                                                      │
│           ├──Critical──► 24h remediation SLA                    │
│           │                                                    │
│           ├──High──────► 72h remediation SLA                    │
│           │                                                    │
│           ├──Medium────► 1 week remediation SLA                 │
│           │                                                    │
│           └──Low────────► Next release cycle                    │
│                                                                  │
│  ┌─────────────────┐                                            │
│  │ Remediation     │                                            │
│  │ ├─ Update dep   │                                            │
│  │ ├─ Patch code   │                                            │
│  │ └─ Mitigate     │                                            │
│  └────────┬────────┘                                            │
│           │                                                      │
│           ▼                                                      │
│  ┌─────────────────┐                                            │
│  │ Verification    │                                            │
│  │ ├─ cargo audit  │                                            │
│  │ ├─ cargo test   │                                            │
│  │ └─ cargo vet    │                                            │
│  └────────┬────────┘                                            │
│           │                                                      │
│           ▼                                                      │
│  ┌─────────────────┐                                            │
│  │ Document in     │                                            │
│  │ CHANGELOG.md    │                                            │
│  └─────────────────┘                                            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 7.2 cargo-deny Configuration

```toml
# deny.toml

[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
vulnerability = "deny"
unmaintained = "warn"
yanked = "deny"
notice = "warn"
ignore = []

[licenses]
unlicensed = "deny"
allow = [
    "Apache-2.0",
    "MIT",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
    "ISC",
    "Unicode-DFS-2016",
]
deny = [
    "GPL-1.0",
    "GPL-2.0",
    "GPL-3.0",
    "AGPL-1.0",
    "AGPL-2.0",
    "AGPL-3.0",
]
copyleft = "deny"
allow-osi-fsf-free = "neither"
default = "deny"
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
wildcards = "deny"
highlight = "all"
workspace-default-features = "allow"
external-default-features = "allow"
allow = []
deny = [
    { name = "openssl", reason = "Use rustls instead" },
    { name = "native-tls", reason = "Use rustls instead" },
]

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

---

## 8. Known Issues and Mitigations

### 8.1 Unmaintained Dependencies

*From Phase 1.5 dependency analysis:*

| Package | Issue | Mitigation | Action |
|---------|-------|------------|--------|
| bincode | Unmaintained | Low risk, used by tree-sitter | Monitor, plan replacement |
| fxhash | Unmaintained | Low risk, not in hot path | Monitor, plan replacement |
| paste | Unmaintained | Proc macro, no runtime risk | Monitor |
| yaml-rust | Unmaintained | Not directly used | Transitive, monitor |

### 8.2 Dependency Conflicts

| Conflict | Resolution | Status |
|----------|------------|--------|
| Multiple syn versions | Allow (minor) | Accepted |
| Multiple parking_lot versions | Allow (minor) | Accepted |

---

## 9. Third-Party Security Audits

### 9.1 Audit Schedule

| Dependency | Last Audit | Next Due | Auditor |
|------------|------------|----------|---------|
| wasmtime | 2025-06 | 2026-06 | Bytecode Alliance |
| rusqlite | 2024-03 | 2026-03 | External (TBD) |
| tree-sitter | 2024-01 | 2026-01 | External (TBD) |
| monoio | None | 2026-04 | Internal |

### 9.2 Audit Requirements

For dependencies with unsafe code, audits must verify:

1. **Memory Safety:** All unsafe blocks are sound
2. **Thread Safety:** Proper synchronization in concurrent code
3. **No Backdoors:** No hidden network calls or data exfiltration
4. **Input Validation:** Proper handling of untrusted input
5. **Error Handling:** No panics on invalid input
6. **Resource Management:** Proper cleanup (no leaks)

---

## 10. Supply Chain Security Checklist

### 10.1 Pre-Commit Checklist

- [ ] `cargo deny check` passes
- [ ] `cargo audit` shows no CVEs
- [ ] `cargo vet check` passes (or exemptions documented)
- [ ] Lockfile committed (`Cargo.lock`)
- [ ] No `*` version requirements

### 10.2 Pre-Release Checklist

- [ ] All new dependencies have cargo-vet audit
- [ ] All dependencies with unsafe code audited
- [ ] SBOM updated (`sbom.spdx`)
- [ ] Container image signed
- [ ] License compliance verified
- [ ] Unmaintained dependencies reviewed

### 10.3 Annual Checklist

- [ ] Full dependency audit
- [ ] Third-party security audit
- [ ] Update policy review
- [ ] Vendor risk assessment
- [ ] SBOM accuracy verification

---

## 11. Incident Response

### 11.1 Supply Chain Incident Types

| Type | Example | Response |
|------|---------|----------|
| Malicious package | typosquatting | Immediate removal, scan codebase |
| Compromised maintainer | account takeover | Freeze dependencies, audit changes |
| CVE disclosure | critical vuln | Emergency update process |
| Build system compromise | CI/CD attack | Rebuild from clean state |

### 11.2 Response Contacts

| Role | Contact | Availability |
|------|---------|--------------|
| Security Lead | security@clawdius.dev | 24/7 |
| On-call Engineer | PagerDuty | 24/7 |
| Legal | legal@clawdius.dev | Business hours |

---

**Document Status:** APPROVED  
**Next Review:** 2026-04-01  
**Sign-off:** Security Engineering Team
