use proptest::prelude::*;
use std::collections::HashSet;

use clawdius_core::broker::{
    ExecutionAdapter, MarketUpdate, Order, OrderSide, RejectReason, RingBuffer, RiskDecision,
    RiskParams, SimulatedExecution, SimulatedFeed, Wallet, WalletGuard,
};
use clawdius_core::capability::{CapabilityToken, Permission, ResourceScope};
use clawdius_core::nexus::{
    all_phases, get_phase_by_id, Artifact, ArtifactType, Checkpoint, FsmState, PhaseCategory,
    PhaseId, RecoveryEvent, SessionId, SessionStatus, StatePersistence,
};

fn all_permissions() -> Vec<Permission> {
    vec![
        Permission::FsRead,
        Permission::FsWrite,
        Permission::NetTcp,
        Permission::NetUdp,
        Permission::ExecSpawn,
        Permission::SecretAccess,
        Permission::EnvRead,
        Permission::EnvWrite,
    ]
}

fn any_permission_strategy() -> impl Strategy<Value = Permission> {
    prop_oneof![
        Just(Permission::FsRead),
        Just(Permission::FsWrite),
        Just(Permission::NetTcp),
        Just(Permission::NetUdp),
        Just(Permission::ExecSpawn),
        Just(Permission::SecretAccess),
        Just(Permission::EnvRead),
        Just(Permission::EnvWrite),
    ]
}

fn permission_set_strategy() -> impl Strategy<Value = HashSet<Permission>> {
    proptest::collection::hash_set(any_permission_strategy(), 0..=8)
}

// ---------------------------------------------------------------------------
// Module 1: Ring Buffer Properties
// ---------------------------------------------------------------------------

mod ring_buffer_properties {
    use super::*;

    #[test]
    fn sequential_consistency() {
        let buffer: RingBuffer<u64, 64> = RingBuffer::new();

        let expected: Vec<u64> = (0..50).collect();
        for &val in &expected {
            assert!(
                buffer.push(val).is_ok(),
                "push should succeed for sequential writes"
            );
        }
        let mut actual = Vec::new();
        while let Some(val) = buffer.pop() {
            actual.push(val);
        }
        assert_eq!(actual, expected, "FIFO order must be preserved");
        assert!(buffer.is_empty());
    }

    #[test]
    fn wraparound_correctness() {
        const N: usize = 16;
        let buffer: RingBuffer<u32, N> = RingBuffer::new();
        let cap = buffer.capacity();

        for i in 0..cap {
            assert!(buffer.push(i as u32).is_ok());
        }
        assert!(buffer.push(999).is_err(), "buffer must be full");

        let popped = buffer.pop().expect("pop after fill");
        assert_eq!(popped, 0, "first item out must be 0");

        assert!(buffer.push(1000).is_ok(), "push after one pop must succeed");

        let mut remaining: Vec<u32> = Vec::new();
        while let Some(v) = buffer.pop() {
            remaining.push(v);
        }
        let expected: Vec<u32> = (1..cap as u32).chain(std::iter::once(1000)).collect();
        assert_eq!(remaining, expected, "wraparound must preserve FIFO");
    }

    #[test]
    fn no_overflow_on_burst() {
        const N: usize = 32;
        let buffer: RingBuffer<i32, N> = RingBuffer::new();
        let cap = buffer.capacity();

        let mut accepted = 0usize;
        let mut rejected = 0usize;
        for i in 0..(2 * cap + 5) {
            if buffer.push(i as i32).is_ok() {
                accepted += 1;
            } else {
                rejected += 1;
            }
        }
        assert_eq!(accepted, cap, "exactly capacity items accepted");
        assert_eq!(rejected, cap + 5, "remaining items rejected");
        assert_eq!(buffer.len(), cap);
    }

