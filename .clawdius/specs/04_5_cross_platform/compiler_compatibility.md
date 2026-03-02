# Compiler Compatibility Analysis

**Document ID:** CP-COMPILER-001  
**Version:** 1.0.0  
**Phase:** 4.5 (Cross-Platform Compatibility)  
**Status:** APPROVED  
**Created:** 2026-03-01  
**Trace To:** rust_sop.md, VERSION.md

---

## 1. Overview

### 1.1 Purpose

This document specifies compiler requirements, version constraints, and compatibility considerations for Clawdius.

### 1.2 Compiler Stack

| Component | Minimum Version | Recommended | Notes |
|-----------|-----------------|-------------|-------|
| Rust | 1.85.0 | Latest stable | 2024 Edition |
| LLVM | 19.0 | 19.0+ | Via rustc |
| Linker | mold 2.0 | mold 2.30+ | Linux only |
| cargo | 1.85.0 | Latest stable | Same as rustc |

---

## 2. Rust Version Requirements

### 2.1 Minimum Supported Rust Version (MSRV)

```toml
# Cargo.toml
[package]
rust-version = "1.85"
edition = "2024"
```

**Rationale for Rust 1.85+:**
- Rust 2024 Edition support
- `lazy_cell` in std (1.80+)
- `lazy_cell` stabilized features
- Improved const generics
- Async fn in traits stabilization
- `return_position_impl_trait_in_trait` stabilized

### 2.2 Edition 2024 Features Used

```rust
#![feature(
    lazy_cell,
    let_chains,
    if_let_guard,
    async_fn_in_trait,
    return_position_impl_trait_in_trait,
)]

#![allow(
    async_fn_in_trait,
    return_position_impl_trait_in_trait,
)]
```

### 2.3 Rust Version Detection

```rust
#[cfg(not(rustver_check(1.85)))]
compile_error!("Clawdius requires Rust 1.85.0 or later for 2024 Edition support");
```

---

## 3. LLVM Requirements

### 3.1 LLVM Version Matrix

| Rust Version | LLVM Version | Notes |
|--------------|--------------|-------|
| 1.85.0 | 19.0 | 2024 Edition |
| 1.84.0 | 19.0 | - |
| 1.83.0 | 18.1 | - |

### 3.2 LLVM Features Required

| Feature | LLVM Version | Purpose |
|---------|--------------|---------|
| PGO | 17.0+ | Profile-guided optimization |
| BOLT | 17.0+ | Post-link optimization |
| LTO | All | Link-time optimization |
| SLP Vectorizer | All | SIMD auto-vectorization |

### 3.3 Codegen Options

```toml
# .cargo/config.toml
[profile.release]
codegen-units = 1
lto = "fat"
panic = "abort"
strip = true

[profile.release.package."*"]
codegen-units = 16
opt-level = 3
```

---

## 4. Compiler Extensions

### 4.1 Platform-Specific Intrinsics

#### 4.1.1 x86_64 SIMD (AVX2)

```rust
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
mod avx2 {
    use std::arch::x86_64::*;
    
    #[target_feature(enable = "avx2")]
    pub unsafe fn parse_sbe_message_avx2(data: &[u8]) -> ParsedMessage {
        let ptr = data.as_ptr();
        let vec = _mm256_loadu_si256(ptr as *const __m256i);
        
        let mask = _mm256_set1_epi8(b'|' as i8);
        let cmp = _mm256_cmpeq_epi8(vec, mask);
        let mask = _mm256_movemask_epi8(cmp);
        
        ParsedMessage { field_positions: mask }
    }
}
```

#### 4.1.2 ARM64 SIMD (NEON)

```rust
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod neon {
    use std::arch::aarch64::*;
    
    #[target_feature(enable = "neon")]
    pub unsafe fn parse_sbe_message_neon(data: &[u8]) -> ParsedMessage {
        let ptr = data.as_ptr();
        let vec = vld1q_u8(ptr);
        
        let delimiter = vdupq_n_u8(b'|');
        let cmp = vceqq_u8(vec, delimiter);
        
        ParsedMessage { field_positions: vgetq_lane_u64(vreinterpretq_u64_u8(cmp), 0) }
    }
}
```

#### 4.1.3 Runtime Feature Detection

