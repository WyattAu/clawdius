//! Arena Allocator
//!
//! Zero-GC memory allocator for hot path allocations.

#![allow(unsafe_code)]

use std::cell::UnsafeCell;

/// Arena allocator for zero-GC hot path allocations.
///
/// Pre-allocates memory in chunks and provides fast allocation
/// without individual deallocations.
pub struct Arena<T> {
    chunks: UnsafeCell<Vec<Vec<T>>>,
    current: UnsafeCell<Vec<T>>,
    capacity: usize,
}

impl<T> Arena<T> {
    /// Creates a new arena with the specified chunk capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            chunks: UnsafeCell::new(Vec::new()),
            current: UnsafeCell::new(Vec::with_capacity(capacity)),
            capacity,
        }
    }

    /// Allocates a value in the arena and returns a mutable reference.
    ///
    /// The reference is valid until `clear` is called.
    #[allow(clippy::mut_from_ref)]
    pub fn alloc(&self, value: T) -> &mut T {
        unsafe {
            let current = &mut *self.current.get();
            if current.len() >= self.capacity {
                let old = std::mem::replace(current, Vec::with_capacity(self.capacity));
                (*self.chunks.get()).push(old);
            }
            current.push(value);
            current.last_mut().unwrap()
        }
    }

    /// Clears all allocations in the arena.
    pub fn clear(&mut self) {
        self.chunks.get_mut().clear();
        self.current.get_mut().clear();
    }

    /// Returns the total number of allocated items.
    #[must_use]
    pub fn len(&self) -> usize {
        unsafe {
            let chunks = &*self.chunks.get();
            let current = &*self.current.get();
            chunks.iter().map(|c| c.len()).sum::<usize>() + current.len()
        }
    }

    /// Returns `true` if the arena is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alloc() {
        let arena: Arena<i32> = Arena::new(10);
        let a = arena.alloc(1);
        let b = arena.alloc(2);
        assert_eq!(*a, 1);
        assert_eq!(*b, 2);
    }

    #[test]
    fn test_clear() {
        let mut arena: Arena<String> = Arena::new(10);
        arena.alloc("hello".to_string());
        assert_eq!(arena.len(), 1);
        arena.clear();
        assert!(arena.is_empty());
    }

    #[test]
    fn test_chunk_overflow() {
        let arena: Arena<i32> = Arena::new(2);
        arena.alloc(1);
        arena.alloc(2);
        arena.alloc(3);
        assert_eq!(arena.len(), 3);
    }
}
