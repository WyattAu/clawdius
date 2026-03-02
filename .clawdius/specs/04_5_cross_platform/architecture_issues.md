# Architecture Issues Analysis

**Document ID:** CP-ARCH-001  
**Version:** 1.0.0  
**Phase:** 4.5 (Cross-Platform Compatibility)  
**Status:** APPROVED  
**Created:** 2026-03-01  
**Trace To:** performance_requirements.md, lock_free_design.md

---

## 1. Overview

### 1.1 Purpose

This document analyzes architecture-specific considerations including endianness, word size, alignment, and SIMD availability for Clawdius.

### 1.2 Supported Architectures

| Architecture | Support Level | Notes |
|--------------|---------------|-------|
| x86_64 | Tier 1 | Primary development |
| aarch64 (ARM64) | Tier 2 | Apple Silicon, ARM servers |
| x86 | ❌ Not supported | 32-bit deprecated |
| armv7 | ❌ Not supported | 32-bit deprecated |

---

## 2. Endianness

### 2.1 Endianness Overview

| Architecture | Endianness | Notes |
|--------------|------------|-------|
| x86_64 | Little-endian | All platforms |
| aarch64 | Little-endian | All platforms |
| aarch64 (big-endian) | Bi-endian | Not supported |

### 2.2 Endianness Handling

Clawdius targets little-endian only:

```rust
#[cfg(not(target_endian = "little"))]
compile_error!("Clawdius requires little-endian architecture");
```

### 2.3 Network Byte Order

For network protocols (SBE, FIX), explicit conversion is required:

```rust
pub fn read_u16_be(data: &[u8]) -> u16 {
    u16::from_be_bytes([data[0], data[1]])
}

pub fn read_u32_be(data: &[u8]) -> u32 {
    u32::from_be_bytes([data[0], data[1], data[2], data[3]])
}

pub fn read_u64_le(data: &[u8]) -> u64 {
    u64::from_le_bytes([data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]])
}
```

### 2.4 SBE Protocol Endianness

Simple Binary Encoding (SBE) uses little-endian by default:

```rust
pub struct SbeMessage<'a> {
    data: &'a [u8],
}

impl<'a> SbeMessage<'a> {
    pub fn message_size(&self) -> u32 {
        u32::from_le_bytes([
            self.data[0],
            self.data[1],
            self.data[2],
            self.data[3],
        ])
    }
    
    pub fn schema_id(&self) -> u16 {
        u16::from_le_bytes([self.data[4], self.data[5]])
    }
}
```

---

## 3. Word Size

### 3.1 64-bit Only

Clawdius targets 64-bit architectures exclusively:

```rust
#[cfg(not(target_pointer_width = "64"))]
compile_error!("Clawdius requires 64-bit architecture");
```

### 3.2 Pointer Size Assumptions

| Type | Size (64-bit) | Notes |
|------|---------------|-------|
| `usize` | 8 bytes | Pointer-sized |
| `isize` | 8 bytes | Pointer-sized |
| `*const T` | 8 bytes | Raw pointer |
| `*mut T` | 8 bytes | Raw pointer |

### 3.3 Size-Sensitive Structures

```rust
pub struct RingBufferHeader {
    pub head: AtomicU64,
    pub tail: AtomicU64,
    pub capacity: u64,
    pub mask: u64,
    _padding: [u64; 12],
}

static_assertions::assert_eq_size!(RingBufferHeader, [u8; 128]);
```

---

## 4. Alignment Requirements

### 4.1 Natural Alignment

| Type | Alignment | Size |
|------|-----------|------|
| `u8` / `i8` | 1 byte | 1 byte |
| `u16` / `i16` | 2 bytes | 2 bytes |
| `u32` / `i32` | 4 bytes | 4 bytes |
| `u64` / `i64` | 8 bytes | 8 bytes |
| `u128` / `i128` | 16 bytes | 16 bytes |
| `f32` | 4 bytes | 4 bytes |
| `f64` | 8 bytes | 8 bytes |
| Pointer | 8 bytes | 8 bytes |

### 4.2 Cache Line Alignment

```rust
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;

pub const CACHE_LINE_SIZE: usize = 64;

#[repr(align(64))]
pub struct CachePadded<T>(pub T);

pub struct RingBuffer<T, const N: usize> {
    head: CachePadded<AtomicU64>,
    tail: CachePadded<AtomicU64>,
    buffer: [UnsafeCell<MaybeUninit<T>>; N],
}

static_assertions::assert_eq_align!(RingBufferHeader, [u8; 64]);
```

### 4.3 False Sharing Prevention

```rust
pub struct ShardedCounter {
    counters: [CachePadded<AtomicU64>; 16],
}

impl ShardedCounter {
    pub fn increment(&self, shard: usize) {
        self.counters[shard % 16].0.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn total(&self) -> u64 {
        self.counters.iter().map(|c| c.0.load(Ordering::Relaxed)).sum()
    }
}
```

### 4.4 SIMD Alignment

