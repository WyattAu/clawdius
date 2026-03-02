# Testing Matrix

**Document ID:** CP-TEST-001  
**Version:** 1.0.0  
**Phase:** 4.5 (Cross-Platform Compatibility)  
**Status:** APPROVED  
**Created:** 2026-03-01  
**Trace To:** REQ-6.4, testing_requirements.md

---

## 1. Overview

### 1.1 Purpose

This document defines the comprehensive testing matrix for Clawdius across operating systems, architectures, and feature combinations.

### 1.2 Testing Principles

1. **Automated First:** Maximize automated test coverage
2. **Platform Parity:** Ensure equivalent behavior across platforms
3. **Feature Coverage:** Test all feature flag combinations
4. **Performance Regression:** Continuous benchmark monitoring

---

## 2. OS/Version Matrix

### 2.1 Tier 1: Linux x86_64 (Full Production)

| Distribution | Version | Kernel | CI Runner | Priority |
|--------------|---------|--------|-----------|----------|
| Ubuntu | 22.04 LTS | 5.15+ | ubuntu-latest | P0 |
| Ubuntu | 24.04 LTS | 6.8+ | ubuntu-latest | P0 |
| Debian | 12 (Bookworm) | 6.1+ | self-hosted | P1 |
| Fedora | 39+ | 6.5+ | self-hosted | P1 |
| Arch Linux | Rolling | Latest | self-hosted | P2 |

### 2.2 Tier 2: macOS (Full Production)

| Version | Architecture | CI Runner | Priority |
|---------|--------------|-----------|----------|
| macOS 13 (Ventura) | x86_64 | macos-13 | P0 |
| macOS 14 (Sonoma) | ARM64 | macos-14 | P0 |
| macOS 15 (Sequoia) | ARM64 | macos-15 | P1 |

### 2.3 Tier 3: Windows via WSL2 (Dev Only)

| WSL Version | Windows Version | CI Runner | Priority |
|-------------|-----------------|-----------|----------|
| WSL2 | Windows 11 | self-hosted | P2 |
| WSL2 | Windows 10 (21H2+) | self-hosted | P2 |

### 2.4 Tier 4: Linux ARM64 (Experimental)

| Distribution | Version | Hardware | CI Runner | Priority |
|--------------|---------|----------|-----------|----------|
| Ubuntu | 22.04 | ARM64 | self-hosted | P2 |
| Amazon Linux | 2023 | Graviton3 | self-hosted | P3 |

---

## 3. Architecture Matrix

### 3.1 Supported Architectures

| Architecture | Endianness | Word Size | SIMD | Support |
|--------------|------------|-----------|------|---------|
| x86_64 | Little | 64-bit | SSE2/AVX2 | Tier 1 |
| aarch64 | Little | 64-bit | NEON | Tier 2 |

### 3.2 SIMD Feature Matrix

| Platform | Minimum SIMD | Optimal SIMD | Fallback |
|----------|--------------|--------------|----------|
| x86_64 | SSE2 | AVX2 | Scalar |
| aarch64 | NEON | NEON | Scalar |

---

## 4. Feature Flag Matrix

### 4.1 Core Feature Combinations

| Features | Linux | macOS | WSL2 | Notes |
|----------|-------|-------|------|-------|
| default | ✓ | ✓ | ✓ | Standard build |
| io-uring | ✓ | ✗ | ✓ | Linux 5.1+ |
| hft | ✓ | ✗ | ✗ | Production only |
| wsl2 | ✗ | ✗ | ✓ | WSL2 interop |

### 4.2 Test Matrix

```yaml
matrix:
  os: [ubuntu-22.04, ubuntu-24.04, macos-13, macos-14]
  features:
    - ""
    - "io-uring"
    - "hft"
  exclude:
    - os: macos-13
      features: "io-uring"
    - os: macos-14
      features: "io-uring"
    - os: macos-13
      features: "hft"
    - os: macos-14
      features: "hft"
```

---

## 5. Test Categories

### 5.1 Unit Tests

| Category | Count | Automated | Coverage Target |
|----------|-------|-----------|-----------------|
| Core Logic | 200+ | ✓ | 90% |
| HAL | 50+ | ✓ | 85% |
| Sandbox | 40+ | ✓ | 85% |
| Keyring | 30+ | ✓ | 80% |
| FS Watcher | 25+ | ✓ | 80% |
| SIMD | 20+ | ✓ | 90% |

### 5.2 Integration Tests

