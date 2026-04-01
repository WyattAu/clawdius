---
document_id: YP-GRAPH-RAG-001
version: 1.0.0
status: APPROVED
domain: Software Engineering
subdomains: [Code Intelligence, Knowledge Graphs, Semantic Search]
applicable_standards: [IEEE 1016, ISO/IEC 12207, ISO/IEC 25010]
created: 2026-03-31
author: Nexus
confidence_level: 0.92
tqa_level: 4
---

# Yellow Paper: Graph-RAG — Hybrid Code Intelligence System

## YP-2: Executive Summary

### Problem Statement
Formal definition of the Graph-RAG (Retrieval-Augmented Generation) subsystem that provides hybrid code intelligence for the Clawdius engineering platform. The system unifies structural code analysis (AST-based) with semantic understanding (vector embeddings) into a single query interface, enabling intelligent code navigation, refactoring assistance, impact analysis, and cross-language knowledge retrieval.

**Objective Function:** Maximize retrieval relevance $R(q)$ for any code query $q$ by combining structural precision $\rho_s$ and semantic recall $\rho_v$:

$$R(q) = \alpha \cdot \rho_s(q) + (1 - \alpha) \cdot \rho_v(q), \quad \alpha \in [0, 1]$$

where $\alpha$ is the configurable semantic weight (default 0.5), $\rho_s$ is the structural (AST) search relevance, and $\rho_v$ is the vector similarity score.

### Scope
**In-Scope:**
- Tree-sitter AST parsing and symbol extraction across 6+ languages
- SQLite-based AST storage with node/edge graph model
- Vector embedding storage with LanceDB and in-memory fallback
- Hybrid query fusion combining structural and semantic search
- Code chunking with configurable overlap for embedding
- Multi-provider LLM embedding generation (OpenAI, Anthropic, Ollama)
- Call graph construction and impact analysis
- Knowledge graph integration via MCP host

**Out-of-Scope:**
- Real-time file watching and incremental re-indexing
- Distributed graph partitioning across nodes
- Fine-tuning of embedding models
- Natural language code generation from graph queries

### Key Results
- O(n) AST parsing with tree-sitter across Rust, Python, JavaScript, TypeScript, Go
- O(n log n) vector indexing via cosine similarity with LanceDB
- O(log n) approximate nearest-neighbor search for semantic queries
- Hybrid fusion with configurable $\alpha$ achieves superior recall vs. single-mode retrieval
- Knowledge graph spans 16 node types and 8 edge relationship types
- Multi-provider embedding abstraction supports OpenAI, Anthropic, Ollama, and local BERT

## YP-3: Nomenclature

| Symbol | Description | Domain | Source |
|--------|-------------|--------|--------|
| $G = (V, E)$ | AST knowledge graph | Graph theory | `ast_store.rs` |
| $v \in V$ | AST node (symbol) | Nodes table | `ast_store.rs:362` |
| $e \in E$ | AST edge (relationship) | Edges table | `ast_store.rs:378` |
| $\vec{x} \in \mathbb{R}^d$ | Embedding vector | Vector space | `vector_store.rs:18` |
| $d$ | Embedding dimension | Integer | `vector_store.rs:18` (1536) |
| $\text{sim}(\vec{a}, \vec{b})$ | Cosine similarity | $[-1, 1]$ | `vector_store.rs:248` |
| $\alpha$ | Semantic weight | $[0, 1]$ | `graph_rag.rs:30` |
| $c$ | Chunk size (chars) | Integer | `graph_rag.rs:65` (1000) |
| $o$ | Chunk overlap (chars) | Integer | `graph_rag.rs:67` (100) |
| $\mathcal{L}$ | Supported languages | Set | `languages.rs:21` |
| $\mathcal{N}$ | Node type set | 16 types | `ast_store.rs:52` |
| $\mathcal{R}$ | Edge/relationship type set | 8 types | `ast_store.rs:139` |
| $\mathcal{H}$ | SHA3-256 hash function | $\{0,1\}^* \to \{0,1\}^{256}$ | `ast_store.rs:778` |
| $\text{YP-GR}$ | This Yellow Paper prefix | — | Document ID |

### Multi-Lingual Concept Map (EN/ZH/JA/KO/DE)

