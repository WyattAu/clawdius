//! Lock-free SPSC Ring Buffer
//!
//! Single-producer single-consumer ring buffer with cache-padded atomics
//! for optimal performance in HFT scenarios.

#![allow(unsafe_code)]

use crossbeam_utils::CachePadded;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Lock-free single-producer single-consumer ring buffer.
///
/// Uses cache-padded atomics to prevent false sharing between
/// the head and tail pointers.
pub struct RingBuffer<T, const N: usize> {
    buffer: UnsafeCell<[MaybeUninit<T>; N]>,
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
}

impl<T, const N: usize> RingBuffer<T, N> {
    /// Creates a new empty ring buffer.
    ///
    /// # Panics
    ///
    /// Panics if N is not a power of 2.
    #[must_use]
    pub fn new() -> Self {
        assert!(N.is_power_of_two(), "Capacity must be a power of 2");
        Self {
            buffer: UnsafeCell::new(unsafe { MaybeUninit::uninit().assume_init() }),
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
        }
    }

    /// Pushes an item into the buffer.
    ///
    /// Returns `Err(item)` if the buffer is full.
    pub fn push(&self, item: T) -> Result<(), T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (tail + 1) & (N - 1);

        if next_tail == self.head.load(Ordering::Acquire) {
            return Err(item);
        }

        unsafe {
            (*self.buffer.get())[tail].write(item);
        }
        self.tail.store(next_tail, Ordering::Release);
        Ok(())
    }

    /// Pops an item from the buffer.
    ///
    /// Returns `None` if the buffer is empty.
    pub fn pop(&self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);

        if head == self.tail.load(Ordering::Acquire) {
            return None;
        }

        let item = unsafe { (*self.buffer.get())[head].assume_init_read() };
        self.head.store((head + 1) & (N - 1), Ordering::Release);
        Some(item)
    }

    /// Returns the number of items in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        (tail.wrapping_sub(head)) & (N - 1)
    }

    /// Returns `true` if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Relaxed) == self.tail.load(Ordering::Relaxed)
    }

    /// Returns the capacity of the buffer.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N - 1
    }
}

impl<T, const N: usize> Default for RingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for RingBuffer<T, N> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

unsafe impl<T: Send, const N: usize> Sync for RingBuffer<T, N> {}
unsafe impl<T: Send, const N: usize> Send for RingBuffer<T, N> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop() {
        let buffer: RingBuffer<i32, 16> = RingBuffer::new();
        assert!(buffer.push(1).is_ok());
        assert!(buffer.push(2).is_ok());
        assert_eq!(buffer.pop(), Some(1));
        assert_eq!(buffer.pop(), Some(2));
        assert_eq!(buffer.pop(), None);
    }

    #[test]
    fn test_capacity() {
        let buffer: RingBuffer<i32, 4> = RingBuffer::new();
        assert_eq!(buffer.capacity(), 3);
        assert!(buffer.push(1).is_ok());
        assert!(buffer.push(2).is_ok());
        assert!(buffer.push(3).is_ok());
        assert!(buffer.push(4).is_err());
    }

    #[test]
    fn test_len_and_empty() {
        let buffer: RingBuffer<i32, 8> = RingBuffer::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        buffer.push(1).unwrap();
        assert!(!buffer.is_empty());
        assert_eq!(buffer.len(), 1);
    }
}
