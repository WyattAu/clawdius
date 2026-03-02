# Conditional Compilation Strategy

**Document ID:** CP-CFG-001  
**Version:** 1.0.0  
**Phase:** 4.5 (Cross-Platform Compatibility)  
**Status:** APPROVED  
**Created:** 2026-03-01  
**Trace To:** REQ-6.4, os_compatibility.md

---

## 1. Overview

### 1.1 Purpose

This document defines the conditional compilation strategy, cfg flags, and platform detection mechanisms for Clawdius.

### 1.2 Strategy

- Use `cfg` attributes for compile-time platform detection
- Use feature flags for optional functionality
- Use runtime detection where compile-time is insufficient
- Document all platform-specific code paths

---

## 2. Platform cfg Flags

### 2.1 Operating System Flags

```rust
// Target operating system detection
#[cfg(target_os = "linux")]
#[cfg(target_os = "macos")]
#[cfg(target_os = "windows")]

// Combined conditions
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[cfg(all(target_os = "linux", not(feature = "wsl2")))]
```

### 2.2 Architecture Flags

```rust
// Target architecture detection
#[cfg(target_arch = "x86_64")]
#[cfg(target_arch = "aarch64")]
#[cfg(target_arch = "x86")]
#[cfg(target_arch = "arm")]

// Architecture families
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
```

### 2.3 Endianness Flags

```rust
#[cfg(target_endian = "little")]
#[cfg(target_endian = "big")]

// Clawdius requires little-endian
#[cfg(not(target_endian = "little"))]
compile_error!("Clawdius requires little-endian architecture");
```

### 2.4 Word Size Flags

```rust
#[cfg(target_pointer_width = "64")]
#[cfg(target_pointer_width = "32")]

// Clawdius requires 64-bit
#[cfg(not(target_pointer_width = "64"))]
compile_error!("Clawdius requires 64-bit architecture");
```

### 2.5 Platform Combinations

```rust
// Linux x86_64 (Tier 1)
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]

// macOS ARM64 (Tier 2)
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]

// macOS x86_64 (Tier 2)
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]

// Linux ARM64 (Experimental)
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]

// WSL2 (detected at runtime)
#[cfg(all(target_os = "linux", feature = "wsl2"))]
```

---

## 3. Feature Flags

### 3.1 Core Features

```toml
# Cargo.toml
[features]
default = ["keyring", "sandbox", "fs-watch"]

# Async runtime
io-uring = ["monoio"]
tokio-runtime = ["tokio"]

# Platform features
wsl2 = []

# Sandbox backends
sandbox-bubblewrap = []
sandbox-sandbox-exec = []

# Keyring backends
keyring-libsecret = ["keyring"]
keyring-keychain = ["keyring"]

# FS watcher backends
fs-watch-inotify = ["notify"]
fs-watch-fsevents = ["notify"]

# HFT mode
hft = ["io-uring", "hugepage"]

# Development features
benchmark = ["criterion"]
```

### 3.2 Conditional Feature Dependencies

```toml
# Cargo.toml
[target.'cfg(target_os = "linux")'.dependencies]
monoio = { version = "0.2", optional = true }
bubblewrap = { path = "../bubblewrap", optional = true }

[target.'cfg(target_os = "macos")'.dependencies]
security-framework = "2.9"

[target.'cfg(all(target_os = "linux", feature = "io-uring"))'.dependencies]
io-uring = "0.6"

[target.'cfg(all(target_os = "linux", feature = "hft"))'.dependencies]
hugepage = "0.1"
```

---

## 4. Platform-Specific Code Organization

### 4.1 Module Structure

```
src/
├── platform/
│   ├── mod.rs           # Platform dispatch
│   ├── linux.rs         # Linux-specific code
│   ├── macos.rs         # macOS-specific code
│   ├── wsl2.rs          # WSL2-specific code
│   └── unix.rs          # Shared Unix code
├── hal/
│   ├── mod.rs           # HAL trait definitions
│   ├── sandbox/
│   │   ├── mod.rs
│   │   ├── bubblewrap.rs
│   │   └── sandbox_exec.rs
│   ├── keyring/
│   │   ├── mod.rs
│   │   ├── libsecret.rs
│   │   └── keychain.rs
│   └── fs_watcher/
│       ├── mod.rs
│       └── inotify.rs
└── arch/
    ├── mod.rs
    ├── x86_64.rs
    └── aarch64.rs
```