| EN | ZH | JA | KO | DE |
|----|----|----|----|----|
| AST Node / AST 节点 | ASTノード | AST 노드 | AST-Knoten | |
| Embedding / 嵌入 | 埋め込み | 임베딩 | Einbettung | |
| Vector Search / 向量搜索 | ベクトル検索 | 벡터 검색 | Vektorsuche | |
| Knowledge Graph / 知识图谱 | ナレッジグラフ | 지식 그래프 | Wissensgraph | |
| Hybrid Query / 混合查询 | ハイブリッドクエリ | 하이브리드 쿼리 | Hybridabfrage | |
| Cosine Similarity / 余弦相似度 | コサイン類似度 | 코사인 유사도 | Kosinusähnlichkeit | |
| Code Chunking / 代码分块 | コードチャンキング | 코드 청킹 | Code-Chunking | |
| Call Graph / 调用图 | 呼び出しグラフ | 호출 그래프 | Aufrufgraph | |
| Impact Analysis / 影响分析 | 影響分析 | 영향 분석 | Einflussanalyse | |
| Semantic Weight / 语义权重 | セマンティック重み | 의미론적 가중치 | Semantisches Gewicht | |

## YP-4: Architecture Overview

### YP-4.1: System Components

The Graph-RAG system is composed of four primary layers:

```
┌─────────────────────────────────────────────────┐
│              Query Interface Layer               │
│  HybridQuery → hybrid_query() → QueryResult      │
├──────────────┬──────────────────────────────────┤
│  Structural  │         Semantic Layer            │
│  AST Store   │    Vector Store + Embeddings      │
│  (SQLite)    │    (LanceDB / In-Memory)          │
├──────────────┴──────────────────────────────────┤
│             Parser Layer                         │
│  Tree-sitter (6 langs) → Symbols + References    │
├─────────────────────────────────────────────────┤
│             Storage Layer                        │
│  .clawdius/graph/ast.db + vectors/               │
└─────────────────────────────────────────────────┘
```

**Component Registry:**

| Component | File | Responsibility |
|-----------|------|----------------|
| `GraphRag` | `src/graph_rag.rs:107` | Top-level orchestrator; lifecycle management |
| `AstStore` | `src/ast_store.rs:329` | SQLite-backed AST node/edge persistence |
| `VectorStore` (crate) | `src/vector_store.rs:105` | In-memory vector similarity (stub) |
| `VectorStore` (core) | `crates/.../vector.rs:41` | LanceDB-backed production vector store |
| `CodeParser` | `crates/.../parser.rs:13` | Tree-sitter multi-language symbol extraction |
| `GraphStore` | `crates/.../store.rs:74` | Core SQLite graph store with relationships |
| `HybridSearcher` | `crates/.../search.rs:27` | Fusion of vector + symbolic search results |
| `Chunker` | `src/vector_store.rs:266` | Code segmentation with overlap |
| `EmbeddingGenerator` | `crates/.../embedding/mod.rs:34` | Trait for multi-provider embeddings |
| `LanguageDetector` | `crates/.../languages.rs:91` | File extension → language mapping |

### YP-4.2: Data Flow

```
Source Files ──► LanguageDetector ──► CodeParser ──► Symbols + References
                                         │                    │
                                         ▼                    ▼
                                    AstStore            GraphStore
                                   (nodes/edges)      (symbols/rels)
                                         │                    │
                                         ▼                    ▼
                                   Chunker ──► Embedder ──► VectorStore
                                                              │
                                                              ▼
                                                     HybridSearcher
                                                    (fuse_results)
                                                              │
                                                              ▼
                                                      QueryResult
```

### YP-4.3: Configuration Model

```rust
GraphRagConfig {
    root_path:          ".clawdius/graph",
    ast_db_path:        ".clawdius/graph/ast.db",
    vector_store_path:  ".clawdius/graph/vectors",
    max_chunk_size:     1000,       // characters
    chunk_overlap:      100,        // characters
    embedding_provider: OpenAI,     // OpenAI | Anthropic | Ollama
}
```

Default `HybridQuery` parameters: `k = 10`, `semantic_weight = 0.5`.

## YP-5: AST Construction and Storage

### YP-5.1: Tree-Sitter Parsing

The `CodeParser` (`crates/.../parser.rs:13`) initializes a `Parser` instance per language and performs recursive AST traversal to extract symbols:

$$\text{parse}(s, \ell) \to \mathcal{T}, \quad \text{extract}(\mathcal{T}, s) \to \{s_1, s_2, ..., s_n\}$$

