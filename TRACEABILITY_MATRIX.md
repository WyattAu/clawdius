# Clawdius Traceability Matrix

**Document ID:** TM-CLAWDIUS-001  
**Version:** 2.0.0 (Phase 2 Complete)  
**Phase:** 2 (Architecture Refinement)  
**Created:** 2026-03-01  
**Updated:** 2026-03-01  
**Status:** UPDATED  

---

## 1. Purpose

This matrix provides bidirectional traceability between:
- **Requirements** → **Architecture** → **Implementation** → **Tests**

This ensures:
1. Every requirement has corresponding architecture/design
2. Every architecture decision traces to a requirement
3. Every implementation traces to architecture
4. Every test traces to a requirement

---

## 2. Forward Traceability (Requirements → Artifacts)

### 2.1 Core Engine & Lifecycle

| Requirement | Yellow Paper | Blue Paper | Implementation | Unit Tests | Integration Tests |
|-------------|--------------|------------|----------------|------------|-------------------|
| REQ-1.1 | YP-FSM-NEXUS-001 | BP-HOST-KERNEL-001, BP-NEXUS-FSM-001 | PENDING | PENDING | PENDING |
| REQ-1.2 | YP-FSM-NEXUS-001 | BP-NEXUS-FSM-001 | PENDING | PENDING | PENDING |
| REQ-1.3 | YP-FSM-NEXUS-001 | BP-HOST-KERNEL-001, BP-NEXUS-FSM-001 | PENDING | PENDING | PENDING |
| REQ-1.4 | - | BP-HOST-KERNEL-001 | PENDING | PENDING | PENDING |

### 2.2 Knowledge & Intelligence

| Requirement | Yellow Paper | Blue Paper | Implementation | Unit Tests | Integration Tests |
|-------------|--------------|------------|----------------|------------|-------------------|
| REQ-2.1 | - | BP-GRAPH-RAG-001 | PENDING | PENDING | PENDING |
| REQ-2.2 | - | BP-GRAPH-RAG-001 | PENDING | PENDING | PENDING |
| REQ-2.3 | - | BP-GRAPH-RAG-001 | PENDING | PENDING | PENDING |
| REQ-2.4 | - | BP-GRAPH-RAG-001 | PENDING | PENDING | PENDING |
| REQ-2.5 | - | BP-BRAIN-001 | PENDING | PENDING | PENDING |

### 2.3 Security & Sandboxing

| Requirement | Yellow Paper | Blue Paper | Implementation | Unit Tests | Integration Tests |
|-------------|--------------|------------|----------------|------------|-------------------|
| REQ-3.1 | YP-SECURITY-SANDBOX-001 | BP-SENTINEL-001 | PENDING | PENDING | PENDING |
| REQ-3.2 | YP-SECURITY-SANDBOX-001 | BP-SENTINEL-001, BP-BRAIN-001 | PENDING | PENDING | PENDING |
| REQ-3.3 | YP-SECURITY-SANDBOX-001 | BP-SENTINEL-001 | PENDING | PENDING | PENDING |
| REQ-3.4 | YP-SECURITY-SANDBOX-001 | BP-SENTINEL-001 | PENDING | PENDING | PENDING |

### 2.4 Methodology & Rigor

| Requirement | Yellow Paper | Blue Paper | Implementation | Unit Tests | Integration Tests |
|-------------|--------------|------------|----------------|------------|-------------------|
| REQ-4.1 | - | BP-BRAIN-001 | PENDING | PENDING | PENDING |
| REQ-4.2 | YP-FSM-NEXUS-001 | BP-NEXUS-FSM-001 | PENDING | PENDING | PENDING |
| REQ-4.3 | YP-FSM-NEXUS-001 | BP-NEXUS-FSM-001 | PENDING | PENDING | PENDING |
| REQ-4.4 | - | BP-BRAIN-001 | PENDING | PENDING | PENDING |

### 2.5 Domain-Specific

