//! HFT Ring Buffer - Lock-free SPSC queue for market data
//!
//! Per YP-HFT-BROKER-001:
//! - Buffer size: 2^20 entries (1,048,576)
//! - Memory ordering: Acquire/Release (NOT SeqCst)
//! - CachePadded for false-sharing elimination
//! - WCET: < 100ns per operation

use std::alloc::{alloc, dealloc, Layout};
use std::fmt;
use std::ptr::NonNull;
use std::sync::atomic::{fence, AtomicU64, Ordering};

const CACHE_LINE_SIZE: usize = 64;
const DEFAULT_BUFFER_SIZE: usize = 1 << 20;

#[repr(C, align(64))]
struct CachePadded<T> {
    value: T,
}

impl<T> CachePadded<T> {
    fn new(value: T) -> Self {
        Self { value }
    }
}

#[repr(C, align(128))]
struct AlignedAtomicU64 {
    value: AtomicU64,
}

impl AlignedAtomicU64 {
    fn new(v: u64) -> Self {
        Self {
            value: AtomicU64::new(v),
        }
    }

    fn load(&self, order: Ordering) -> u64 {
        self.value.load(order)
    }

    fn store(&self, val: u64, order: Ordering) {
        self.value.store(val, order)
    }
}

/// SAFETY: AlignedAtomicU64 is Send because AtomicU64 is Send
/// and we only use atomic operations.
#[expect(unsafe_code)]
unsafe impl Send for AlignedAtomicU64 {}
/// SAFETY: AlignedAtomicU64 is Sync because AtomicU64 is Sync
/// and we only use atomic operations.
#[expect(unsafe_code)]
unsafe impl Sync for AlignedAtomicU64 {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingBufferError {
    BufferFull,
    BufferEmpty,
    AllocationFailed,
    InvalidCapacity,
}

#[derive(Debug, Clone, Copy)]
pub struct MarketDataMessage {
    pub msg_type: u8,
    pub symbol_id: u32,
    pub price: i64,
    pub quantity: u32,
    pub timestamp_ns: u64,
}

impl Default for MarketDataMessage {
    fn default() -> Self {
        Self {
            msg_type: 0,
            symbol_id: 0,
            price: 0,
            quantity: 0,
            timestamp_ns: 0,
        }
    }
}

pub struct RingBuffer {
    buffer: NonNull<MarketDataMessage>,
    capacity: usize,
    mask: usize,
    head: AlignedAtomicU64,
    tail: AlignedAtomicU64,
}

impl fmt::Debug for RingBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingBuffer")
            .field("capacity", &self.capacity)
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

/// SAFETY: RingBuffer is Send because the buffer pointer is only accessed
/// through atomic operations and the buffer is never shared across threads
/// simultaneously (SPSC pattern).
#[expect(unsafe_code)]
unsafe impl Send for RingBuffer {}
/// SAFETY: RingBuffer is Sync because all accesses to the buffer pointer
/// are protected by atomic operations with proper Acquire/Release ordering.
#[expect(unsafe_code)]
unsafe impl Sync for RingBuffer {}

impl RingBuffer {
    // VERIFY: PROP-RB-008 — Power-of-2 capacity enforced at construction
    // Proof: proof_ring_buffer.lean::power_of_two_masking
    // Status: VERIFIED
    pub fn new(capacity: usize) -> Result<Self, RingBufferError> {
        if !capacity.is_power_of_two() {
            return Err(RingBufferError::InvalidCapacity);
        }

        let layout = Layout::array::<MarketDataMessage>(capacity)
            .map_err(|_| RingBufferError::AllocationFailed)?;

        // SAFETY: Layout is valid (computed above) and we check for allocation failure.
        #[expect(unsafe_code)]
        let ptr = unsafe { alloc(layout) };
        let buffer =
            NonNull::new(ptr as *mut MarketDataMessage).ok_or(RingBufferError::AllocationFailed)?;

        Ok(Self {
            buffer,
            capacity,
            mask: capacity - 1,
            head: AlignedAtomicU64::new(0),
            tail: AlignedAtomicU64::new(0),
        })
    }

    pub fn with_default_capacity() -> Result<Self, RingBufferError> {
        Self::new(DEFAULT_BUFFER_SIZE)
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    // VERIFY: PROP-RB-001 — Write preserves ring buffer invariants (bounded length, valid head)
    // Proof: proof_ring_buffer.lean::write_preserves_invariants
    // Status: VERIFIED
    pub fn try_write(&self, message: MarketDataMessage) -> Result<(), RingBufferError> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let next_head = head.wrapping_add(1);

        if next_head.wrapping_sub(tail) > self.capacity as u64 {
            return Err(RingBufferError::BufferFull);
        }

        let index = (head as usize) & self.mask;
        // SAFETY: index is within bounds (masked by capacity-1) and we use
        // write_volatile to ensure the write is not optimized away.
        #[expect(unsafe_code)]
        unsafe {
            std::ptr::write_volatile(self.buffer.as_ptr().add(index), message);
        }

        fence(Ordering::Release);
        self.head.store(next_head, Ordering::Release);

        Ok(())
    }