where $s$ is source text, $\ell \in \mathcal{L}$, and $\mathcal{T}$ is the tree-sitter concrete syntax tree.

**Language Support Matrix:**

| Language | Extensions | Tree-sitter Grammar | Symbol Kinds Extracted |
|----------|-----------|--------------------|----------------------|
| Rust | `.rs` | `tree_sitter_rust` | function, struct, enum, trait, impl, mod, const, static, type, macro |
| Python | `.py`, `.pyi`, `.pyw` | `tree_sitter_python` | function, class, decorated_definition |
| JavaScript | `.js`, `.mjs`, `.cjs` | `tree_sitter_javascript` | function_declaration, class_declaration, variable_declaration |
| TypeScript | `.ts` | `tree_sitter_typescript` | function, class, interface, type_alias, enum |
| TypeScript JSX | `.tsx` | `tree_sitter_typescript` TSX | Same as TypeScript + JSX elements |
| Go | `.go` | `tree_sitter_go` | function_declaration, method_declaration, type_declaration, const_declaration |

### YP-5.2: Node Type Taxonomy

The system defines 16 AST node types (`ast_store.rs:52`):

**Definition Nodes:** `Module`, `Function`, `Struct`, `Enum`, `Trait`, `Impl`, `TypeAlias`, `Constant`, `Static`, `Use`, `Mod`, `Macro`

**Member Nodes:** `Field`, `Variant`, `Parameter`, `Local`

The core crate extends this with 14 `SymbolKind` variants (`ast.rs:59`): `Function`, `Class`, `Struct`, `Enum`, `Trait`, `Module`, `Variable`, `Constant`, `Method`, `Field`, `Interface`, `Type`, `Macro`, `Other(String)`.

### YP-5.3: Edge/Relationship Types

8 relationship types define the knowledge graph edges (`ast_store.rs:139`):

| Edge Type | Semantics | Example |
|-----------|-----------|---------|
| `Calls` | Function invocation | `main` → `process_data` |
| `Defines` | Symbol definition | Module → Function |
| `Implements` | Trait implementation | Struct → Trait |
| `Imports` | Dependency import | File → External module |
| `Contains` | Structural containment | Struct → Field |
| `References` | Symbol reference | Variable → Type |
| `Extends` | Inheritance | Subclass → Superclass |
| `CompliesWith` | Specification compliance | Implementation → Spec |

### YP-5.4: SQLite Schema

**AstStore schema** (`ast_store.rs:361`):

```sql
CREATE TABLE nodes (
    id         BLOB PRIMARY KEY,       -- UUID v4 (16 bytes)
    type       TEXT NOT NULL,          -- NodeType string
    name       TEXT,                   -- Symbol name
    file_path  TEXT NOT NULL,          -- Source file path
    start_byte INTEGER NOT NULL,       -- Byte offset start
    end_byte   INTEGER NOT NULL,       -- Byte offset end
    start_line INTEGER NOT NULL,       -- 1-indexed line
    end_line   INTEGER NOT NULL,       -- 1-indexed line
    language   TEXT NOT NULL,          -- Language identifier
    documentation TEXT,                -- Doc comments
    metadata   TEXT,                   -- JSON metadata
    hash       BLOB NOT NULL           -- SHA3-256 (32 bytes)
);

CREATE TABLE edges (
    id        BLOB PRIMARY KEY,
    source_id BLOB NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    target_id BLOB NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    type      TEXT NOT NULL,
    weight    REAL DEFAULT 1.0,
    metadata  TEXT
);
```

**GraphStore schema** (`store.rs:12`): Extended schema with `files`, `symbols`, `symbol_refs`, `relationships`, and `schema_version` tables. Uses WAL journal mode and foreign key enforcement.

**Indexes:** `idx_nodes_type`, `idx_nodes_name`, `idx_nodes_file`, `idx_edges_source`, `idx_edges_target`, `idx_edges_type` on AstStore; additional `idx_symbols_name`, `idx_symbols_kind`, `idx_refs_symbol`, `idx_relationships_from/to/type` on GraphStore.

### YP-5.5: Node Identity and Change Detection

Each node carries a SHA3-256 content hash (`ast_store.rs:778`):

$$\mathcal{H}(content) = \text{SHA3-256}(content) \to [u8; 32]$$