### 4.2 Platform Dispatch Module

```rust
// src/platform/mod.rs

mod unix;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(all(target_os = "linux", feature = "wsl2"))]
mod wsl2;

#[cfg(unix)]
pub use unix::*;

/// Platform detection at runtime
pub fn detect_platform() -> Platform {
    #[cfg(target_os = "linux")]
    {
        if is_wsl2() {
            Platform::WindowsWSL2
        } else {
            Platform::Linux
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        Platform::MacOS
    }
}

#[cfg(target_os = "linux")]
fn is_wsl2() -> bool {
    std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists()
}
```

### 4.3 Linux Platform Module

```rust
// src/platform/linux.rs

use crate::hal::{Hal, SandboxBackend, KeyringBackend, FsWatcherBackend};

pub struct LinuxHal {
    sandbox: Box<dyn SandboxBackend>,
    keyring: Box<dyn KeyringBackend>,
    fs_watcher: Box<dyn FsWatcherBackend>,
}

impl LinuxHal {
    pub fn new() -> Self {
        let features = detect_features();
        
        let sandbox: Box<dyn SandboxBackend> = if features.user_namespaces {
            Box::new(crate::hal::sandbox::BubblewrapBackend::new())
        } else {
            log::warn!("User namespaces unavailable");
            Box::new(crate::hal::sandbox::RestrictedBubblewrapBackend::new())
        };
        
        Self {
            sandbox,
            keyring: Box::new(crate::hal::keyring::LibsecretBackend::new()),
            fs_watcher: Box::new(crate::hal::fs_watcher::InotifyWatcher::new()),
        }
    }
}

impl Hal for LinuxHal {
    fn platform(&self) -> crate::hal::Platform {
        crate::hal::Platform::Linux
    }
    
    fn sandbox(&self) -> &dyn SandboxBackend {
        self.sandbox.as_ref()
    }
    
    fn keyring(&self) -> &dyn KeyringBackend {
        self.keyring.as_ref()
    }
    
    fn fs_watcher(&self) -> &dyn FsWatcherBackend {
        self.fs_watcher.as_ref()
    }
}

pub struct LinuxFeatures {
    pub io_uring: bool,
    pub user_namespaces: bool,
    pub seccomp: bool,
}

pub fn detect_features() -> LinuxFeatures {
    LinuxFeatures {
        io_uring: detect_io_uring(),
        user_namespaces: detect_user_namespaces(),
        seccomp: detect_seccomp(),
    }
}

fn detect_io_uring() -> bool {
    std::fs::read_to_string("/proc/version")
        .ok()
        .and_then(|v| {
            let parts: Vec<&str> = v.split_whitespace().nth(2)?.split('.').collect();
            let major: u32 = parts.first()?.parse().ok()?;
            let minor: u32 = parts.get(1)?.parse().ok()?;
            Some(major > 5 || (major == 5 && minor >= 1))
        })
        .unwrap_or(false)
}

fn detect_user_namespaces() -> bool {
    std::path::Path::new("/proc/self/ns/user").exists()
}

fn detect_seccomp() -> bool {
    std::path::Path::new("/proc/self/status").exists()
}
```

### 4.4 macOS Platform Module

```rust
// src/platform/macos.rs

use crate::hal::{Hal, SandboxBackend, KeyringBackend, FsWatcherBackend};

pub struct MacOsHal {
    sandbox: Box<dyn SandboxBackend>,
    keyring: Box<dyn KeyringBackend>,
    fs_watcher: Box<dyn FsWatcherBackend>,
}

impl MacOsHal {
    pub fn new() -> Self {
        Self {
            sandbox: Box::new(crate::hal::sandbox::SandboxExecBackend::new()),
            keyring: Box::new(crate::hal::keyring::KeychainBackend::new()),
            fs_watcher: Box::new(crate::hal::fs_watcher::FseventsWatcher::new()),
        }
    }
}

impl Hal for MacOsHal {
    fn platform(&self) -> crate::hal::Platform {
        crate::hal::Platform::MacOS
    }
    
    fn sandbox(&self) -> &dyn SandboxBackend {
        self.sandbox.as_ref()
    }
    
    fn keyring(&self) -> &dyn KeyringBackend {
        self.keyring.as_ref()
    }
    
    fn fs_watcher(&self) -> &dyn FsWatcherBackend {
        self.fs_watcher.as_ref()
    }
}
```