```rust
#[cfg(target_arch = "x86_64")]
mod simd_align {
    use std::arch::x86_64::__m256i;
    
    #[repr(align(32))]
    pub struct AvxBuffer([u8; 32]);
    
    impl AvxBuffer {
        pub fn as_m256i(&self) -> &__m256i {
            unsafe { &*(self.0.as_ptr() as *const __m256i) }
        }
    }
}

#[cfg(target_arch = "aarch64")]
mod simd_align {
    use std::arch::aarch64::uint8x16_t;
    
    #[repr(align(16))]
    pub struct NeonBuffer([u8; 16]);
    
    impl NeonBuffer {
        pub fn as_u8x16(&self) -> &uint8x16_t {
            unsafe { &*(self.0.as_ptr() as *const uint8x16_t) }
        }
    }
}
```

---

## 5. SIMD Availability

### 5.1 x86_64 SIMD Features

| Feature | Vector Width | Min CPU | Availability |
|---------|--------------|---------|--------------|
| SSE2 | 128-bit | Pentium 4 | Universal (x86_64) |
| SSE4.2 | 128-bit | Nehalem | ~100% |
| AVX | 256-bit | Sandy Bridge | ~95% |
| AVX2 | 256-bit | Haswell | ~90% |
| AVX-512 | 512-bit | Skylake-X | ~20% |

### 5.2 ARM64 SIMD Features

| Feature | Vector Width | Min CPU | Availability |
|---------|--------------|---------|--------------|
| NEON | 128-bit | All ARM64 | 100% |
| SVE | 128-2048-bit | Graviton 3+ | ~10% |
| SVE2 | 128-2048-bit | ARMv9 | New |

### 5.3 SIMD Feature Detection

```rust
#[cfg(target_arch = "x86_64")]
mod simd_detect {
    pub fn available_simd_level() -> SimdLevel {
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
    
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SimdLevel {
        Sse2,
        Sse42,
        Avx2,
        Avx512,
    }
}

#[cfg(target_arch = "aarch64")]
mod simd_detect {
    pub fn available_simd_level() -> SimdLevel {
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
        Neon,
        Sve,
        Sve2,
    }
}
```

### 5.4 SIMD Dispatcher Pattern

```rust
pub fn parse_market_data(data: &[u8]) -> ParseResult {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { parse_market_data_avx2(data) };
        }
    }
    
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { parse_market_data_neon(data) };
        }
    }
    
    parse_market_data_scalar(data)
}

fn parse_market_data_scalar(data: &[u8]) -> ParseResult {
    ParseResult::from_bytes(data)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn parse_market_data_avx2(data: &[u8]) -> ParseResult {
    use std::arch::x86_64::*;
    
    let ptr = data.as_ptr();
    let len = data.len();
    
    let mut result = ParseResult::new();
    let mut i = 0;
    
    while i + 32 <= len {
        let chunk = _mm256_loadu_si256(ptr.add(i) as *const __m256i);
        let delimiter = _mm256_set1_epi8(b'|' as i8);
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

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn parse_market_data_neon(data: &[u8]) -> ParseResult {
    use std::arch::aarch64::*;
    
    let ptr = data.as_ptr();
    let len = data.len();
    
    let mut result = ParseResult::new();
    let mut i = 0;
    
    let delimiter = vdupq_n_u8(b'|');
    
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

## 6. Memory Model Considerations

### 6.1 Memory Ordering

| Ordering | Use Case | Cost |
|----------|----------|------|
| `Relaxed` | Counters, statistics | Lowest |
| `Acquire` | Read side of synchronization | Medium |
| `Release` | Write side of synchronization | Medium |
| `AcqRel` | Read-modify-write | High |
| `SeqCst` | Global ordering | Highest |

### 6.2 Lock-Free Ring Buffer

```rust
pub struct SpscRingBuffer<T> {
    head: CachePadded<AtomicU64>,
    tail: CachePadded<AtomicU64>,
    buffer: Box<[UnsafeCell<MaybeUninit<T>>]>,
    mask: u64,
}

impl<T> SpscRingBuffer<T> {
    pub fn push(&self, value: T) -> Result<(), T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        
        if tail.wrapping_sub(head) >= self.buffer.len() as u64 {
            return Err(value);
        }
        
        let index = (tail & self.mask) as usize;
        unsafe {
            (*self.buffer[index].get()).write(value);
        }
        
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Ok(())
    }
    
    pub fn pop(&self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        
        if head == tail {
            return None;
        }
        
        let index = (head & self.mask) as usize;
        let value = unsafe { (*self.buffer[index].get()).assume_init_read() };
        
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Some(value)
    }
}
```

### 6.3 Architecture-Specific Memory Barriers

```rust
#[cfg(target_arch = "x86_64")]
mod barrier {
    use std::arch::asm;
    
    #[inline]
    pub fn sfence() {
        unsafe { asm!("sfence", options(nostack, preserves_flags)) }
    }
    
    #[inline]
    pub fn lfence() {
        unsafe { asm!("lfence", options(nostack, preserves_flags)) }
    }
    
    #[inline]
    pub fn mfence() {
        unsafe { asm!("mfence", options(nostack, preserves_flags)) }
    }
}

#[cfg(target_arch = "aarch64")]
mod barrier {
    use std::arch::asm;
    