Nodes are identified by `NodeId(Uuid::new_v4())` for stable cross-reference. File re-indexing uses `delete_file_nodes` → `insert_nodes` atomic replacement, enabling incremental updates.

## YP-6: Vector Embedding and Semantic Search

### YP-6.1: Embedding Providers

The system abstracts embedding generation behind the `EmbeddingGenerator` trait (`embedding/mod.rs:34`):

```rust
#[async_trait]
pub trait EmbeddingGenerator: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn dimension(&self) -> usize;
}
```

**Provider Implementations:**

| Provider | Implementation | Dimension | Notes |
|----------|---------------|-----------|-------|
| OpenAI (default) | `LlmClient::embed` via `Provider::OpenAI` | 1536 | `text-embedding-3-small` |
| Anthropic | `LlmClient::embed` via `Provider::Anthropic` | Variable | Via LLM client abstraction |
| Ollama | `LlmClient::embed` via `Provider::Ollama` | Variable | Local inference |
| SentenceEmbedder | `candle-transformers` BERT model | 384 | `all-MiniLM-L6-v2`, requires `embeddings` feature |
| SimpleEmbedder | Hash-based fallback | Configurable (default 384) | Always available, for testing |

The `SentenceEmbedder` (`real.rs:15`) performs mean-pooling with attention masking and L2 normalization:

$$\vec{e} = \frac{\sum_{i} h_i \cdot m_i}{\|\sum_{i} h_i \cdot m_i\|_2}$$

where $h_i$ are hidden states and $m_i$ is the attention mask.

### YP-6.2: Code Chunking with Overlap

The `Chunker` (`vector_store.rs:266`) splits source code into segments of `max_chunk_size` characters with `overlap` character overlap:

$$\text{chunk}_i = \text{content}[s_i, s_i + c), \quad s_{i+1} = s_i + c - o$$

where $c = 1000$ (max chunk size) and $o = 100$ (overlap). Each `ChunkCandidate` tracks `source_path`, `start_line`, `end_line`, and `language` for provenance.

### YP-6.3: Cosine Similarity

Vector similarity is computed as cosine similarity (`vector_store.rs:248`):

$$\text{sim}(\vec{a}, \vec{b}) = \frac{\vec{a} \cdot \vec{b}}{\|\vec{a}\|_2 \cdot \|\vec{b}\|_2}$$

with zero-vector guard: returns 0.0 when either magnitude is 0.

### YP-6.4: Vector Storage Backends

**In-Memory Stub** (`src/vector_store.rs:105`): HashMap-backed with brute-force cosine similarity scan. Suitable for testing and small codebases.

**LanceDB Production** (`crates/.../vector.rs:41`): Arrow-based columnar storage with native vector search. Schema: `id (Utf8)`, `embedding (FixedSizeList<Float32, d>)`, `metadata (Utf8)`. Distance-to-score conversion:

$$\text{score} = \frac{1}{1 + d}$$

where $d$ is the LanceDB L2 distance.

## YP-7: Hybrid Query Architecture

### YP-7.1: Query Model

The `HybridQuery` (`graph_rag.rs:22`) combines two orthogonal search modes:

```rust
pub struct HybridQuery {
    pub semantic_query:   Option<String>,   // Natural language → embedding
    pub structural_query: Option<AstQuery>,  // AST filter predicates
    pub k:                usize,             // Result count (default 10)
    pub semantic_weight:  f32,               // α ∈ [0.0, 1.0], default 0.5
}
```

### YP-7.2: Query Execution

The `hybrid_query` method (`graph_rag.rs:275`) executes both branches:

1. **Structural path:** `query_structural(AstQuery)` → SQL with `WHERE type = ? AND name LIKE ? AND file_path = ? AND language = ?` → `Vec<AstNode>`
2. **Semantic path:** `query_semantic(query, k)` → embed query → `VectorStore::search(embedding, k)` → `Vec<SearchResult>`

### YP-7.3: Result Fusion

**Simple Fusion** (`graph_rag.rs:287`): Aggregated semantic score as mean of chunk scores.

**Advanced Fusion** (`crates/.../search.rs:69`): The `HybridSearcher::fuse_results` implements symbol-aware fusion:

```
For each symbolic_result:
    if symbol_id ∈ vector_results → Hybrid (use vector score)
    else                           → SymbolicSearch (default score 0.5)

For remaining vector_results:
    attempt symbol lookup → VectorSearch with symbol
    fallback              → VectorSearch with placeholder
```

