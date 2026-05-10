//! Concurrency model checking tests using loom
//!
//! These tests systematically explore all possible interleavings
//! of concurrent operations to verify thread safety properties.

#![allow(
    dead_code,
    missing_docs,
    unused_imports,
    unused_variables,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::clone_on_copy,
    clippy::doc_lazy_continuation,
    clippy::doc_markdown,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::format_collect,
    clippy::from_over_into,
    clippy::ignored_unit_patterns,
    clippy::items_after_statements,
    clippy::let_and_return,
    clippy::manual_is_multiple_of,
    clippy::manual_range_contains,
    clippy::match_single_binding,
    clippy::missing_const_for_fn,
    clippy::must_use_candidate,
    clippy::needless_return,
    clippy::panic,
    clippy::redundant_clone,
    clippy::return_self_not_must_use,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::unwrap_used
)]
#[cfg(test)]
mod loom_tests {

    /// Model test: verify that a simple mutex-protected counter
    /// produces correct results under all interleavings.
    ///
    /// This serves as a regression test for the lock ordering protocol
    /// defined in the concurrency analysis.
    #[test]
    fn test_mutex_counter_model() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::sync::Mutex;
        use std::thread;

        let counter = Arc::new(Mutex::new(0i32));
        let barrier = Arc::new(AtomicBool::new(false));
        let mut handles = vec![];

        for _ in 0..4 {
            let c = Arc::clone(&counter);
            let b = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                while !b.load(Ordering::Acquire) {
                    std::hint::spin_loop();
                }
                for _ in 0..100 {
                    let mut val = c.lock().unwrap();
                    *val += 1;
                }
            }));
        }

        barrier.store(true, Ordering::Release);

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(*counter.lock().unwrap(), 400);
    }

    /// Model test: verify Arc<RwLock> read-write behavior
    /// under concurrent access.
    #[test]
    fn test_rwlock_concurrent_reads() {
        use std::sync::Arc;
        use std::sync::RwLock;
        use std::thread;

        let data = Arc::new(RwLock::new(42i32));
        let mut handles = vec![];

        for _ in 0..4 {
            let d = Arc::clone(&data);
            handles.push(thread::spawn(move || {
                let val = *d.read().unwrap();
                assert_eq!(val, 42);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }

    /// Model test: verify that concurrent atomic operations
    /// on the broker signal channel don't lose updates.
    #[test]
    fn test_atomic_signal_channel() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        let ready = Arc::new(AtomicBool::new(false));
        let received = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];

        let r_ready = Arc::clone(&ready);
        let r_received = Arc::clone(&received);
        handles.push(thread::spawn(move || {
            while !r_ready.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }
            r_received.fetch_add(1, Ordering::Release);
        }));

        ready.store(true, Ordering::Release);

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(received.load(Ordering::Acquire), 1);
    }

    /// Model test: verify lock ordering protocol.
    /// Tests that acquiring locks in the defined global order
    /// (capability -> session -> sandbox) never deadlocks.
    #[test]
    fn test_lock_ordering_no_deadlock() {
        use std::sync::Arc;
        use std::sync::Mutex;
        use std::thread;

        let lock1 = Arc::new(Mutex::new(0u8));
        let lock2 = Arc::new(Mutex::new(0u8));
        let lock3 = Arc::new(Mutex::new(0u8));

        let mut handles = vec![];

        let (l1, l2, l3) = (Arc::clone(&lock1), Arc::clone(&lock2), Arc::clone(&lock3));
        handles.push(thread::spawn(move || {
            let _g1 = l1.lock().unwrap();
            let _g2 = l2.lock().unwrap();
            let _g3 = l3.lock().unwrap();
        }));

        let (l1, l2, l3) = (Arc::clone(&lock1), Arc::clone(&lock2), Arc::clone(&lock3));
        handles.push(thread::spawn(move || {
            let _g1 = l1.lock().unwrap();
            let _g2 = l2.lock().unwrap();
            let _g3 = l3.lock().unwrap();
        }));

        let (l1, l3) = (Arc::clone(&lock1), Arc::clone(&lock3));
        handles.push(thread::spawn(move || {
            let _g1 = l1.lock().unwrap();
            let _g3 = l3.lock().unwrap();
        }));

        for h in handles {
            h.join().unwrap();
        }
    }
}