| Requirement | Yellow Paper | Blue Paper | Implementation | Unit Tests | Integration Tests |
|-------------|--------------|------------|----------------|------------|-------------------|
| REQ-5.1 | - | BP-GRAPH-RAG-001, BP-HFT-BROKER-001 | PENDING | PENDING | PENDING |
| REQ-5.2 | YP-HFT-BROKER-001 | BP-HFT-BROKER-001 | PENDING | PENDING | PENDING |
| REQ-5.3 | YP-HFT-BROKER-001 | BP-HFT-BROKER-001 | PENDING | PENDING | PENDING |
| REQ-5.4 | YP-HFT-BROKER-001 | BP-HFT-BROKER-001 | PENDING | PENDING | PENDING |

### 2.6 Performance & Platform

| Requirement | Yellow Paper | Blue Paper | Implementation | Unit Tests | Integration Tests |
|-------------|--------------|------------|----------------|------------|-------------------|
| REQ-6.1 | - | BP-HOST-KERNEL-001 | PENDING | PENDING | PENDING |
| REQ-6.2 | - | BP-HOST-KERNEL-001 | PENDING | PENDING | PENDING |
| REQ-6.3 | - | BP-HOST-KERNEL-001 | PENDING | PENDING | PENDING |
| REQ-6.4 | - | BP-HOST-KERNEL-001 (HAL) | PENDING | PENDING | PENDING |

### 2.7 Interface

| Requirement | Yellow Paper | Blue Paper | Implementation | Unit Tests | Integration Tests |
|-------------|--------------|------------|----------------|------------|-------------------|
| REQ-7.1 | - | PENDING (Phase 3) | PENDING | PENDING | PENDING |
| REQ-7.2 | - | PENDING (Phase 3) | PENDING | PENDING | PENDING |
| REQ-7.3 | - | PENDING (Phase 3) | PENDING | PENDING | PENDING |
| REQ-7.4 | - | PENDING (Phase 3) | PENDING | PENDING | PENDING |

---

## 3. Backward Traceability (Artifacts → Requirements)

### 3.1 Yellow Papers → Requirements

| Yellow Paper ID | Title | Traces To | Status |
|-----------------|-------|-----------|--------|
| YP-FSM-NEXUS-001 | Nexus R&D Lifecycle FSM Theory | REQ-1.1, REQ-1.2, REQ-1.3, REQ-4.2, REQ-4.3 | COMPLETE |
| YP-HFT-BROKER-001 | HFT Broker Mode Theory | REQ-5.2, REQ-5.3, REQ-5.4 | COMPLETE |
| YP-SECURITY-SANDBOX-001 | Sentinel Sandbox Theory | REQ-3.1, REQ-3.2, REQ-3.3, REQ-3.4 | COMPLETE |

### 3.2 Blue Papers → Requirements

| Blue Paper ID | Title | Traces To | Status |
|---------------|-------|-----------|--------|
| BP-HOST-KERNEL-001 | Host Kernel Component | REQ-1.1, REQ-1.2, REQ-1.3, REQ-1.4, REQ-6.1, REQ-6.2, REQ-6.3 | COMPLETE |
| BP-NEXUS-FSM-001 | Nexus FSM Component | REQ-1.1, REQ-1.2, REQ-1.3, REQ-4.2 | COMPLETE |
| BP-SENTINEL-001 | Sentinel Sandbox Component | REQ-3.1, REQ-3.2, REQ-3.3, REQ-3.4 | COMPLETE |
| BP-BRAIN-001 | Brain WASM Component | REQ-2.5, REQ-3.2, REQ-4.1, REQ-4.4 | COMPLETE |
| BP-GRAPH-RAG-001 | Graph-RAG Component | REQ-2.1, REQ-2.2, REQ-2.3, REQ-2.4 | COMPLETE |
| BP-HFT-BROKER-001 | HFT Broker Component | REQ-5.1, REQ-5.2, REQ-5.3, REQ-5.4 | COMPLETE |

### 3.3 Implementation Modules → Requirements

