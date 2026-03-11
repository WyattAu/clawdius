# Clawdius Knowledge Graph

Knowledge graph mapping concepts, relationships, and terminology from the Clawdius Yellow Papers and Blue Papers.

## Structure

```
.clawdius/knowledge_graph/
├── concepts.json         # 40 core concepts extracted from specifications
├── relationships.json    # 42 relationships between concepts
├── terminology.json      # 27 multi-lingual term translations (7 languages)
├── graph_metadata.json   # Statistics and source document mapping
└── README.md             # This file
```

## Concepts by Category

### Process Engineering
- Nexus FSM (24-Phase R&D Lifecycle)
- Phase, Transition, Event
- Quality Gate, Artifact
- SOP (Standard Operating Procedure)
- Artifact Tracker

### Security
- Sentinel Sandbox (4-Tier Isolation)
- Capability-Based Security
- Isolation Boundary
- Sandbox Tier (Native/Container/WASM/Hardened)
- Secret Isolation
- Brain-Leaking (Attack Pattern)
- Supply Chain Security

### Memory & Performance
- Zero-GC Memory Model
- Arena Allocator
- Ring Buffer (Lock-Free SPSC)
- WCET (Worst-Case Execution Time)
- Cache Line Optimization

### Architecture
- Host Kernel (TCB)
- Brain WASM (Intelligence Layer)
- Graph-RAG (Knowledge Layer)
- HAL (Hardware Abstraction Layer)

### AI/ML
- AST Index (SQLite)
- Vector Store (LanceDB)
- Embedding, Semantic Search

### Risk Management (HFT)
- Wallet Guard
- Position Limit
- Drawdown
- SEC Rule 15c3-5

## Relationship Types

| Type | Count | Description |
|------|-------|-------------|
| `uses` | 9 | Component dependency |
| `enforces` | 6 | Policy/gate enforcement |
| `implements` | 5 | Specification implementation |
| `requires` | 4 | Necessary condition |
| `enables` | 4 | Makes possible |
| `provides` | 3 | Service offering |
| `integrates` | 2 | System connection |
| `contains` | 2 | Composition |

## Multi-Lingual Support

Languages supported (7 total):
- **EN** - English (canonical)
- **ZH** - 中文 (Chinese)
- **RU** - Русский (Russian)
- **DE** - Deutsch (German)
- **FR** - Français (French)
- **JP** - 日本語 (Japanese)
- **KO** - 한국어 (Korean)

## Source Documents

### Yellow Papers (Theoretical Foundations)
| ID | Title | Concepts |
|----|-------|----------|
| YP-FSM-NEXUS-001 | Nexus R&D Lifecycle FSM Theory | 7 |
| YP-SECURITY-SANDBOX-001 | Sentinel Sandbox Theory | 6 |
| YP-HFT-BROKER-001 | HFT Broker Mode Theory | 7 |

### Blue Papers (Architectural Specifications)
| ID | Title | Concepts |
|----|-------|----------|
| BP-NEXUS-FSM-001 | Nexus FSM Component | 2 |
| BP-HOST-KERNEL-001 | Host Kernel Component | 4 |
| BP-BRAIN-001 | Brain WASM Component | 3 |
| BP-SENTINEL-001 | Sentinel Sandbox Component | 2 |
| BP-GRAPH-RAG-001 | Graph-RAG Component | 5 |
| BP-HFT-BROKER-001 | HFT Broker Component | 2 |

## Usage

### Querying Concepts
```bash
# Find concept by ID
jq '.concepts[] | select(.id == "CONCEPT-001")' concepts.json

# List all security concepts
jq '.concepts[] | select(.category == "Security")' concepts.json
```

### Querying Relationships
```bash
# Find all relationships for a concept
jq '.relationships[] | select(.source_id == "CONCEPT-001")' relationships.json

# Find dependencies
jq '.relationships[] | select(.type == "depends-on" or .type == "requires")' relationships.json
```

### Multi-Lingual Lookup
```bash
# Get Chinese translation for a term
jq '.terms[] | select(.canonical == "Finite State Machine") | .translations.ZH' terminology.json
```

## Confidence Levels

| Level | Range | Count |
|-------|-------|-------|
| High | ≥ 0.90 | 38 |
| Medium | 0.85-0.89 | 2 |
| Low | < 0.85 | 0 |

Average confidence: **91.25%**

## Graph Statistics

- **Total Concepts**: 40
- **Total Relationships**: 42
- **Total Terms**: 27 (with 7-language translations)
- **Categories**: 22
- **Source Documents**: 9 (3 YP + 6 BP)

## Maintenance

To update the knowledge graph:
1. Add new concepts to `concepts.json` with unique IDs
2. Create relationships in `relationships.json`
3. Add translations to `terminology.json`
4. Update statistics in `graph_metadata.json`

---

**Version**: 1.0.0  
**Created**: 2026-03-08  
**Maintainer**: Clawdius Project