    #[inline]
    pub fn dmb() {
        unsafe { asm!("dmb sy", options(nostack, preserves_flags)) }
    }
    
    #[inline]
    pub fn dsb() {
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)) }
    }
    
    #[inline]
    pub fn isb() {
        unsafe { asm!("isb", options(nostack, preserves_flags)) }
    }
}
```

---

## 7. HugePage Support (Linux)

### 7.1 HugePage Configuration

```rust
#[cfg(target_os = "linux")]
mod hugepage {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;
    
    const MAP_HUGETLB: i32 = 0x40000;
    const MAP_HUGE_2MB: i32 = 21 << 26;
    
    pub fn allocate_hugepage(size: usize) -> Result<*mut u8, std::io::Error> {
        let fd = -1isize;
        let addr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | MAP_HUGETLB | MAP_HUGE_2MB,
                fd,
                0,
            )
        };
        
        if addr == libc::MAP_FAILED {
            return Err(std::io::Error::last_os_error());
        }
        
        Ok(addr as *mut u8)
    }
}
```

### 7.2 Ring Buffer with HugePage

```rust
#[cfg(target_os = "linux")]
pub struct HugePageRingBuffer<T> {
    head: CachePadded<AtomicU64>,
    tail: CachePadded<AtomicU64>,
    buffer_ptr: *mut T,
    capacity: usize,
    hugepage_allocated: bool,
}

#[cfg(target_os = "linux")]
impl<T> HugePageRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        let size = capacity * std::mem::size_of::<T>();
        
        let (ptr, hugepage) = match hugepage::allocate_hugepage(size) {
            Ok(p) => (p as *mut T, true),
            Err(_) => {
                let layout = std::alloc::Layout::array::<T>(capacity).unwrap();
                let ptr = unsafe { std::alloc::alloc(layout) as *mut T };
                (ptr, false)
            }
        };
        
        Self {
            head: CachePadded(AtomicU64::new(0)),
            tail: CachePadded(AtomicU64::new(0)),
            buffer_ptr: ptr,
            capacity,
            hugepage_allocated: hugepage,
        }
    }
}
```

---

## 8. CPU Affinity and Isolation

### 8.1 CPU Affinity (Linux)

```rust
#[cfg(target_os = "linux")]
mod cpu_affinity {
    use libc::{cpu_set_t, sched_setaffinity, CPU_SET, CPU_ZERO};
    
    pub fn set_affinity(core: usize) -> Result<(), std::io::Error> {
        unsafe {
            let mut set: cpu_set_t = std::mem::zeroed();
            CPU_ZERO(&mut set);
            CPU_SET(core, &mut set);
            
            let result = sched_setaffinity(0, std::mem::size_of::<cpu_set_t>(), &set);
            if result != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }
    
    pub fn set_affinity_range(cores: &[usize]) -> Result<(), std::io::Error> {
        unsafe {
            let mut set: cpu_set_t = std::mem::zeroed();
            CPU_ZERO(&mut set);
            for &core in cores {
                CPU_SET(core, &mut set);
            }
            
            let result = sched_setaffinity(0, std::mem::size_of::<cpu_set_t>(), &set);
            if result != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }
}
```

### 8.2 HFT Core Configuration

```rust
pub struct HftCoreConfig {
    pub market_data_core: usize,
    pub signal_processing_cores: Vec<usize>,
    pub order_execution_core: usize,
}

impl HftCoreConfig {
    pub fn apply(&self) -> Result<(), std::io::Error> {
        #[cfg(target_os = "linux")]
        {
            cpu_affinity::set_affinity(self.market_data_core)?;
        }
        Ok(())
    }
    
    pub fn from_isolated_cores() -> Self {
        #[cfg(target_os = "linux")]
        {
            let isolated = read_isolated_cores();
            Self {
                market_data_core: isolated.first().copied().unwrap_or(0),
                signal_processing_cores: isolated.iter().skip(1).cloned().collect(),
                order_execution_core: isolated.get(1).copied().unwrap_or(1),
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Self {
                market_data_core: 0,
                signal_processing_cores: vec![1, 2],
                order_execution_core: 3,
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn read_isolated_cores() -> Vec<usize> {
    std::fs::read_to_string("/sys/devices/system/cpu/isolated")
        .ok()
        .and_then(|s| parse_cpu_list(&s))
        .unwrap_or_default()
}
```

---

## 9. Architecture-Specific Limits

| Resource | x86_64 | ARM64 | Notes |
|----------|--------|-------|-------|
| Max atomic size | 16 bytes | 16 bytes | CMPXCHG16B / LDXP |
| Cache line | 64 bytes | 64-256 bytes | Varies by CPU |
| Virtual address | 48-bit | 48-bit | With 5-level paging: 57-bit |
| Physical address | 46-bit | 48-bit | Varies by implementation |

---

## 10. Compliance

| Standard | Clause | Compliance |
|----------|--------|------------|
| IEEE 754 | - | Full |
| ISO/IEC 9899 | Alignment | Full |
| Rust SOP | §3.2 | Full |

---

**Document Status:** APPROVED  
**Next Review:** After ARM64 testing