---

## 5. SIMD Conditional Compilation

### 5.1 SIMD Module Organization

```rust
// src/arch/mod.rs

mod scalar;

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "aarch64")]
mod aarch64;

pub fn parse_market_data(data: &[u8]) -> ParseResult {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { x86_64::parse_market_data_avx2(data) };
        } else if is_x86_feature_detected!("sse4.2") {
            return unsafe { x86_64::parse_market_data_sse42(data) };
        }
    }
    
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::parse_market_data_neon(data) };
        }
    }
    
    scalar::parse_market_data(data)
}
```

### 5.2 x86_64 SIMD Module

```rust
// src/arch/x86_64.rs

use std::arch::x86_64::*;

#[target_feature(enable = "avx2")]
pub unsafe fn parse_market_data_avx2(data: &[u8]) -> ParseResult {
    let ptr = data.as_ptr();
    let len = data.len();
    
    let delimiter = _mm256_set1_epi8(b'|' as i8);
    let mut result = ParseResult::new();
    let mut i = 0;
    
    while i + 32 <= len {
        let chunk = _mm256_loadu_si256(ptr.add(i) as *const __m256i);
        let cmp = _mm256_cmpeq_epi8(chunk, delimiter);
        let mask = _mm256_movemask_epi8(cmp);
        
        if mask != 0 {
            result.add_positions_from_mask(mask, i);
        }
        
        i += 32;
    }
    
    result.process_tail(&data[i..]);
    result
}

#[target_feature(enable = "sse4.2")]
pub unsafe fn parse_market_data_sse42(data: &[u8]) -> ParseResult {
    let ptr = data.as_ptr();
    let len = data.len();
    
    let delimiter = _mm_set1_epi8(b'|' as i8);
    let mut result = ParseResult::new();
    let mut i = 0;
    
    while i + 16 <= len {
        let chunk = _mm_loadu_si128(ptr.add(i) as *const __m128i);
        let cmp = _mm_cmpeq_epi8(chunk, delimiter);
        let mask = _mm_movemask_epi8(cmp);
        
        if mask != 0 {
            result.add_positions_from_mask(mask as i32, i);
        }
        
        i += 16;
    }
    
    result.process_tail(&data[i..]);
    result
}
```

### 5.3 ARM64 SIMD Module

```rust
// src/arch/aarch64.rs

use std::arch::aarch64::*;

#[target_feature(enable = "neon")]
pub unsafe fn parse_market_data_neon(data: &[u8]) -> ParseResult {
    let ptr = data.as_ptr();
    let len = data.len();
    
    let delimiter = vdupq_n_u8(b'|');
    let mut result = ParseResult::new();
    let mut i = 0;
    
    while i + 16 <= len {
        let chunk = vld1q_u8(ptr.add(i));
        let cmp = vceqq_u8(chunk, delimiter);
        
        if vmaxvq_u8(cmp) != 0 {
            result.add_positions_from_neon(&cmp, i);
        }
        
        i += 16;
    }
    
    result.process_tail(&data[i..]);
    result
}
```

---

## 6. Runtime vs Compile-Time Detection

### 6.1 Decision Matrix

| Feature | Detection | Reason |
|---------|-----------|--------|
| OS | Compile-time | Static target |
| Architecture | Compile-time | Static target |
| SIMD level | Runtime | CPU-dependent |
| io_uring | Runtime | Kernel-dependent |
| User namespaces | Runtime | Kernel config |
| Sandbox backend | Compile-time + Runtime | Feature + availability |

### 6.2 Hybrid Detection Pattern

