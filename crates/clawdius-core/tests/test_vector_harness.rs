use std::collections::HashMap;

use clawdius_core::broker::{
    Order, OrderSide, RejectReason, RingBuffer, RiskDecision, RiskParams, Wallet, WalletGuard,
};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
struct TypedValue {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    type_name: String,
    value: toml::Value,
}

#[derive(Deserialize)]
struct TestVector {
    id: String,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    category: String,
    input: HashMap<String, TypedValue>,
    expected_output: HashMap<String, TypedValue>,
}

#[derive(Deserialize)]
struct TestVectorFile {
    test_vector: Vec<TestVector>,
}

impl TypedValue {
    fn as_i64(&self) -> i64 {
        self.value.as_integer().unwrap_or(0)
    }
    fn as_u64(&self) -> u64 {
        self.value.as_integer().unwrap_or(0) as u64
    }
    fn as_u32(&self) -> u32 {
        self.value.as_integer().unwrap_or(0) as u32
    }
    fn as_usize(&self) -> usize {
        self.value.as_integer().unwrap_or(0) as usize
    }
    fn as_str(&self) -> &str {
        self.value.as_str().unwrap_or("")
    }
    fn as_bool(&self) -> bool {
        self.value.as_bool().unwrap_or(false)
    }
    fn as_str_list(&self) -> Vec<String> {
        self.value
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn tv<'a>(input: &'a HashMap<String, TypedValue>, key: &str) -> &'a TypedValue {
    input
        .get(key)
        .unwrap_or_else(|| panic!("missing key: {key}"))
}

fn tv_expect<'a>(output: &'a HashMap<String, TypedValue>, key: &str) -> &'a TypedValue {
    output
        .get(key)
        .unwrap_or_else(|| panic!("missing expected key: {key}"))
}

// ---------------------------------------------------------------------------
// HFT / Wallet Guard Tests
// ---------------------------------------------------------------------------

mod hft {
    use super::*;

    fn load_vectors() -> Vec<TestVector> {
        let raw = include_str!("../../../.specs/01_research/test_vectors/test_vectors_hft.toml");
        // u64::MAX (18446744073709551615) overflows TOML's i64 integer type.
        // Replace with i64::MAX so the file parses. TV-HFT-003's cash field is not
        // used by the real WalletGuard::check_order API, so this is semantically safe.
        let sanitized = raw.replace("18446744073709551615", "9223372036854775807");
        let file: TestVectorFile =
            toml::from_str(&sanitized).expect("failed to parse HFT test vectors");
        file.test_vector
    }

    fn find(id: &str) -> TestVector {
        load_vectors().into_iter().find(|v| v.id == id).unwrap()
    }

    fn make_order(input: &HashMap<String, TypedValue>) -> Order {
        let symbol = tv(input, "symbol").as_u32();
        let side = match tv(input, "side").as_str() {
            "buy" => OrderSide::Buy,
            "sell" => OrderSide::Sell,
            other => panic!("unknown side: {other}"),
        };
        Order::new(
            symbol,
            side,
            tv(input, "quantity").as_u64(),
            tv(input, "price").as_u64(),
        )
    }

    /// TV-HFT-001: Standard buy order with value equal to max_order_value is approved
    /// (guard rejects only when value > max, not >=).
    #[test]
    fn tv_hft_001_valid_buy_within_limits() {
        let vec = find("TV-HFT-001");
        let guard = WalletGuard::with_defaults();
        let wallet = Wallet::new(1_000_000);
        let order = make_order(&vec.input);
        assert_eq!(
            guard.check(&wallet, &order),
            RiskDecision::Approve,
            "TV-HFT-001: order should be approved within limits"
        );
    }

    /// TV-HFT-002: Order that would push position beyond pi_max is rejected.
    #[test]
    fn tv_hft_002_position_limit_exceeded() {
        let guard = WalletGuard::with_defaults();
        let mut wallet = Wallet::new(1_000_000);
        wallet.positions.insert(1, 10_000);
        let order = Order::new(1, OrderSide::Buy, 1, 100);
        assert!(
            matches!(
                guard.check(&wallet, &order),
                RiskDecision::Reject(RejectReason::PositionLimitExceeded { .. })
            ),
            "TV-HFT-002: order exceeding position limit should be rejected"
        );
    }