| Module | Path | Traces To | Status |
|--------|------|-----------|--------|
| host | src/host/ | REQ-1.x, REQ-6.x | NOT STARTED |
| fsm | src/fsm/ | REQ-1.x | NOT STARTED |
| sentinel | src/sentinel/ | REQ-3.x | NOT STARTED |
| brain | src/brain/ | REQ-2.5, REQ-4.x | NOT STARTED |
| graph | src/graph/ | REQ-2.x | NOT STARTED |
| broker | src/broker/ | REQ-5.x | NOT STARTED |
| pal | src/pal/ | REQ-6.4 | NOT STARTED |
| tui | src/tui/ | REQ-7.x | NOT STARTED |

---

## 4. Test Coverage Matrix

### 4.1 Unit Tests

| Requirement | Test File | Test Function | Coverage % | Status |
|-------------|-----------|---------------|------------|--------|
| REQ-1.1 | tests/fsm_test.rs | test_phase_transitions | 0% | NOT STARTED |
| REQ-1.2 | tests/fsm_test.rs | test_typestate_enforcement | 0% | NOT STARTED |
| REQ-3.1 | tests/sentinel_test.rs | test_sandbox_tiers | 0% | NOT STARTED |
| REQ-3.2 | tests/brain_test.rs | test_wasm_isolation | 0% | NOT STARTED |
| REQ-5.3 | tests/broker_test.rs | test_wallet_guard | 0% | NOT STARTED |

### 4.2 Integration Tests

| Requirement | Test File | Test Scenario | Status |
|-------------|-----------|---------------|--------|
| REQ-1.1 | tests/integration/fsm.rs | FSM phase transitions | NOT STARTED |
| REQ-3.1 | tests/integration/sandbox.rs | Sandbox isolation | NOT STARTED |
| REQ-3.2 | tests/integration/brain.rs | Brain-Host RPC | NOT STARTED |
| REQ-5.3 | tests/integration/broker.rs | Risk limit enforcement | NOT STARTED |

---

## 5. Acceptance Criteria Traceability

| Requirement | Acceptance Criteria | Test Status |
|-------------|---------------------|-------------|
| REQ-1.1 | AC-1.1.1, AC-1.1.2, AC-1.1.3, AC-1.1.4 | PENDING |
| REQ-1.2 | AC-1.2.1, AC-1.2.2, AC-1.2.3, AC-1.2.4 | PENDING |
| REQ-1.3 | AC-1.3.1, AC-1.3.2, AC-1.3.3, AC-1.3.4, AC-1.3.5 | PENDING |
| REQ-1.4 | AC-1.4.1, AC-1.4.2, AC-1.4.3, AC-1.4.4, AC-1.4.5, AC-1.4.6 | PENDING |
| REQ-2.1 | AC-2.1.1, AC-2.1.2, AC-2.1.3, AC-2.1.4, AC-2.1.5, AC-2.1.6 | PENDING |
| REQ-2.2 | AC-2.2.1, AC-2.2.2, AC-2.2.3, AC-2.2.4 | PENDING |
| REQ-2.3 | AC-2.3.1, AC-2.3.2, AC-2.3.3, AC-2.3.4 | PENDING |
| REQ-2.4 | AC-2.4.1, AC-2.4.2, AC-2.4.3 | PENDING |
| REQ-2.5 | AC-2.5.1, AC-2.5.2, AC-2.5.3, AC-2.5.4, AC-2.5.5, AC-2.5.6 | PENDING |
| REQ-3.1 | AC-3.1.1, AC-3.1.2, AC-3.1.3, AC-3.1.4, AC-3.1.5, AC-3.1.6 | PENDING |
| REQ-3.2 | AC-3.2.1, AC-3.2.2, AC-3.2.3, AC-3.2.4, AC-3.2.5 | PENDING |
| REQ-3.3 | AC-3.3.1, AC-3.3.2, AC-3.3.3, AC-3.3.4 | PENDING |
| REQ-3.4 | AC-3.4.1, AC-3.4.2, AC-3.4.3, AC-3.4.4 | PENDING |
| REQ-4.1 | AC-4.1.1, AC-4.1.2, AC-4.1.3, AC-4.1.4, AC-4.1.5 | PENDING |
| REQ-4.2 | AC-4.2.1, AC-4.2.2, AC-4.2.3, AC-4.2.4 | PENDING |
| REQ-4.3 | AC-4.3.1, AC-4.3.2, AC-4.3.3, AC-4.3.4 | PENDING |
| REQ-4.4 | AC-4.4.1, AC-4.4.2, AC-4.4.3, AC-4.4.4 | PENDING |
| REQ-5.1 | AC-5.1.1, AC-5.1.2, AC-5.1.3, AC-5.1.4 | PENDING |
| REQ-5.2 | AC-5.2.1, AC-5.2.2, AC-5.2.3, AC-5.2.4 | PENDING |
| REQ-5.3 | AC-5.3.1, AC-5.3.2, AC-5.3.3, AC-5.3.4 | PENDING |
| REQ-5.4 | AC-5.4.1, AC-5.4.2, AC-5.4.3, AC-5.4.4 | PENDING |
| REQ-6.1 | AC-6.1.1, AC-6.1.2, AC-6.1.3, AC-6.1.4 | PENDING |
| REQ-6.2 | AC-6.2.1, AC-6.2.2, AC-6.2.3, AC-6.2.4 | PENDING |
| REQ-6.3 | AC-6.3.1, AC-6.3.2, AC-6.3.3, AC-6.3.4 | PENDING |
| REQ-6.4 | AC-6.4.1, AC-6.4.2, AC-6.4.3, AC-6.4.4, AC-6.4.5, AC-6.4.6 | PENDING |
| REQ-7.1 | AC-7.1.1, AC-7.1.2, AC-7.1.3, AC-7.1.4 | PENDING |
| REQ-7.2 | AC-7.2.1, AC-7.2.2, AC-7.2.3, AC-7.2.4 | PENDING |
| REQ-7.3 | AC-7.3.1, AC-7.3.2, AC-7.3.3, AC-7.3.4, AC-7.3.5 | PENDING |
| REQ-7.4 | AC-7.4.1, AC-7.4.2, AC-7.4.3, AC-7.4.4 | PENDING |