```rust
#[cfg(target_arch = "x86_64")]
fn parse_sbe_message(data: &[u8]) -> ParsedMessage {
    if is_x86_feature_detected!("avx2") {
        unsafe { avx2::parse_sbe_message_avx2(data) }
    } else {
        parse_sbe_message_fallback(data)
    }
}

#[cfg(target_arch = "aarch64")]
fn parse_sbe_message(data: &[u8]) -> ParsedMessage {
    if std::arch::is_aarch64_feature_detected!("neon") {
        unsafe { neon::parse_sbe_message_neon(data) }
    } else {
        parse_sbe_message_fallback(data)
    }
}

fn parse_sbe_message_fallback(data: &[u8]) -> ParsedMessage {
    ParsedMessage::from_bytes(data)
}
```

---

## 5. Link-Time Optimization (LTO)

### 5.1 LTO Configuration

```toml
# Cargo.toml
[profile.release]
lto = "fat"
codegen-units = 1

[profile.release-opt]
inherits = "release"
lto = "thin"
codegen-units = 16
```

### 5.2 LTO Trade-offs

| LTO Type | Build Time | Binary Size | Performance |
|----------|------------|-------------|-------------|
| None | Fast | Large | Baseline |
| Thin | Medium | Medium | +5-10% |
| Fat | Slow | Smallest | +10-20% |

### 5.3 Linker Selection

```toml
# .cargo/config.toml (Linux)
[target.x86_64-unknown-linux-gnu]
linker = "clang"

[target.x86_64-unknown-linux-gnu.'cfg(not(debug_assertions))']
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

# .cargo/config.toml (macOS)
[target.aarch64-apple-darwin]
linker = "clang"

[target.x86_64-apple-darwin]
linker = "clang"
```

### 5.4 mold Linker (Linux)

```bash
# Install mold on Linux
apt install mold  # Debian/Ubuntu
dnf install mold  # Fedora
pacman -S mold    # Arch

# Verify mold version
mold --version  # Requires 2.0+
```

---

## 6. Profile-Guided Optimization (PGO)

### 6.1 PGO Workflow

```bash
# Step 1: Build instrumented binary
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" \
  cargo build --release

# Step 2: Run representative workload
./target/release/clawdius --pgo-training

# Step 3: Merge profile data
llvm-profdata merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data/*.profraw

# Step 4: Build optimized binary
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" \
  cargo build --release
```

### 6.2 PGO CI Integration

```yaml
# .github/workflows/pgo.yml
name: PGO Build
on:
  release:
    types: [published]

jobs:
  pgo:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install LLVM tools
        run: rustup component add llvm-tools-preview
      
      - name: Build instrumented
        run: |
          RUSTFLAGS="-Cprofile-generate=target/pgo" \
            cargo build --release
      
      - name: Run training
        run: |
          ./target/release/clawdius test --pgo-mode
          ./target/release/clawdius benchmark --quick
      
      - name: Merge profiles
        run: |
          llvm-profdata merge -o target/pgo.profdata target/pgo/*.profraw
      
      - name: Build optimized
        run: |
          RUSTFLAGS="-Cprofile-use=target/pgo.profdata" \
            cargo build --release
      
      - name: Upload binary
        uses: actions/upload-artifact@v4
        with:
          name: clawdius-pgo
          path: target/release/clawdius
```

---

## 7. BOLT Optimization (Linux x86_64)

### 7.1 BOLT Overview

BOLT (Binary Optimization and Layout Tool) is a post-link optimizer for x86_64 Linux binaries.

### 7.2 BOLT Workflow

```bash
# Step 1: Build with debug info and relocations
cargo build --release

# Step 2: Instrument with BOLT
llvm-bolt target/release/clawdius \
  -instrument \
  -instrumentation-file=target/bolt.fdata \
  -o target/clawdius.instrumented

# Step 3: Run workload
./target/clawdius.instrumented test
./target/clawdius.instrumented benchmark

# Step 4: Optimize with collected data
llvm-bolt target/release/clawdius \
  -data=target/bolt.fdata \
  -reorder-blocks=cache+ \
  -reorder-functions=hotsite \
  -split-functions=3 \
  -split-all-cold \
  -dyno-stats \
  -o target/clawdius.optimized

# Step 5: Replace binary
cp target/clawdius.optimized target/release/clawdius
```