| Category | Count | Automated | Platform-Specific |
|----------|-------|-----------|-------------------|
| Sandbox Integration | 15 | ✓ | Yes |
| Keyring Integration | 10 | ✓ | Yes |
| FS Watcher Integration | 8 | ✓ | Yes |
| End-to-End | 20 | ✓ | No |
| CLI | 15 | ✓ | No |

### 5.3 Platform-Specific Tests

```rust
#[cfg(target_os = "linux")]
mod linux_tests {
    use super::*;
    
    #[test]
    fn test_bubblewrap_spawn() {
        let sandbox = BubblewrapBackend::new();
        let handle = sandbox.spawn(default_config()).unwrap();
        assert!(handle.pid().is_some());
    }
    
    #[test]
    fn test_inotify_watcher() {
        let mut watcher = InotifyWatcher::new();
        watcher.watch(Path::new("/tmp")).unwrap();
    }
    
    #[test]
    fn test_io_uring_available() {
        assert!(detect_io_uring());
    }
}

#[cfg(target_os = "macos")]
mod macos_tests {
    use super::*;
    
    #[test]
    fn test_sandbox_exec_spawn() {
        let sandbox = SandboxExecBackend::new();
        let handle = sandbox.spawn(default_config()).unwrap();
        assert!(handle.pid().is_some());
    }
    
    #[test]
    fn test_keychain_access() {
        let keyring = KeychainBackend::new();
        keyring.set("test", "account", "password").unwrap();
        let secret = keyring.get("test", "account").unwrap();
        assert_eq!(secret.expose(), "password");
        keyring.delete("test", "account").unwrap();
    }
}
```

---

## 6. CI/CD Pipeline

### 6.1 Pipeline Stages

```
┌─────────────────────────────────────────────────────────────────┐
│                        CI/CD Pipeline                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐        │
│  │  Lint   │──▶│  Build  │──▶│  Test   │──▶│  Bench  │        │
│  └─────────┘   └─────────┘   └─────────┘   └─────────┘        │
│       │             │             │             │               │
│       ▼             ▼             ▼             ▼               │
│   clippy       debug/release  unit/integration criterion       │
│   rustfmt      all targets    coverage        regression       │
│   docs         features       platform                         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 GitHub Actions Workflow

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo fmt --check
      - run: cargo clippy --all-targets --all-features -- -D warnings

  test-linux:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-22.04, ubuntu-24.04]
        features: ["", "io-uring", "hft"]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Install dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y bubblewrap libsecret-1-dev
      - name: Run tests
        run: cargo test --features "${{ matrix.features }}"
      - name: Run coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --features "${{ matrix.features }}" --out Xml
      - uses: codecov/codecov-action@v4
        with:
          files: cobertura.xml

  test-macos:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [macos-13, macos-14]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run tests
        run: cargo test

  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run benchmarks
        run: cargo bench --no-run
      - name: Check benchmark regression
        run: |
          cargo install critcmp
          critcmp main HEAD

  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run cargo-deny
        uses: EmbarkStudios/cargo-deny-action@v1
      - name: Run cargo-vet
        run: |
          cargo install cargo-vet
          cargo vet

  build-release:
    needs: [lint, test-linux, test-macos]
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
          - target: x86_64-apple-darwin
            os: macos-13
          - target: aarch64-apple-darwin
            os: macos-14
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Build release
        run: cargo build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: clawdius-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/clawdius
```

---

## 7. Manual Testing Requirements

### 7.1 Platform-Specific Manual Tests

| Test | Platform | Frequency | Owner |
|------|----------|-----------|-------|
| bubblewrap sandbox escape | Linux | Per release | Security |
| sandbox-exec profile validation | macOS | Per release | Security |
| WSL2 interop | Windows | Per release | Platform |
| Keychain biometric prompt | macOS | Per release | Platform |
| Secret Service daemon restart | Linux | Per release | Platform |

### 7.2 Performance Manual Tests

| Test | Platform | Metric | Threshold |
|------|----------|--------|-----------|
| Boot time | All | ms | < 20ms |
| Memory idle | All | MB | < 30MB |
| HFT latency | Linux | µs | < 1000µs |
| Graph-RAG query | All | ms | < 100ms |

### 7.3 Manual Test Checklist

```markdown
## Pre-Release Manual Test Checklist

### Linux
- [ ] bubblewrap isolation verified
- [ ] io_uring performance validated
- [ ] libsecret integration working
- [ ] inotify events correct

### macOS
- [ ] sandbox-exec isolation verified
- [ ] Keychain prompts working
- [ ] FSEvents correct
- [ ] Universal binary builds

### WSL2
- [ ] WSL detection correct
- [ ] Windows interop working
- [ ] File system performance acceptable

### All Platforms
- [ ] Boot time < 20ms
- [ ] Memory usage < 30MB idle
- [ ] No crashes in 24h stress test
- [ ] Documentation accurate
```