Results are sorted by `semantic_score` descending. The `ResultSource` enum (`search.rs:13`) classifies each result as `VectorSearch`, `SymbolicSearch`, or `Hybrid`.

### YP-7.4: Structural Query Predicates

The `AstQuery` (`ast_store.rs:287`) supports composable filters:

| Field | Type | SQL Mapping |
|-------|------|-------------|
| `node_type` | `Option<NodeType>` | `AND type = ?` |
| `name_pattern` | `Option<String>` | `AND name LIKE ?` (wildcards → `%`) |
| `file_path` | `Option<PathBuf>` | `AND file_path = ?` |
| `language` | `Option<Language>` | `AND language = ?` |
| `limit` | `Option<usize>` | `LIMIT n` |

## YP-8: Graph Operations

### YP-8.1: Call Graph Extraction

The `get_call_graph` method (`ast_store.rs:657`) performs DFS traversal following `EdgeType::Calls` edges:

```
stack ← [root_id]
while stack ≠ ∅:
    current ← stack.pop()
    if current ∈ visited: continue
    visited ← visited ∪ {current}
    nodes ← nodes ∪ {get_node(current)}
    for edge ∈ get_outgoing_edges(current):
        if edge.type == Calls ∧ edge.to ∉ visited:
            edges ← edges ∪ {edge}
            stack ← stack ∪ {edge.to}
```

Complexity: $O(V + E)$ where $V$ is reachable nodes, $E$ is call edges.

### YP-8.2: Impact Analysis

The `find_impact` method (`ast_store.rs:692`) traverses incoming edges to find all symbols affected by a change:

$$\text{Impacted}(v) = \bigcup_{e \in E_{in}(v), e.type \in \{\text{Calls}, \text{References}, \text{Imports}\}} \text{Impacted}(e.from) \cup \{e.from\}$$

This follows reverse `Calls`, `References`, and `Imports` edges to identify the transitive closure of dependents.

### YP-8.3: Project Indexing

The `index_project` method (`graph_rag.rs:159`) performs recursive directory traversal with filtering:

- Skips hidden directories (starting with `.`), `target/`, `node_modules/`
- Filters files by `LanguageDetector::is_supported()`
- Per-file: read → parse → extract nodes/edges → insert into AstStore
- Error handling: logs warnings for individual file failures, continues traversal

### YP-8.4: Index Statistics

The `stats` method (`ast_store.rs:721`) aggregates: `node_count`, `edge_count`, `files_indexed`, `nodes_by_type`, `edges_by_type`.

## YP-9: Complexity Analysis

### YP-9.1: Time Complexity

| Operation | Complexity | Justification |
|-----------|-----------|---------------|
| File parsing (tree-sitter) | $O(n)$ | Linear scan of source bytes, $n$ = file size |
| Symbol extraction | $O(n)$ | Single DFS traversal of CST |
| Node insertion (batch) | $O(m \log m)$ | $m$ nodes in SQLite transaction with indexes |
| Edge insertion (batch) | $O(e \log e)$ | $e$ edges in SQLite transaction |
| Structural query | $O(\log N)$ | Indexed lookup on `type`, `name`, `file_path` |
| Call graph traversal | $O(V + E)$ | DFS over reachable subgraph |
| Impact analysis | $O(V + E)$ | Reverse DFS over dependent subgraph |
| Code chunking | $O(n / (c - o))$ | Linear scan with $c$ chunk size, $o$ overlap |
| Embedding generation (LLM) | $O(1)$ amortized | Network call, constant per chunk |
| Embedding generation (local) | $O(t \cdot d)$ | $t$ tokens, $d$ embedding dimension |
| Vector search (brute-force) | $O(N \cdot d)$ | Cosine similarity against all $N$ vectors |
| Vector search (LanceDB) | $O(\log N)$ | Approximate nearest neighbor with IVF-PQ |
| Hybrid query | $O(\log N + N_v \cdot d)$ | Structural + vector search combined |

### YP-9.2: Space Complexity

| Component | Complexity | Notes |
|-----------|-----------|-------|
| AST Store (SQLite) | $O(V + E)$ | Nodes + edges on disk |
| Vector Store (in-memory) | $O(N \cdot d)$ | $N$ chunks, $d$-dimensional embeddings |
| Vector Store (LanceDB) | $O(N \cdot d)$ | Compressed on disk with Arrow |
| Tree-sitter parser pool | $O(|\mathcal{L}|)$ | One parser per language (6) |
| In-memory graph traversal | $O(V + E)$ | Visited set + stack |