    /// TV-HFT-003: Order with very large values produces overflow in position or margin check,
    /// triggering rejection without panic. The TOML vector uses i64::MAX price which causes
    /// InsufficientMargin (not PositionOverflow). We test both paths: PositionOverflow via
    /// wallet position near i64::MAX, and InsufficientMargin via large price.
    #[test]
    fn tv_hft_003_overflow_protection() {
        // Path 1: PositionOverflow — wallet at i64::MAX-5, buy 10 more
        let guard = WalletGuard::new(RiskParams {
            pi_max: i64::MAX,
            sigma_max: u64::MAX,
            lambda_max: i64::MAX,
            margin_ratio: 1,
        });
        let mut wallet = Wallet::new(u64::MAX);
        wallet.positions.insert(1, i64::MAX - 5);
        let order = Order::new(1, OrderSide::Buy, 10, 1);
        let result = guard.check(&wallet, &order);
        assert!(
            matches!(result, RiskDecision::Reject(RejectReason::PositionOverflow)),
            "TV-HFT-003: position arithmetic overflow must reject with PositionOverflow"
        );

        // Path 2: InsufficientMargin — i64::MAX price with insufficient cash
        let guard = WalletGuard::with_defaults();
        let wallet = Wallet::new(1_000_000);
        let order = Order::new(2, OrderSide::Buy, 1, i64::MAX as u64);
        let result = guard.check(&wallet, &order);
        assert!(
            matches!(
                result,
                RiskDecision::Reject(RejectReason::InsufficientMargin { .. })
            ),
            "TV-HFT-003: extreme price must reject with InsufficientMargin"
        );
    }

    // TV-HFT-004: SKIPPED — Order.price is u64, so negative prices are impossible to represent.
    // The unified API rejects zero-price orders (RejectReason::ZeroPrice) but a u64 field
    // cannot hold negative values, making this test vector inapplicable.

    /// TV-HFT-005: Sell order approved when value is within elevated limits.
    /// Uses custom RiskParams with large pi_max and sigma_max.
    #[test]
    fn tv_hft_005_sell_within_limits() {
        let vec = find("TV-HFT-005");
        let guard = WalletGuard::new(RiskParams {
            pi_max: 10_000_000,
            sigma_max: 10_000_000,
            lambda_max: 10_000_000,
            margin_ratio: 1,
        });
        let wallet = Wallet::new(10_000_000);
        let order = make_order(&vec.input);
        assert_eq!(
            guard.check(&wallet, &order),
            RiskDecision::Approve,
            "TV-HFT-005: sell order should be approved with elevated limits"
        );
    }

    /// TV-HFT-006: Short sell approved with default guard and wallet.
    #[test]
    fn tv_hft_006_short_sell_approved() {
        let vec = find("TV-HFT-006");
        let guard = WalletGuard::with_defaults();
        let wallet = Wallet::new(1_000_000);
        let order = make_order(&vec.input);
        assert_eq!(
            guard.check(&wallet, &order),
            RiskDecision::Approve,
            "TV-HFT-006: short sell should be approved"
        );
    }

    /// TV-HFT-007: Zero-quantity orders are rejected by the unified API.
    #[test]
    fn tv_hft_007_zero_quantity_rejected() {
        let guard = WalletGuard::with_defaults();
        let wallet = Wallet::new(1_000_000);
        let order = Order::new(1, OrderSide::Buy, 0, 100);
        assert_eq!(
            guard.check(&wallet, &order),
            RiskDecision::Reject(RejectReason::ZeroQuantity),
            "TV-HFT-007: zero-quantity order should be rejected"
        );
    }