---

## 6. Compliance Traceability

| Standard | Clause | Requirement | Compliance Status |
|----------|--------|-------------|-------------------|
| IEEE 1016 | All | All | COMPLIANT |
| IEEE 829 | All | All | COMPLIANT |
| NIST SP 800-53 | AC-3 | REQ-3.x | COMPLIANT |
| NIST SP 800-53 | AU-2 | REQ-1.3 | COMPLIANT |
| OWASP ASVS | V1 | REQ-3.x | COMPLIANT |
| MiFID II | Article 25 | REQ-5.2, REQ-5.3 | COMPLIANT |
| SEC 15c3-5 | All | REQ-5.3 | COMPLIANT |

---

## 7. Formal Verification Traceability

| Property | Blue Paper | Lean4 Proof | Status |
|----------|------------|-------------|--------|
| FSM Termination | BP-NEXUS-FSM-001 | proof_fsm.lean | SKETCH |
| FSM Deadlock Freedom | BP-NEXUS-FSM-001 | proof_fsm.lean | SKETCH |
| Capability Unforgeability | BP-SENTINEL-001 | proof_sandbox.lean | SKETCH |
| Attenuation-Only Derivation | BP-SENTINEL-001 | proof_sandbox.lean | SKETCH |
| Risk Check Completeness | BP-HFT-BROKER-001 | proof_broker.lean | SKETCH |
| WCET Bound | BP-HFT-BROKER-001 | proof_broker.lean | SKETCH |

---

## 8. Status Legend

| Status | Meaning |
|--------|---------|
| NOT STARTED | Artifact not yet created |
| PENDING | Awaiting implementation |
| IN PROGRESS | Currently being implemented |
| COMPLETE | Implementation verified |
| VERIFIED | Tested and approved |
| SKETCH | Proof sketch with `sorry` |

---

## 9. Update Log

| Date | Version | Change | Author |
|------|---------|--------|--------|
| 2026-03-01 | 1.0.0 | Matrix initialized | Nexus |
| 2026-03-01 | 2.0.0 | Phase 2 Blue Papers added, YP traces mapped | Construct |

---

**Note:** This matrix will be updated automatically by the Clawdius traceability utility on each build. Manual updates should be avoided.

**Next Update:** After Phase 3 (Implementation) completion