### 7.3 BOLT Requirements

| Requirement | Minimum |
|-------------|---------|
| LLVM BOLT | 17.0+ |
| Linux kernel | 4.5+ |
| Architecture | x86_64 only |

---

## 8. Compiler Flags Summary

### 8.1 Release Build Flags

```bash
# Full optimization flags
RUSTFLAGS="\
  -C target-cpu=native \
  -C opt-level=3 \
  -C lto=fat \
  -C codegen-units=1 \
  -C panic=abort \
  -C strip=symbols \
  -C embed-bitcode=no \
  -Z location-detail=none \
" cargo build --release
```

### 8.2 Platform-Specific Flags

```toml
# .cargo/config.toml

[target.x86_64-unknown-linux-gnu]
rustflags = [
    "-C", "target-cpu=native",
    "-C", "link-arg=-fuse-ld=mold",
]

[target.aarch64-apple-darwin]
rustflags = [
    "-C", "target-cpu=apple-m1",
]

[target.aarch64-unknown-linux-gnu]
rustflags = [
    "-C", "target-cpu=native",
]
```

### 8.3 HFT Mode Flags

```toml
# For HFT builds, maximum optimization
[profile.hft]
inherits = "release"
lto = "fat"
codegen-units = 1
opt-level = 3
debug = false
strip = true
panic = "abort"

[profile.hft.build-override]
opt-level = 3
```

---

## 9. Compiler Warnings

### 9.1 Enforced Warnings

```rust
#![deny(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_op_in_unsafe_fn,
    unused_import_braces,
    unused_qualifications,
)]

#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
)]
```

### 9.2 Allowed Warnings

```rust
#![allow(
    clippy::module_name_repetitions,
    clippy::multiple_crate_versions,
    clippy::missing_errors_doc,
)]
```

---

## 10. Compiler Compatibility Matrix

| Compiler | Version | Status | Notes |
|----------|---------|--------|-------|
| rustc stable | 1.85+ | ✅ Supported | Primary |
| rustc beta | Latest | ⚠️ Testing | CI only |
| rustc nightly | Latest | ❌ Not supported | Dev experiments |
| rustc < 1.85 | Any | ❌ Not supported | MSRV policy |

---

## 11. Build Toolchain Verification

### 11.1 Toolchain Check Script

```bash
#!/bin/bash
# scripts/check-toolchain.sh

set -e

RUST_MIN_VERSION="1.85.0"
RUST_VERSION=$(rustc --version | grep -oP '\d+\.\d+\.\d+')

# Version comparison
version_ge() {
    printf '%s\n%s\n' "$1" "$2" | sort -V -C
}

if ! version_ge "$RUST_VERSION" "$RUST_MIN_VERSION"; then
    echo "Error: Rust $RUST_MIN_VERSION required, found $RUST_VERSION"
    exit 1
fi

echo "✓ Rust $RUST_VERSION (>= $RUST_MIN_VERSION)"

# Check cargo
cargo --version

# Check linker (Linux only)
if [[ "$OSTYPE" == "linux"* ]]; then
    if command -v mold &> /dev/null; then
        echo "✓ mold linker: $(mold --version)"
    else
        echo "⚠ mold not found, using default linker"
    fi
fi

# Check LLVM tools
if rustup component list | grep -q llvm-tools; then
    echo "✓ llvm-tools installed"
else
    echo "⚠ llvm-tools not installed (required for PGO/BOLT)"
fi
```

### 11.2 CI Toolchain Setup

```yaml
# .github/workflows/ci.yml
- name: Setup Rust
  uses: dtolnay/rust-toolchain@stable
  with:
    toolchain: 1.85
    components: clippy, rustfmt, llvm-tools-preview
    targets: x86_64-unknown-linux-gnu, aarch64-apple-darwin
```

---

## 12. Compliance

| Standard | Clause | Compliance |
|----------|--------|------------|
| Rust SOP | §1.1 | Full |
| IEEE 1016 | 7.2 | Full |
| MSRV Policy | - | Full |

---

**Document Status:** APPROVED  
**Next Review:** After Rust 2024 Edition stabilization