    /// TV-HFT-008: Daily drawdown exceeding lambda_max triggers rejection.
    #[test]
    fn tv_hft_008_daily_drawdown_rejected() {
        let guard = WalletGuard::with_defaults();
        let mut wallet = Wallet::new(1_000_000);
        wallet.session_start_pnl = 0;
        wallet.realized_pnl = -20_000_000;
        let order = Order::new(1, OrderSide::Buy, 10, 100);
        assert!(
            matches!(
                guard.check(&wallet, &order),
                RiskDecision::Reject(RejectReason::DailyDrawdownExceeded { .. })
            ),
            "TV-HFT-008: order during drawdown should be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// Ring Buffer Tests
// ---------------------------------------------------------------------------

mod ring_buffer {
    use super::*;

    fn load_vectors() -> Vec<TestVector> {
        let raw =
            include_str!("../../../.specs/01_research/test_vectors/test_vectors_ring_buffer.toml");
        let file: TestVectorFile =
            toml::from_str(raw).expect("failed to parse ring buffer test vectors");
        file.test_vector
    }

    fn find(id: &str) -> TestVector {
        load_vectors().into_iter().find(|v| v.id == id).unwrap()
    }

    /// TV-RB-001: Single write-read cycle preserves all fields and empties buffer.
    /// Uses a tuple (symbol, price, quantity, timestamp) since real RingBuffer is generic.
    #[test]
    fn tv_rb_001_single_write_read() {
        let vec = find("TV-RB-001");
        let inp = &vec.input;
        let buf: RingBuffer<(u32, i64, u32, u64), 1024> = RingBuffer::new();
        let msg = (
            tv(inp, "write_symbol").as_u32(),
            tv(inp, "write_price").as_i64(),
            tv(inp, "write_quantity").as_u32(),
            tv(inp, "write_timestamp").as_u64(),
        );
        buf.push(msg).expect("write should succeed");
        let out = buf.pop().expect("read should succeed");
        let exp = &vec.expected_output;
        assert_eq!(out.0, tv_expect(exp, "read_symbol").as_u32());
        assert_eq!(out.1, tv_expect(exp, "read_price").as_i64());
        assert_eq!(out.2, tv_expect(exp, "read_quantity").as_u32());
        assert_eq!(out.3, tv_expect(exp, "read_timestamp").as_u64());
        assert_eq!(buf.len(), tv_expect(exp, "buffer_len_after").as_usize());
    }

    /// TV-RB-002: Filling buffer to capacity, then writing one more, returns error.
    /// RingBuffer<T, 16> has usable capacity 15; fill_count in TOML is 15 — compatible.
    #[test]
    fn tv_rb_002_buffer_full_rejection() {
        let buf: RingBuffer<u32, 16> = RingBuffer::new();
        let fill_count = buf.capacity();
        for i in 0..fill_count {
            buf.push(i as u32).unwrap();
        }
        assert!(buf.push(999).is_err(), "full buffer must reject write");
    }

    /// TV-RB-003: Reading from empty buffer returns None.
    #[test]
    fn tv_rb_003_buffer_empty_rejection() {
        let buf: RingBuffer<u64, 1024> = RingBuffer::new();
        assert!(buf.pop().is_none(), "empty buffer must return None");
        assert!(buf.is_empty());
    }

    /// TV-RB-004: Wraparound — fill capacity-1, pop 1, push 1, drain all, verify FIFO.
    /// RingBuffer<u32, 8> has capacity 7; TOML operations "write_7,read_1,write_1,read_7" match.
    #[test]
    fn tv_rb_004_wraparound_fifo() {
        const N: usize = 8;
        let buf: RingBuffer<u32, N> = RingBuffer::new();
        let cap = buf.capacity();
        for i in 0..cap {
            buf.push(i as u32).unwrap();
        }
        let first = buf.pop().unwrap();
        assert_eq!(first, 0);
        buf.push(100).unwrap();
        let mut all: Vec<u32> = vec![first];
        while let Some(v) = buf.pop() {
            all.push(v);
        }
        assert_eq!(all.len(), 8);
        let expected: Vec<u32> = (0..7u32).chain(std::iter::once(100)).collect();
        assert_eq!(all, expected, "wraparound must preserve FIFO order");
    }

    /// TV-RB-005: Non-power-of-2 capacity panics at construction.
    /// Real API uses assert!(N.is_power_of_two()) — tested via #[should_panic].
    #[test]
    #[should_panic(expected = "Capacity must be a power of 2")]
    fn tv_rb_005_non_power_of_two_rejected() {
        let _: RingBuffer<u64, 100> = RingBuffer::new();
    }

    /// TV-RB-006: Burst write 10000 messages, drain all, verify no loss and FIFO order.
    /// Uses N=16384 (capacity 16383) instead of TOML's 1048576 to avoid 8MB stack allocation.
    #[test]
    fn tv_rb_006_burst_write_drain() {
        const N: usize = 16384;
        let buf: RingBuffer<u64, N> = RingBuffer::new();
        let burst_count: u64 = 10000;
        for i in 0..burst_count {
            buf.push(i).unwrap();
        }
        let mut read_count = 0u64;
        while let Some(v) = buf.pop() {
            assert_eq!(v, read_count, "FIFO violated at index {read_count}");
            read_count += 1;
        }
        assert_eq!(read_count, burst_count, "all 10000 messages must be read");
    }
}

// ---------------------------------------------------------------------------
// Capability Tests
// ---------------------------------------------------------------------------

mod capability {
    use super::*;
    use clawdius_core::capability::{CapabilityToken, Permission, ResourceScope};
    use proptest::prelude::*;
    use std::collections::HashSet;
    use std::thread;
    use std::time::Duration;

    /// Parse a TOML permission string into a real Permission enum variant.
    /// Names MUST match the Rust Permission enum at capability.rs.
    fn parse_permission(s: &str) -> Option<Permission> {
        match s {
            "FsRead" => Some(Permission::FsRead),
            "FsWrite" => Some(Permission::FsWrite),
            "NetTcp" => Some(Permission::NetTcp),
            "NetUdp" => Some(Permission::NetUdp),
            "ExecSpawn" => Some(Permission::ExecSpawn),
            "SecretAccess" => Some(Permission::SecretAccess),
            "EnvRead" => Some(Permission::EnvRead),
            "EnvWrite" => Some(Permission::EnvWrite),
            _ => None,
        }
    }

    fn parse_permissions(list: &[String]) -> HashSet<Permission> {
        list.iter().filter_map(|s| parse_permission(s)).collect()
    }

    fn load_vectors() -> Vec<TestVector> {
        let raw =
            include_str!("../../../.specs/01_research/test_vectors/test_vectors_capability.toml");
        let file: TestVectorFile =
            toml::from_str(raw).expect("failed to parse capability test vectors");
        file.test_vector
    }

    fn find(id: &str) -> TestVector {
        load_vectors().into_iter().find(|v| v.id == id).unwrap()
    }

    fn default_resource() -> ResourceScope {
        ResourceScope {
            paths: vec!["/tmp/project".to_string()],
            hosts: vec![],
            env_vars: vec![],
        }
    }

    /// TV-CAP-001: Freshly created capability token must pass verification.
    /// Uses real CapabilityToken from clawdius_core::capability.
    #[test]
    fn tv_cap_001_fresh_token_verifies() {
        let vec = find("TV-CAP-001");
        let perms = parse_permissions(&tv(&vec.input, "permissions").as_str_list());
        let token = CapabilityToken::new(default_resource(), perms);
        assert!(token.verify(), "TV-CAP-001: fresh token must verify");
    }

    /// TV-CAP-002: Deriving with a strict subset of parent permissions succeeds.
    /// Tests real derive() method which creates a new token with attenuated permissions.
    #[test]
    fn tv_cap_002_attenuation_to_subset() {
        let vec = find("TV-CAP-002");
        let parent_perms = parse_permissions(&tv(&vec.input, "parent_permissions").as_str_list());
        let derive_perms = parse_permissions(&tv(&vec.input, "derive_permissions").as_str_list());
        let parent = CapabilityToken::new(default_resource(), parent_perms);
        let child = parent.derive(derive_perms.clone());
        assert!(child.is_some(), "TV-CAP-002: subset derive must succeed");
        let child = child.unwrap();
        assert!(child.verify(), "TV-CAP-002: derived token must verify");
        assert_eq!(child.permissions(), &derive_perms);
    }

    /// TV-CAP-003: Deriving with permissions not held by parent must return None.
    /// NetUdp is not in the parent set {FsRead}, so derive must fail.
    #[test]
    fn tv_cap_003_escalation_blocked() {
        let vec = find("TV-CAP-003");
        let parent_perms = parse_permissions(&tv(&vec.input, "parent_permissions").as_str_list());
        let derive_strs = tv(&vec.input, "derive_permissions").as_str_list();
        let derive_perms = parse_permissions(&derive_strs);
        let parent = CapabilityToken::new(default_resource(), parent_perms);
        let result = parent.derive(derive_perms);
        assert!(result.is_none(), "TV-CAP-003: escalation must be blocked");
    }

    /// TV-CAP-004: Transitive attenuation — derive(derive(parent, s1), s2) has only s2.
    /// Tests that attenuation is transitive: grandchild has exactly the second subset.
    #[test]
    fn tv_cap_004_transitive_attenuation() {
        let vec = find("TV-CAP-004");
        let parent_perms = parse_permissions(&tv(&vec.input, "parent_permissions").as_str_list());
        let first = parse_permissions(&tv(&vec.input, "first_derive").as_str_list());
        let second = parse_permissions(&tv(&vec.input, "second_derive").as_str_list());
        let parent = CapabilityToken::new(default_resource(), parent_perms);
        let child = parent
            .derive(first)
            .expect("TV-CAP-004: first derive should succeed");
        let grandchild = child
            .derive(second.clone())
            .expect("TV-CAP-004: second derive should succeed");
        assert!(grandchild.verify());
        let expected_final =
            parse_permissions(&tv_expect(&vec.expected_output, "final_permissions").as_str_list());
        assert_eq!(grandchild.permissions(), &expected_final);
    }

    /// TV-CAP-005: Token with empty permission set denies every permission check.
    /// Uses real has_permission() from clawdius_core::capability.
    #[test]
    fn tv_cap_005_empty_capability_denies_all() {
        let vec = find("TV-CAP-005");
        let perms = parse_permissions(&tv(&vec.input, "permissions").as_str_list());
        assert!(perms.is_empty(), "TV-CAP-005: permissions should be empty");
        let token = CapabilityToken::new(default_resource(), perms);
        for check in ["FsRead", "FsWrite", "ExecSpawn", "NetTcp"] {
            let perm = parse_permission(check).expect("known permission should parse");
            assert!(
                !token.has_permission(perm),
                "TV-CAP-005: empty token must deny {check}"
            );
        }
    }

    /// TV-CAP-006: Token created with zero-duration expiry is expired immediately.
    /// Tests real with_expiry() and is_expired() from clawdius_core::capability.
    /// Uses a short sleep to ensure the instant has definitely passed.
    #[test]
    fn tv_cap_006_expired_token_detection() {
        let vec = find("TV-CAP-006");
        let perms = parse_permissions(&tv(&vec.input, "permissions").as_str_list());
        let expiry_ms = tv(&vec.input, "expiry_duration_ms").as_u64();
        let token = CapabilityToken::new(default_resource(), perms)
            .with_expiry(Duration::from_millis(expiry_ms));
        // Sleep to ensure expiry has passed
        thread::sleep(Duration::from_millis(expiry_ms + 1));
        assert!(
            token.is_expired(),
            "TV-CAP-006: token with {}ms expiry must be expired after waiting",
            expiry_ms
        );
        assert!(
            !token.verify(),
            "TV-CAP-006: expired token must fail verification"
        );
    }

    /// TV-CAP-007: Token with 60-second expiry is NOT expired immediately.
    #[test]
    fn tv_cap_007_non_expired_token_within_window() {
        let vec = find("TV-CAP-007");
        let perms = parse_permissions(&tv(&vec.input, "permissions").as_str_list());
        let expiry_ms = tv(&vec.input, "expiry_duration_ms").as_u64();
        let token = CapabilityToken::new(default_resource(), perms)
            .with_expiry(Duration::from_millis(expiry_ms));
        assert!(
            !token.is_expired(),
            "TV-CAP-007: token with {}ms expiry must NOT be expired immediately",
            expiry_ms
        );
        assert!(token.verify(), "TV-CAP-007: non-expired token must verify");
    }

    /// TV-CAP-008: All 8 Rust Permission variants parse correctly from TOML strings.
    #[test]
    fn tv_cap_008_all_permission_variants_parse() {
        let vec = find("TV-CAP-008");
        let perm_strs = tv(&vec.input, "permissions").as_str_list();
        let parsed: HashSet<Permission> = parse_permissions(&perm_strs);
        assert_eq!(
            parsed.len(),
            tv_expect(&vec.expected_output, "permission_count").as_u64() as usize,
            "TV-CAP-008: must parse exactly 8 distinct permission variants"
        );
        // Verify every known variant was parsed
        for name in &[
            "FsRead",
            "FsWrite",
            "NetTcp",
            "NetUdp",
            "ExecSpawn",
            "SecretAccess",
            "EnvRead",
            "EnvWrite",
        ] {
            let perm = parse_permission(name).expect(&format!("TV-CAP-008: '{name}' must parse"));
            assert!(
                parsed.contains(&perm),
                "TV-CAP-008: parsed set must contain {name}"
            );
        }
        // Token with all 8 permissions must verify
        let token = CapabilityToken::new(default_resource(), parsed);
        assert!(
            token.verify(),
            "TV-CAP-008: full-permission token must verify"
        );
    }

    proptest! {
        /// Every known Rust Permission variant string parses to Some and round-trips
        /// through a CapabilityToken that verifies.
        #[test]
        fn proptest_permission_parsing_roundtrip(
            perm_name in proptest::sample::select(&[
                "FsRead", "FsWrite", "NetTcp", "NetUdp",
                "ExecSpawn", "SecretAccess", "EnvRead", "EnvWrite",
            ])
        ) {
            let parsed = parse_permission(perm_name);
            prop_assert!(parsed.is_some(), "known permission '{}' must parse", perm_name);
            let parsed = parsed.unwrap();
            let set = HashSet::from([parsed]);
            let token = CapabilityToken::new(default_resource(), set.clone());
            prop_assert!(token.verify());
            prop_assert!(token.has_permission(parsed));
        }

        /// Unknown permission strings (not matching any Rust variant) always map to None.
        #[test]
        fn proptest_unknown_permission_maps_to_none(
            name in "[a-zA-Z]{3,15}"
        ) {
            let known = [
                "FsRead", "FsWrite", "NetTcp", "NetUdp",
                "ExecSpawn", "SecretAccess", "EnvRead", "EnvWrite",
            ];
            if !known.contains(&name.as_str()) {
                prop_assert_eq!(parse_permission(&name), None);
            }
        }

        /// Fresh tokens always verify, regardless of permission set size.
        #[test]
        fn proptest_fresh_token_always_verifies(
            perm_count in 0usize..8
        ) {
            let all = [
                Permission::FsRead, Permission::FsWrite, Permission::NetTcp,
                Permission::NetUdp, Permission::ExecSpawn, Permission::SecretAccess,
                Permission::EnvRead, Permission::EnvWrite,
            ];
            let set: HashSet<Permission> = all[..perm_count].iter().copied().collect();
            let token = CapabilityToken::new(default_resource(), set);
            prop_assert!(token.verify());
        }
    }
}

// ---------------------------------------------------------------------------
// FSM / Nexus Phase Tests
// ---------------------------------------------------------------------------

mod fsm {
    use super::*;
    use clawdius_core::nexus::{all_phases, get_phase_by_id, NexusEngine, PhaseCategory, PhaseId};
    use tempfile::TempDir;

    fn load_vectors() -> Vec<TestVector> {
        let raw = include_str!("../../../.specs/01_research/test_vectors/test_vectors_fsm.toml");
        let file: TestVectorFile = toml::from_str(raw).expect("failed to parse FSM test vectors");
        file.test_vector
    }

    fn find(id: &str) -> TestVector {
        load_vectors().into_iter().find(|v| v.id == id).unwrap()
    }

    /// TV-FSM-001: PhaseId::new accepts 0 through 23
    #[test]
    fn tv_fsm_001_valid_phase_ids() {
        let vec = find("TV-FSM-001");
        let min = tv(&vec.input, "min").as_u32() as u8;
        let max = tv(&vec.input, "max").as_u32() as u8;
        let exp = &vec.expected_output;
        let count = tv_expect(exp, "count").as_u64() as usize;

        let mut valid_count = 0usize;
        for n in min..=max {
            assert!(
                PhaseId::new(n).is_ok(),
                "TV-FSM-001: PhaseId({n}) must be valid"
            );
            valid_count += 1;
        }
        assert!(tv_expect(exp, "valid").as_bool());
        assert_eq!(valid_count, count);
    }

    /// TV-FSM-002: PhaseId::new rejects 24 and 255
    #[test]
    fn tv_fsm_002_invalid_phase_ids() {
        let vec = find("TV-FSM-002");
        let invalid = tv(&vec.input, "invalid_id").as_u32() as u8;
        let another = tv(&vec.input, "another_invalid").as_u32() as u8;

        assert!(
            PhaseId::new(invalid).is_err(),
            "TV-FSM-002: PhaseId({invalid}) must be invalid"
        );
        assert!(
            PhaseId::new(another).is_err(),
            "TV-FSM-002: PhaseId({another}) must be invalid"
        );
        assert!(!tv_expect(&vec.expected_output, "valid").as_bool());
    }

    /// TV-FSM-003: Phase 0 is Context Discovery in Discovery category
    #[test]
    fn tv_fsm_003_phase_0_is_start() {
        let vec = find("TV-FSM-003");
        let n = tv(&vec.input, "phase_number").as_u32() as u8;
        let exp = &vec.expected_output;
        let phase = get_phase_by_id(PhaseId(n));
        assert_eq!(phase.phase_name(), tv_expect(exp, "name").as_str());
        let cat_str = tv_expect(exp, "category").as_str();
        let cat = match cat_str {
            "Discovery" => PhaseCategory::Discovery,
            "Requirements" => PhaseCategory::Requirements,
            "Architecture" => PhaseCategory::Architecture,
            "Planning" => PhaseCategory::Planning,
            "Implementation" => PhaseCategory::Implementation,
            "Verification" => PhaseCategory::Verification,
            "Validation" => PhaseCategory::Validation,
            "Transition" => PhaseCategory::Transition,
            _ => panic!("TV-FSM-003: unknown category '{cat_str}'"),
        };
        assert_eq!(phase.category(), cat);
    }

    /// TV-FSM-004: Phase 23 is terminal with no next
    #[test]
    fn tv_fsm_004_phase_23_is_terminal() {
        let vec = find("TV-FSM-004");
        let n = tv(&vec.input, "phase_number").as_u32() as u8;
        let exp = &vec.expected_output;
        let id = PhaseId(n);
        assert_eq!(id.is_terminal(), tv_expect(exp, "is_terminal").as_bool());
        assert_eq!(id.next().is_some(), tv_expect(exp, "has_next").as_bool());
    }

    /// TV-FSM-005: Phase 22 is not terminal and has next
    #[test]
    fn tv_fsm_005_phase_22_has_next() {
        let vec = find("TV-FSM-005");
        let n = tv(&vec.input, "phase_number").as_u32() as u8;
        let exp = &vec.expected_output;
        let id = PhaseId(n);
        assert_eq!(id.is_terminal(), tv_expect(exp, "is_terminal").as_bool());
        assert_eq!(
            id.next().is_some(),
            tv_expect(exp, "next_is_some").as_bool()
        );
        assert_eq!(id.next().unwrap(), PhaseId(23));
    }

    /// TV-FSM-006: all_phases() returns exactly 24
    #[test]
    fn tv_fsm_006_all_phases_count() {
        let vec = find("TV-FSM-006");
        let phases = all_phases();
        assert_eq!(
            phases.len(),
            tv_expect(&vec.expected_output, "count").as_u64() as usize,
            "TV-FSM-006: all_phases must return 24"
        );
    }

    /// TV-FSM-007: Each phase maps to correct category
    #[test]
    fn tv_fsm_007_phase_categories_correct() {
        let vec = find("TV-FSM-007");
        let test_data = tv(&vec.input, "test_phases").as_str_list();
        for entry in &test_data {
            let parts: Vec<&str> = entry.split(',').collect();
            assert_eq!(
                parts.len(),
                2,
                "TV-FSM-007: each entry must be 'num,Category'"
            );
            let n: u8 = parts[0].parse().unwrap();
            let expected_cat_str = parts[1];
            let expected_cat = match expected_cat_str {
                "Discovery" => PhaseCategory::Discovery,
                "Requirements" => PhaseCategory::Requirements,
                "Architecture" => PhaseCategory::Architecture,
                "Planning" => PhaseCategory::Planning,
                "Implementation" => PhaseCategory::Implementation,
                "Verification" => PhaseCategory::Verification,
                "Validation" => PhaseCategory::Validation,
                "Transition" => PhaseCategory::Transition,
                _ => panic!("TV-FSM-007: unknown category '{expected_cat_str}'"),
            };
            let actual = PhaseCategory::from_phase_number(n);
            assert_eq!(
                actual, expected_cat,
                "TV-FSM-007: phase {n} category mismatch"
            );
        }
        assert!(tv_expect(&vec.expected_output, "all_match").as_bool());
    }

    /// TV-FSM-008: PhaseId arithmetic for phase 5
    #[test]
    fn tv_fsm_008_phase_id_properties() {
        let vec = find("TV-FSM-008");
        let n = tv(&vec.input, "phase").as_u32() as u8;
        let exp = &vec.expected_output;
        let id = PhaseId(n);
        assert_eq!(id.next().unwrap().0, tv_expect(exp, "next").as_u32() as u8);
        assert_eq!(id.is_terminal(), tv_expect(exp, "is_terminal").as_bool());
    }

    /// TV-FSM-009: Phases are numbered contiguously 0-23
    #[test]
    fn tv_fsm_009_phase_number_sequence() {
        let vec = find("TV-FSM-009");
        let phases = all_phases();
        let exp = &vec.expected_output;
        let starts = tv_expect(exp, "starts_at").as_u64() as u8;
        let ends = tv_expect(exp, "ends_at").as_u64() as u8;

        for (i, phase) in phases.iter().enumerate() {
            assert_eq!(
                phase.phase_number(),
                starts + i as u8,
                "TV-FSM-009: phase at index {i} has wrong number"
            );
        }
        assert_eq!(phases.last().unwrap().phase_number(), ends);
        assert!(tv_expect(exp, "contiguous").as_bool());
    }

    /// TV-FSM-010: NexusEngine starts at phase 0
    #[test]
    fn tv_fsm_010_engine_creation() {
        let vec = find("TV-FSM-010");
        let exp = &vec.expected_output;
        let tmp = TempDir::new().expect("TV-FSM-010: temp dir creation failed");
        let engine =
            NexusEngine::new(tmp.path().to_path_buf()).expect("TV-FSM-010: engine creation failed");
        assert_eq!(
            engine.current_phase().0,
            tv_expect(exp, "current_phase").as_u32() as u8
        );
        assert_eq!(engine.phase_name(), tv_expect(exp, "phase_name").as_str());
    }
}