```rust
pub struct PlatformCapabilities {
    pub simd_level: SimdLevel,
    pub io_uring: bool,
    pub user_namespaces: bool,
}

impl PlatformCapabilities {
    pub fn detect() -> Self {
        Self {
            simd_level: detect_simd_level(),
            io_uring: detect_io_uring(),
            user_namespaces: detect_user_namespaces(),
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn detect_simd_level() -> SimdLevel {
    if is_x86_feature_detected!("avx512f") {
        SimdLevel::Avx512
    } else if is_x86_feature_detected!("avx2") {
        SimdLevel::Avx2
    } else if is_x86_feature_detected!("sse4.2") {
        SimdLevel::Sse42
    } else {
        SimdLevel::Sse2
    }
}

#[cfg(target_arch = "aarch64")]
fn detect_simd_level() -> SimdLevel {
    if std::arch::is_aarch64_feature_detected!("sve2") {
        SimdLevel::Sve2
    } else if std::arch::is_aarch64_feature_detected!("sve") {
        SimdLevel::Sve
    } else {
        SimdLevel::Neon
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdLevel {
    // x86_64
    Sse2,
    Sse42,
    Avx2,
    Avx512,
    // ARM64
    Neon,
    Sve,
    Sve2,
}
```

---

## 7. Build Script Configuration

### 7.1 build.rs

```rust
// build.rs

fn main() {
    // Print platform information
    println!("cargo:rustc-env=TARGET={}", std::env::var("TARGET").unwrap());
    println!("cargo:rustc-env=HOST={}", std::env::var("HOST").unwrap());
    
    // Platform-specific configuration
    #[cfg(target_os = "linux")]
    {
        // Check for io_uring support
        if let Ok(version) = std::fs::read_to_string("/proc/version") {
            println!("cargo:rustc-env=KERNEL_VERSION={}", version.trim());
        }
    }
    
    // Set cfg flags based on detection
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists() {
            println!("cargo:rustc-cfg=wsl2");
        }
    }
}
```

### 7.2 Generated Configuration

```rust
// Generated by build.rs, available at compile time
pub const KERNEL_VERSION: &str = env!("KERNEL_VERSION");
pub const TARGET_TRIPLE: &str = env!("TARGET");

pub const fn is_wsl2() -> bool {
    cfg!(wsl2)
}
```

---

## 8. Documentation of Platform Paths

### 8.1 Platform Path Comments

```rust
/// Parse market data from exchange feed.
///
/// # Platform-specific behavior
///
/// - **x86_64**: Uses AVX2 if available, falls back to SSE4.2, then scalar
/// - **ARM64**: Uses NEON if available, falls back to scalar
/// - **Other**: Uses scalar implementation
pub fn parse_market_data(data: &[u8]) -> ParseResult {
    // ... implementation
}
```

### 8.2 Feature Documentation

```rust
/// # Feature flags
///
/// | Flag | Description | Platforms |
/// |------|-------------|-----------|
/// | `io-uring` | Enable io_uring async runtime | Linux 5.1+ |
/// | `hft` | Enable HFT optimizations | Linux x86_64 |
/// | `wsl2` | Enable WSL2 interop | Linux (WSL2) |
```

---

## 9. CI/CD Configuration

### 9.1 Platform Matrix

```yaml
# .github/workflows/test.yml
strategy:
  matrix:
    include:
      - target: x86_64-unknown-linux-gnu
        os: ubuntu-latest
        features: io-uring,hft
      - target: aarch64-unknown-linux-gnu
        os: ubuntu-latest
        features: ""
      - target: x86_64-apple-darwin
        os: macos-latest
        features: ""
      - target: aarch64-apple-darwin
        os: macos-latest
        features: ""
```

### 9.2 Cross-Compilation

```yaml
- name: Install cross-compiler
  run: |
    rustup target add aarch64-unknown-linux-gnu
    sudo apt-get install gcc-aarch64-linux-gnu

- name: Build for ARM64
  run: cargo build --target aarch64-unknown-linux-gnu
```

---

## 10. Compliance

| Standard | Clause | Compliance |
|----------|--------|------------|
| REQ-6.4 | PAL | Full |
| Rust SOP | §1.2 | Full |
| IEEE 1016 | 8.2 | Full |

---

**Document Status:** APPROVED  
**Next Review:** After CI/CD setup