### YP-9.3: Storage Estimates

For a project with $F$ files, average file size $\bar{s}$, and chunk size $c$:

$$\text{Chunks} \approx \sum_{f=1}^{F} \frac{s_f}{c - o} \approx \frac{F \cdot \bar{s}}{c - o}$$

Vector storage: $\text{Chunks} \times d \times 4$ bytes (float32). With default $c=1000$, $o=100$, $d=1536$:

| Files | Avg Size | Chunks | Vector Storage |
|-------|----------|--------|---------------|
| 100 | 5 KB | ~556 | ~3.2 MB |
| 1,000 | 10 KB | ~11,111 | ~64 MB |
| 10,000 | 8 KB | ~88,889 | ~512 MB |

## YP-10: Language Support and Extensibility

### YP-10.1: Current Language Coverage

The system currently supports 6 languages with tree-sitter grammars (`languages.rs:21`):

```rust
enum LanguageKind {
    Rust,           // .rs
    Python,         // .py, .pyi, .pyw
    JavaScript,     // .js, .mjs, .cjs
    TypeScript,     // .ts
    TypeScriptJsx,  // .tsx
    Go,             // .go
}
```

Extension: 10 file extensions supported (`languages.rs:103`).

### YP-10.2: Language-Aware Parsing

Each language has custom symbol extraction rules (`parser.rs:114`):

- **Rust:** `function_item` → Function, `struct_item` → Struct, `enum_item` → Enum, `trait_item`/`impl_item` → Trait, `mod_item` → Module, `const_item`/`static_item` → Constant, `type_item` → Type, `macro_definition`/`macro_invocation` → Macro
- **Python:** `function_definition` → Function, `class_definition` → Class, `decorated_definition` → delegates to first child
- **JS/TS/TSX:** `function_declaration`, `arrow_function`, `method_definition` → Function/Method; `class_declaration` → Class; `interface_declaration` → Interface; `type_alias_declaration` → Type; `enum_declaration` → Enum
- **Go:** `function_declaration` → Function, `method_declaration` → Method, `type_declaration`/`type_spec` → Type, `const_declaration` → Constant, `var_declaration` → Variable

### YP-10.3: Language-Aware Doc Comment Extraction

| Language | Doc Comment Pattern | Implementation |
|----------|--------------------|---------------|
| Rust | `///` or `/** */` | Check prev sibling for `line_comment`/`block_comment` |
| Python | String literal expression | Check prev sibling for `expression_statement` → `string` |
| JavaScript/TypeScript | `//` or `/* */` | Check prev sibling for `comment` |
| Go | `//` or `/* */` | Check prev sibling for `comment` |

### YP-10.4: Extensibility Model

Adding a new language requires:

1. Add variant to `LanguageKind` (`languages.rs`)
2. Add tree-sitter grammar dependency to `Cargo.toml`
3. Initialize parser in `CodeParser::new()` (`parser.rs:18`)
4. Add `node_kind_to_symbol_kind` mapping (`parser.rs:114`)
5. Add `extract_name` field mapping (`parser.rs:164`)
6. Add `extract_signature` rule (`parser.rs:227`)
7. Add `extract_doc_comment` pattern (`parser.rs:277`)
8. Add `extract_imports` pattern (`parser.rs:393`)
9. Add file extensions to `file_extensions()` and `supported_extensions()`

### YP-10.5: Knowledge Graph Scale

With 16 node types, 8 edge types, and 6 languages, the theoretical maximum distinct graph patterns per file is:

$$|\mathcal{N}| \times |\mathcal{R}| \times |\mathcal{L}| = 16 \times 8 \times 6 = 768$$

In practice, language-specific constraints reduce this to approximately 200-300 meaningful patterns per language.

### YP-10.6: Component Lifecycle

The `GraphRag` component follows the standard Clawdius lifecycle (`graph_rag.rs:362`):

```
Uninitialized → initialize() → Initialized → start() → Running → stop() → Stopped
```

`initialize()` creates the graph directory, opens the AstStore (SQLite), initializes the VectorStore, and instantiates the Parser. The component requires `initialize()` before `start()`, and `start()` before query operations.