    proptest! {
        #[test]
        fn len_equals_writes_minus_reads(ops in proptest::collection::vec(
            proptest::bool::ANY, 0..200
        )) {
            const N: usize = 64;
            let buffer: RingBuffer<u32, N> = RingBuffer::new();
            let mut total_writes: usize = 0;
            let mut total_reads: usize = 0;

            for (i, is_write) in ops.iter().copied().enumerate() {
                if is_write {
                    if buffer.push(i as u32).is_ok() {
                        total_writes += 1;
                    }
                } else {
                    if buffer.pop().is_some() {
                        total_reads += 1;
                    }
                }
                prop_assert_eq!(
                    buffer.len(),
                    total_writes.saturating_sub(total_reads),
                    "len() invariant violated"
                );
            }
        }
    }

    proptest! {
        #[test]
        fn push_pop_roundtrip(vals in proptest::collection::vec(
            proptest::num::u64::ANY, 0..100
        )) {
            const N: usize = 256;
            let buffer: RingBuffer<u64, N> = RingBuffer::new();

            for &v in &vals {
                let _ = buffer.push(v);
            }
            let mut collected = Vec::new();
            while let Some(v) = buffer.pop() {
                collected.push(v);
            }
            let cap = buffer.capacity();
            let capped_len = vals.len().min(cap);
            prop_assert_eq!(collected.len(), capped_len);
            let expected: Vec<u64> = vals.into_iter().take(cap).collect();
            prop_assert_eq!(collected, expected);
        }
    }

    #[test]
    fn capacity_boundary() {
        // Can't use runtime N with const generics, so test specific power-of-2 sizes
        let check = |b: RingBuffer<u8, 2>| assert_eq!(b.capacity(), 1);
        check(RingBuffer::new());

        let check = |b: RingBuffer<u8, 4>| assert_eq!(b.capacity(), 3);
        check(RingBuffer::new());

        let check = |b: RingBuffer<u8, 16>| assert_eq!(b.capacity(), 15);
        check(RingBuffer::new());

        let check = |b: RingBuffer<u8, 256>| assert_eq!(b.capacity(), 255);
        check(RingBuffer::new());

        let check = |b: RingBuffer<u8, 1024>| assert_eq!(b.capacity(), 1023);
        check(RingBuffer::new());

        let check = |b: RingBuffer<u8, 4096>| assert_eq!(b.capacity(), 4095);
        check(RingBuffer::new());

        let check = |b: RingBuffer<u8, 65536>| assert_eq!(b.capacity(), 65535);
        check(RingBuffer::new());
    }
}

// ---------------------------------------------------------------------------
// Module 2: Wallet Guard Properties
// ---------------------------------------------------------------------------

mod wallet_guard_properties {
    use super::*;

    fn make_order(symbol: u32, quantity: u64, price: u64, side: OrderSide) -> Order {
        Order::new(symbol, side, quantity, price)
    }

    #[test]
    fn monotonic_rejection() {
        let guard = WalletGuard::new(RiskParams {
            pi_max: i64::MAX,
            sigma_max: 1000,
            lambda_max: i64::MAX,
            margin_ratio: 1,
        });
        let wallet = Wallet::new(u64::MAX);

        for qty in 1..=2000u64 {
            let order = make_order(1, qty, 1, OrderSide::Buy);
            let result = guard.check(&wallet, &order);
            if let RiskDecision::Reject(RejectReason::OrderSizeExceeded { .. }) = result {
                let larger_order = make_order(1, qty + 1, 1, OrderSide::Buy);
                assert!(
                    matches!(
                        guard.check(&wallet, &larger_order),
                        RiskDecision::Reject(RejectReason::OrderSizeExceeded { .. })
                    ),
                    "if order qty {} rejected, {} must also be rejected",
                    qty,
                    qty + 1
                );
            }
        }
    }

    proptest! {
        #[test]
        fn no_panic_on_extreme_values(
            quantity in proptest::num::u64::ANY,
            price in proptest::num::u64::ANY,
        ) {
            let guard = WalletGuard::new(RiskParams {
                pi_max: i64::MAX,
                sigma_max: u64::MAX,
                lambda_max: i64::MAX,
                margin_ratio: 1,
            });
            let wallet = Wallet::new(u64::MAX);
            let order = Order::new(1, OrderSide::Buy, quantity, price);
            let _ = guard.check(&wallet, &order);
        }
    }