---

## 8. Test Environment Setup

### 8.1 Linux Environment

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y \
    bubblewrap \
    libsecret-1-dev \
    pkg-config \
    build-essential \
    cmake

# Fedora
sudo dnf install -y \
    bubblewrap \
    libsecret-devel \
    pkgconfig \
    gcc \
    cmake

# Arch
sudo pacman -S --needed \
    bubblewrap \
    libsecret \
    pkg-config \
    base-devel \
    cmake
```

### 8.2 macOS Environment

```bash
# Install Xcode Command Line Tools
xcode-select --install

# Install Homebrew packages (optional)
brew install cmake
```

### 8.3 WSL2 Environment

```bash
# Same as Linux, plus WSL2 kernel update
wsl --update

# Enable systemd (for keyring)
# /etc/wsl.conf:
[boot]
systemd = true
```

---

## 9. Test Data Management

### 9.1 Test Fixtures

```
tests/
├── fixtures/
│   ├── sandbox/
│   │   ├── hello_world.rs
│   │   ├── network_test.rs
│   │   └── fs_test.rs
│   ├── keyring/
│   │   ├── test_credentials.json
│   │   └── mock_keychain.json
│   └── market_data/
│       ├── sample_sbe.bin
│       └── sample_fix.txt
└── integration/
    ├── sandbox_test.rs
    ├── keyring_test.rs
    └── hft_test.rs
```

### 9.2 Mock Backends

```rust
pub struct MockSandboxBackend {
    spawns: Arc<Mutex<Vec<SandboxConfig>>>,
    responses: Vec<Result<SandboxHandle, SandboxError>>,
}

impl MockSandboxBackend {
    pub fn new() -> Self {
        Self {
            spawns: Arc::new(Mutex::new(Vec::new())),
            responses: vec![Ok(SandboxHandle::mock())],
        }
    }
    
    pub fn with_response(mut self, response: Result<SandboxHandle, SandboxError>) -> Self {
        self.responses.push(response);
        self
    }
}

impl SandboxBackend for MockSandboxBackend {
    fn spawn(&self, config: SandboxConfig) -> Result<SandboxHandle, SandboxError> {
        self.spawns.lock().unwrap().push(config);
        self.responses.first().cloned().unwrap_or_else(|| Ok(SandboxHandle::mock()))
    }
}
```

---

## 10. Coverage Targets

### 10.1 Code Coverage

| Component | Target | Current |
|-----------|--------|---------|
| Core | 90% | - |
| HAL | 85% | - |
| Sandbox | 85% | - |
| Keyring | 80% | - |
| FS Watcher | 80% | - |
| SIMD | 90% | - |
| **Overall** | **85%** | - |

### 10.2 Feature Coverage

| Feature | Linux | macOS | WSL2 |
|---------|-------|-------|------|
| Sandbox spawn | ✓ | ✓ | ✓ |
| Sandbox kill | ✓ | ✓ | ✓ |
| Keyring get/set | ✓ | ✓ | ✓ |
| Keyring delete | ✓ | ✓ | ✓ |
| FS watch | ✓ | ✓ | ✓ |
| io_uring | ✓ | N/A | ✓ |

---

## 11. Regression Testing

### 11.1 Benchmark Regression

```yaml
# .github/workflows/benchmark.yml
name: Benchmark Regression

on:
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      
      - uses: dtolnay/rust-toolchain@stable
      
      - name: Install critcmp
        run: cargo install critcmp
      
      - name: Run benchmarks (PR)
        run: cargo bench -- --save-baseline pr
      
      - name: Checkout main
        run: git checkout main
      
      - name: Run benchmarks (main)
        run: cargo bench -- --save-baseline main
      
      - name: Compare
        run: critcmp main pr --threshold 5
```

### 11.2 Performance Thresholds

| Benchmark | Threshold | Action |
|-----------|-----------|--------|
| Boot time | +10% | Warning |
| Boot time | +20% | Block |
| HFT latency | +5% | Warning |
| HFT latency | +10% | Block |
| Memory | +20% | Warning |

---

## 12. Compliance

| Standard | Clause | Compliance |
|----------|--------|------------|
| IEEE 829 | Test Documentation | Full |
| ISO/IEC 25010 | Quality Assurance | Full |
| REQ-6.4 | PAL Testing | Full |

---

**Document Status:** APPROVED  
**Next Review:** After CI/CD implementation