    // VERIFY: PROP-RB-003 — Read preserves ring buffer invariants (bounded length, valid tail)
    // Proof: proof_ring_buffer.lean::read_preserves_invariants
    // Status: VERIFIED
    pub fn try_read(&self) -> Result<MarketDataMessage, RingBufferError> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if tail >= head {
            return Err(RingBufferError::BufferEmpty);
        }

        let index = (tail as usize) & self.mask;
        fence(Ordering::Acquire);

        // SAFETY: index is within bounds (masked by capacity-1) and we use
        // read_volatile to ensure the read is not optimized away.
        #[expect(unsafe_code)]
        let message = unsafe { std::ptr::read_volatile(self.buffer.as_ptr().add(index)) };
        self.tail.store(tail.wrapping_add(1), Ordering::Release);

        Ok(message)
    }

    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        head.wrapping_sub(tail) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Relaxed) == self.tail.load(Ordering::Relaxed)
    }

    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        head.wrapping_sub(tail) >= self.capacity as u64
    }

    pub fn available_space(&self) -> usize {
        self.capacity - self.len()
    }
}

// VERIFY: PROP-RB-009 — Safe deallocation via matching layout from new()
// Proof: N/A (destructive operation, ensured by construction)
// Status: AXIOM
impl Drop for RingBuffer {
    fn drop(&mut self) {
        let layout = Layout::array::<MarketDataMessage>(self.capacity).unwrap();
        // SAFETY: Layout matches the allocation in new(), and we own the buffer.
        #[expect(unsafe_code)]
        unsafe {
            dealloc(self.buffer.as_ptr() as *mut u8, layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_creation() {
        let rb = RingBuffer::new(16).unwrap();
        assert_eq!(rb.capacity(), 16);
        assert!(rb.is_empty());
    }

    #[test]
    fn test_write_read_cycle() {
        let rb = RingBuffer::new(16).unwrap();
        let msg = MarketDataMessage {
            msg_type: 1,
            symbol_id: 42,
            price: 10000,
            quantity: 100,
            timestamp_ns: 1234567890123456,
        };

        rb.try_write(msg).unwrap();
        assert!(!rb.is_empty());

        let read_msg = rb.try_read().unwrap();
        assert_eq!(read_msg.msg_type, msg.msg_type);
        assert_eq!(read_msg.symbol_id, msg.symbol_id);
        assert_eq!(read_msg.price, msg.price);
        assert!(rb.is_empty());
    }

    #[test]
    fn test_buffer_full() {
        let rb = RingBuffer::new(16).unwrap();
        let msg = MarketDataMessage::default();

        for _ in 0..16 {
            rb.try_write(msg).unwrap();
        }

        assert!(rb.is_full());
        assert!(matches!(
            rb.try_write(msg),
            Err(RingBufferError::BufferFull)
        ));
    }

    #[test]
    fn test_buffer_empty() {
        let rb = RingBuffer::new(16).unwrap();
        assert!(matches!(rb.try_read(), Err(RingBufferError::BufferEmpty)));
    }

    #[test]
    fn test_wraparound() {
        let rb = RingBuffer::new(16).unwrap();

        for i in 0..16u64 {
            let msg = MarketDataMessage {
                msg_type: 1,
                symbol_id: i as u32,
                price: i as i64,
                quantity: 1,
                timestamp_ns: i,
            };
            rb.try_write(msg).unwrap();
        }

        for i in 0..16u64 {
            let msg = rb.try_read().unwrap();
            assert_eq!(msg.symbol_id, i as u32);
        }

        assert!(rb.is_empty());

        let msg = MarketDataMessage {
            msg_type: 2,
            symbol_id: 999,
            price: 999,
            quantity: 999,
            timestamp_ns: 999,
        };
        rb.try_write(msg).unwrap();
        let read = rb.try_read().unwrap();
        assert_eq!(read.symbol_id, 999);
    }

    #[test]
    fn test_invalid_capacity() {
        assert!(matches!(
            RingBuffer::new(15),
            Err(RingBufferError::InvalidCapacity)
        ));
        assert!(matches!(
            RingBuffer::new(17),
            Err(RingBufferError::InvalidCapacity)
        ));
    }

    #[test]
    fn test_boundary_full_after_write() {
        let rb = RingBuffer::new(16).unwrap();
        let msg = MarketDataMessage::default();

        for _ in 0..15 {
            rb.try_write(msg).unwrap();
        }
        assert!(!rb.is_full());

        rb.try_write(msg).unwrap();
        assert!(rb.is_full());
    }

    #[test]
    fn test_consecutive_operations() {
        let rb = RingBuffer::new(1024).unwrap();

        for round in 0..10 {
            for i in 0..100u64 {
                let msg = MarketDataMessage {
                    msg_type: (round * 100 + i) as u8,
                    symbol_id: i as u32,
                    price: i as i64 * 100,
                    quantity: i as u32,
                    timestamp_ns: round * 1000000 + i,
                };
                rb.try_write(msg).unwrap();
            }

            for i in 0..100u64 {
                let msg = rb.try_read().unwrap();
                assert_eq!(msg.symbol_id, i as u32);
            }

            assert!(rb.is_empty());
        }
    }
}