    #[test]
    fn valid_order_always_passes() {
        let guard = WalletGuard::new(RiskParams {
            pi_max: 10_000,
            sigma_max: 100_000,
            lambda_max: i64::MAX,
            margin_ratio: 1,
        });
        let wallet = Wallet::new(u64::MAX);

        for qty in 1..=100u64 {
            for px in 1..=100u64 {
                let order = make_order(1, qty, px, OrderSide::Buy);
                if qty <= 100_000 {
                    assert!(
                        matches!(guard.check(&wallet, &order), RiskDecision::Approve),
                        "small order qty={qty} px={px} should pass"
                    );
                }
            }
        }
    }

    #[test]
    fn position_limit_enforced() {
        let guard = WalletGuard::new(RiskParams {
            pi_max: 100,
            sigma_max: u64::MAX,
            lambda_max: i64::MAX,
            margin_ratio: 1,
        });
        let mut wallet = Wallet::new(u64::MAX);
        wallet.positions.insert(1, 100);

        let ok_order = make_order(1, 1, 1, OrderSide::Sell);
        assert!(matches!(
            guard.check(&wallet, &ok_order),
            RiskDecision::Approve
        ));

        let over_order = make_order(1, 1, 1, OrderSide::Buy);
        assert!(
            matches!(
                guard.check(&wallet, &over_order),
                RiskDecision::Reject(RejectReason::PositionLimitExceeded { .. })
            ),
            "order exceeding pi_max must be rejected"
        );
    }

    #[test]
    fn insufficient_margin_denied() {
        let guard = WalletGuard::new(RiskParams {
            pi_max: i64::MAX,
            sigma_max: u64::MAX,
            lambda_max: i64::MAX,
            margin_ratio: 4,
        });
        let wallet = Wallet::new(100);

        let ok_order = make_order(1, 1_000_000, 100, OrderSide::Sell);
        assert!(matches!(
            guard.check(&wallet, &ok_order),
            RiskDecision::Approve
        ));

        let over_order = make_order(1, 1_000, 100, OrderSide::Buy);
        assert!(
            matches!(
                guard.check(&wallet, &over_order),
                RiskDecision::Reject(RejectReason::InsufficientMargin { .. })
            ),
            "buy order exceeding cash margin must be rejected"
        );
    }

    proptest! {
        #[test]
        fn order_value_rejection_boundary(
            qty in 1u64..=10000u64,
            px in 1u64..=10000u64,
        ) {
            let guard = WalletGuard::new(RiskParams {
                pi_max: i64::MAX,
                sigma_max: u64::MAX,
                lambda_max: i64::MAX,
                margin_ratio: 1,
            });
            let cash = 50_000u64;
            let wallet = Wallet::new(cash);
            let order = Order::new(1, OrderSide::Buy, qty, px);
            let notional = qty.saturating_mul(px);

            if notional > cash {
                let result = guard.check(&wallet, &order);
                prop_assert!(
                    matches!(result, RiskDecision::Reject(RejectReason::InsufficientMargin { .. })),
                    "expected InsufficientMargin for qty={} px={} notional={}",
                    qty, px, notional
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Module 3: Capability Properties
// ---------------------------------------------------------------------------

mod capability_properties {
    use super::*;

    proptest! {
        #[test]
        fn derive_is_subset(
            parent_perms in permission_set_strategy(),
            subset_perms in permission_set_strategy(),
        ) {
            let parent_perms_clone = parent_perms.clone();
            let parent = CapabilityToken::new(ResourceScope::default(), parent_perms);
            let subset: HashSet<_> = subset_perms.intersection(&parent_perms_clone).cloned().collect();

            let child = parent.derive(subset.clone());
            match child {
                Some(token) => {
                    prop_assert!(token.verify(), "derived token must verify");
                    prop_assert_eq!(token.permissions(), &subset);
                }
                None if !subset.is_empty() => {
                    prop_assert!(!subset_perms.is_subset(&parent_perms_clone));
                }
                None => {}
            }
        }
    }

    proptest! {
        #[test]
        fn escalation_blocked(
            parent_perms in permission_set_strategy(),
            extra in permission_set_strategy(),
        ) {
            let parent = CapabilityToken::new(ResourceScope::default(), parent_perms.clone());
            let escalated: HashSet<_> = parent_perms.union(&extra).cloned().collect();

            if escalated.len() > parent_perms.len() {
                prop_assert!(
                    parent.derive(escalated).is_none(),
                    "derive with superset must return None"
                );
            }
        }
    }

    proptest! {
        #[test]
        fn fresh_token_verifies(permissions in permission_set_strategy()) {
            let token = CapabilityToken::new(ResourceScope::default(), permissions);
            prop_assert!(token.verify(), "freshly created token must verify");
        }
    }

    proptest! {
        #[test]
        fn transitive_attenuation(
            full_perms in permission_set_strategy(),
            s2 in permission_set_strategy(),
            s1 in permission_set_strategy(),
        ) {
            let parent = CapabilityToken::new(ResourceScope::default(), full_perms.clone());
            let s2_sub: HashSet<_> = s2.intersection(&full_perms).cloned().collect();

            if let Some(child) = parent.derive(s2_sub.clone()) {
                let s1_sub: HashSet<_> = s1.intersection(&s2_sub).cloned().collect();
                if let Some(grandchild) = child.derive(s1_sub.clone()) {
                    prop_assert!(grandchild.verify());
                    prop_assert_eq!(grandchild.permissions(), &s1_sub);
                }
            }
        }
    }

    #[test]
    fn empty_capability_denies_all() {
        let token = CapabilityToken::new(ResourceScope::default(), HashSet::new());
        for perm in all_permissions() {
            assert!(
                !token.has_permission(perm),
                "empty capability must deny {:?}",
                perm
            );
        }
    }

    #[test]
    fn derive_empty_always_succeeds() {
        let perms: HashSet<_> = all_permissions().into_iter().collect();
        let token = CapabilityToken::new(ResourceScope::default(), perms);
        assert!(token.derive(HashSet::new()).is_some());
        let derived = token.derive(HashSet::new()).unwrap();
        assert!(derived.verify());
        assert!(derived.permissions().is_empty());
    }

    #[test]
    fn identity_derive_preserves_permissions() {
        let perms: HashSet<_> = all_permissions().into_iter().collect();
        let token = CapabilityToken::new(ResourceScope::default(), perms.clone());
        let derived = token.derive(perms.clone()).unwrap();
        assert!(derived.verify());
        assert_eq!(derived.permissions(), &perms);
    }

    #[test]
    fn with_expiry_sets_expired() {
        let token = CapabilityToken::new(ResourceScope::default(), HashSet::new())
            .with_expiry(std::time::Duration::from_millis(1));
        assert!(!token.is_expired());
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(token.is_expired());
    }

    #[test]
    fn expired_token_fails_verify() {
        let token = CapabilityToken::new(ResourceScope::default(), HashSet::new())
            .with_expiry(std::time::Duration::from_millis(1));
        assert!(token.verify());
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(!token.verify());
    }

    #[test]
    fn resource_accessor_returns_scope() {
        let scope = ResourceScope {
            paths: vec!["/tmp".to_string()],
            hosts: vec!["localhost".to_string()],
            env_vars: vec!["PATH".to_string()],
        };
        let token = CapabilityToken::new(scope.clone(), HashSet::new());
        assert_eq!(token.resource().paths, scope.paths);
        assert_eq!(token.resource().hosts, scope.hosts);
        assert_eq!(token.resource().env_vars, scope.env_vars);
    }

    #[test]
    fn id_is_unique() {
        let t1 = CapabilityToken::new(ResourceScope::default(), HashSet::new());
        let t2 = CapabilityToken::new(ResourceScope::default(), HashSet::new());
        assert_ne!(t1.id(), t2.id());
    }
}

// ---------------------------------------------------------------------------
// Module 4: FSM (Nexus) Properties
// ---------------------------------------------------------------------------

mod fsm_properties {
    use super::*;

    #[test]
    fn phase_count_is_24() {
        assert_eq!(all_phases().len(), 24, "there must be exactly 24 phases");
    }

    #[test]
    fn next_always_increases_index() {
        for phase in all_phases() {
            let id = phase.phase_id();
            if let Some(next_id) = id.next() {
                assert_eq!(
                    next_id.0,
                    id.0 + 1,
                    "next({}) should have index {}",
                    id.0,
                    id.0 + 1
                );
            }
        }
    }

    #[test]
    fn terminal_phase_has_no_next() {
        let terminal = PhaseId(23);
        assert!(terminal.is_terminal());
        assert!(terminal.next().is_none(), "phase 23 must have no next");

        let terminal_phase = get_phase_by_id(PhaseId(23));
        assert!(terminal_phase.phase_id().is_terminal());
    }

    #[test]
    fn all_phases_reachable() {
        let mut reached = HashSet::new();
        let mut current = Some(PhaseId(0));

        while let Some(id) = current {
            assert!(
                reached.insert(id),
                "phase {} visited twice — self-loop detected",
                id.0
            );
            let _ = get_phase_by_id(id);
            current = id.next();
        }

        for i in 0..24u8 {
            assert!(
                reached.contains(&PhaseId(i)),
                "phase {} not reachable from start",
                i
            );
        }
        assert_eq!(reached.len(), 24);
    }

    #[test]
    fn no_self_loops() {
        for i in 0..24u8 {
            let id = PhaseId(i);
            if let Some(next) = id.next() {
                assert_ne!(id, next, "phase {} transitions to itself", i);
            }
        }
    }

    proptest! {
        #[test]
        fn artifact_hash_deterministic(content in proptest::string::string_regex("[a-zA-Z0-9 ]{0,500}").unwrap()) {
            let json_val = serde_json::json!({"data": content});
            let hash1 = Artifact::compute_hash(&json_val);
            let hash2 = Artifact::compute_hash(&json_val);
            prop_assert_eq!(hash1, hash2, "same content must produce same hash");
        }
    }

    #[test]
    fn artifact_integrity_holds() {
        for artifact_type in ArtifactType::all() {
            for phase_num in 0..24u8 {
                let artifact = Artifact::new(
                    artifact_type.clone(),
                    serde_json::json!({"test": phase_num}),
                    PhaseId(phase_num),
                );
                assert!(
                    artifact.verify_integrity(),
                    "fresh artifact type={:?} phase={} must have valid integrity",
                    artifact_type,
                    phase_num
                );
            }
        }
    }

    proptest! {
        #[test]
        fn phase_category_assignment(index in 0u8..24u8) {
            let id = PhaseId::new(index).unwrap();
            let phase = get_phase_by_id(id);
            let from_macro = PhaseCategory::from_phase_number(index);
            prop_assert_eq!(phase.category(), from_macro);
        }
    }

    #[test]
    fn phase_id_roundtrip() {
        for i in 0..24u8 {
            let id = PhaseId::new(i);
            assert!(id.is_ok(), "PhaseId::new({i}) should succeed");
            assert_eq!(id.unwrap().0, i);
        }
        assert!(PhaseId::new(24).is_err());
        assert!(PhaseId::new(255).is_err());
    }

    #[test]
    fn phase_numbers_contiguous() {
        let phases = all_phases();
        for (i, phase) in phases.iter().enumerate() {
            assert_eq!(
                phase.phase_number(),
                i as u8,
                "phase at position {} should have number {}",
                i,
                i
            );
        }
    }

    proptest! {
        #[test]
        fn phase_id_display_parsable(index in 0u8..24u8) {
            let id = PhaseId::new(index).unwrap();
            let display = format!("{}", id);
            prop_assert!(display.starts_with("Phase"));
            prop_assert!(display.contains(&index.to_string()));
        }
    }
}

// ---------------------------------------------------------------------------
// Module 5: Execution Properties
// ---------------------------------------------------------------------------

mod execution_properties {
    use super::*;

    fn order_side_strategy() -> impl Strategy<Value = OrderSide> {
        prop_oneof![Just(OrderSide::Buy), Just(OrderSide::Sell)]
    }

    proptest! {
        #[test]
        fn fill_roundtrip(
            symbol in 0u32..=10000u32,
            side in order_side_strategy(),
            quantity in 1u64..=1_000_000u64,
            price in 1u64..=1_000_000u64,
        ) {
            let exec = SimulatedExecution::new();
            let order = Order::new(symbol, side, quantity, price);
            let result = exec.submit_order(&order);
            prop_assert!(result.is_ok(), "valid order should not be rejected");
            let fill = result.unwrap();
            prop_assert!(fill.quantity <= order.quantity, "fill quantity must not exceed order quantity");
            prop_assert!(fill.price > 0, "fill price must be positive");
            prop_assert_eq!(fill.symbol, order.symbol);
            prop_assert_eq!(fill.side, order.side);
        }
    }

    proptest! {
        #[test]
        fn no_partial_fills(
            symbol in 0u32..=10000u32,
            side in order_side_strategy(),
            quantity in 1u64..=1_000_000u64,
            price in 1u64..=1_000_000u64,
        ) {
            let exec = SimulatedExecution::new();
            let order = Order::new(symbol, side, quantity, price);
            let fill = exec.submit_order(&order).unwrap();
            prop_assert_eq!(fill.quantity, order.quantity, "SimulatedExecution should fill completely");
        }
    }

    proptest! {
        #[test]
        fn fill_has_valid_timestamp(
            symbol in 0u32..=10000u32,
            side in order_side_strategy(),
            quantity in 1u64..=1_000_000u64,
            price in 1u64..=1_000_000u64,
        ) {
            let exec = SimulatedExecution::new();
            let order = Order::new(symbol, side, quantity, price);
            let fill = exec.submit_order(&order).unwrap();
            prop_assert!(fill.timestamp >= 0, "fill timestamp must be non-negative");
        }
    }
}

// ---------------------------------------------------------------------------
// Module 6: Feed Properties
// ---------------------------------------------------------------------------

mod feed_properties {
    use super::*;

    proptest! {
        #[test]
        fn quote_validity(
            num_symbols in 1usize..=10usize,
            base_price in 1.0f64..10000.0f64,
        ) {
            let symbols: Vec<String> = (0..num_symbols)
                .map(|i| format!("SYM{}", i))
                .collect();
            let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
            let price_pairs: Vec<(&str, f64)> = symbol_refs.iter().map(|s| (*s, base_price)).collect();

            let feed = SimulatedFeed::with_symbols(&symbol_refs, &price_pairs);

            for _ in 0..20 {
                let update = feed.generate_quote();
                let quote = match update {
                    MarketUpdate::Quote(q) => Some(q),
                    MarketUpdate::Heartbeat => None,
                };
                if let Some(q) = quote {
                    prop_assert!(q.bid > 0.0, "quote bid must be positive");
                    prop_assert!(q.ask > 0.0, "quote ask must be positive");
                }
            }
        }
    }

    proptest! {
        #[test]
        fn symbol_consistency(
            num_symbols in 1usize..=10usize,
            base_price in 1.0f64..10000.0f64,
        ) {
            let symbols: Vec<String> = (0..num_symbols)
                .map(|i| format!("SYM{}", i))
                .collect();
            let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
            let price_pairs: Vec<(&str, f64)> = symbol_refs.iter().map(|s| (*s, base_price)).collect();
            let registered_names: HashSet<String> = symbols.iter().cloned().collect();

            let feed = SimulatedFeed::with_symbols(&symbol_refs, &price_pairs);

            for _ in 0..20 {
                let update = feed.generate_quote();
                let quote = match update {
                    MarketUpdate::Quote(q) => Some(q),
                    MarketUpdate::Heartbeat => None,
                };
                if let Some(q) = quote {
                    prop_assert!(registered_names.contains(&q.symbol.0),
                        "quote symbol {:?} not in registered symbols", q.symbol.0);
                }
            }
        }
    }

    proptest! {
        #[test]
        fn bid_ask_spread(
            num_symbols in 1usize..=10usize,
            base_price in 1.0f64..10000.0f64,
        ) {
            let symbols: Vec<String> = (0..num_symbols)
                .map(|i| format!("SYM{}", i))
                .collect();
            let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
            let price_pairs: Vec<(&str, f64)> = symbol_refs.iter().map(|s| (*s, base_price)).collect();

            let feed = SimulatedFeed::with_symbols(&symbol_refs, &price_pairs);

            for _ in 0..20 {
                let update = feed.generate_quote();
                let quote = match update {
                    MarketUpdate::Quote(q) => Some(q),
                    MarketUpdate::Heartbeat => None,
                };
                if let Some(q) = quote {
                    prop_assert!(q.ask >= q.bid, "ask ({}) must be >= bid ({})", q.ask, q.bid);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Module 7: Persistence Properties
// ---------------------------------------------------------------------------

mod persistence_properties {
    use super::*;

    proptest! {
        #[test]
        fn checkpoint_roundtrip(
            phase in 0u8..24u8,
            event_count in 0u64..1000u64,
        ) {
            let persistence = StatePersistence::in_memory();
            let session_id = SessionId::new("proptest-session");
            let state = FsmState::new(session_id.clone(), PhaseId(0));
            persistence.create_session(&state).unwrap();

            let artifact_ids: Vec<String> = (0..5).map(|i| format!("artifact-{}", i)).collect();
            let checkpoint = Checkpoint::new(session_id.clone(), PhaseId(phase))
                .with_artifacts(artifact_ids.clone())
                .with_event_count(event_count);
            persistence.create_checkpoint(&checkpoint).unwrap();

            let loaded = persistence.load_checkpoint(&checkpoint.checkpoint_id).unwrap();
            prop_assert!(loaded.is_some(), "checkpoint should be retrievable");
            let loaded = loaded.unwrap();
            prop_assert_eq!(loaded.session_id.0, session_id.0);
            prop_assert_eq!(loaded.phase, PhaseId(phase));
            prop_assert_eq!(loaded.artifact_ids, artifact_ids);
            prop_assert_eq!(loaded.event_count, event_count);
        }
    }

    proptest! {
        #[test]
        fn session_uniqueness(
            phase1 in 0u8..24u8,
            phase2 in 0u8..24u8,
        ) {
            let persistence = StatePersistence::in_memory();
            let session1 = SessionId::new("session-alpha");
            let session2 = SessionId::new("session-beta");

            let state1 = FsmState::new(session1.clone(), PhaseId(phase1));
            let state2 = FsmState::new(session2.clone(), PhaseId(phase2));
            persistence.create_session(&state1).unwrap();
            persistence.create_session(&state2).unwrap();

            let mut loaded1 = persistence.load_session(&session1).unwrap().unwrap();
            loaded1.current_phase = PhaseId(23);
            loaded1.touch();
            persistence.update_session(&loaded1).unwrap();

            let loaded2 = persistence.load_session(&session2).unwrap().unwrap();
            prop_assert_eq!(loaded2.current_phase, PhaseId(phase2),
                "session2 should be unaffected by session1 update");
            prop_assert_eq!(loaded2.status, SessionStatus::Active);
        }
    }

    proptest! {
        #[test]
        fn event_ordering(
            num_events in 1usize..=20usize,
        ) {
            let persistence = StatePersistence::in_memory();
            let session_id = SessionId::new("ordering-session");
            let state = FsmState::new(session_id.clone(), PhaseId(0));
            persistence.create_session(&state).unwrap();

            for i in 0..num_events {
                let event = RecoveryEvent::new(
                    session_id.clone(),
                    format!("event-{}", i),
                    serde_json::json!({"seq": i}),
                );
                persistence.log_recovery_event(&event).unwrap();
            }

            let retrieved = persistence.get_recovery_events(&session_id, 100).unwrap();
            prop_assert_eq!(retrieved.len(), num_events, "all events should be retrievable");

            let retrieved_seqs: HashSet<u64> = retrieved
                .iter()
                .filter_map(|e| e.event_data.get("seq").and_then(|v| v.as_u64()))
                .collect();
            for i in 0..num_events {
                prop_assert!(retrieved_seqs.contains(&(i as u64)),
                    "event with seq {} should be present", i);
            }

            for window in retrieved.windows(2) {
                prop_assert!(window[0].timestamp >= window[1].timestamp,
                    "timestamps should be in descending order");
            }
        }
    }
}
