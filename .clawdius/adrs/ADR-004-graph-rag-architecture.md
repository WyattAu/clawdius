# ADR-004: Graph-RAG Architecture

## Status
Accepted

## Context
Clawdius requires intelligent code understanding to support:
- **Cross-file refactoring**: Identifying all call-sites when changing APIs
- **Impact analysis**: Understanding downstream effects of modifications
- **Semantic search**: Natural language queries against codebases
- **Research synthesis**: Integrating multi-lingual technical documentation

Traditional approaches have limitations:
- **grep/ripgrep**: Text-only, no structural understanding
- **LSP-based indexing**: Language-specific, limited semantic capabilities
- **Full-text search**: No code structure awareness
- **Vector-only approaches**: No structural relationships

The system needs both:
1. **Structural understanding**: AST-based code graphs with call relationships
2. **Semantic understanding**: Vector embeddings for natural language queries

## Decision
Implement a **hybrid Graph-RAG (Retrieval-Augmented Generation)** architecture combining:

### AST Index (SQLite)
- **Storage**: Local SQLite database for nodes and edges
- **Parsing**: tree-sitter for incremental, multi-language AST extraction
- **Schema**: Nodes (functions, structs, modules) and Edges (calls, defines, imports)

```sql
CREATE TABLE nodes (
    id BLOB PRIMARY KEY,
    type TEXT NOT NULL,
    name TEXT,
    file_path TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    language TEXT NOT NULL,
    hash BLOB NOT NULL
);

CREATE TABLE edges (
    source_id BLOB REFERENCES nodes(id),
    target_id BLOB REFERENCES nodes(id),
    type TEXT NOT NULL
);
```

### Vector Store (LanceDB)
- **Storage**: Columnar vector database for embeddings
- **Embedding**: 1536-dimensional vectors (CodeBERT/OpenAI)
- **Chunking**: Function/class-level granularity

### Supported Languages
| Language | tree-sitter Grammar | Embedding Model |
|----------|---------------------|-----------------|
| Rust | tree-sitter-rust | codebert |
| TypeScript | tree-sitter-typescript | codebert |
| Python | tree-sitter-python | codebert |
| C++ | tree-sitter-cpp | codebert |
| Go | tree-sitter-go | codebert |
| Java | tree-sitter-java | codebert |

### MCP Host Integration
Model Context Protocol support for extensible tool integration without code changes.

## Consequences

### Positive
- **Zero-configuration**: SQLite embedded, no external database required
- **Incremental indexing**: tree-sitter supports delta updates on file changes
- **Hybrid queries**: Combine structural (AST) and semantic (vector) results
- **Performance**: <10ms structural queries, <50ms semantic search
- **Portability**: SQLite + LanceDB files stored in `.clawdius/graph/`

### Negative
- **Storage overhead**: ~50MB AST + ~500MB vectors per 100k LOC
- **Embedding cost**: API calls for initial indexing (mitigated by caching)
- **Language support**: Limited to tree-sitter grammar availability
- **Memory usage**: 200MB+ for large project indices

## Alternatives Considered

### Neo4j
| Aspect | Neo4j | SQLite |
|--------|-------|--------|
| Deployment | External server | Embedded |
| Query power | Cypher, graph algorithms | SQL |
| Setup | Complex | Zero-config |
| Cost | Commercial/Resource-heavy | Free |

**Rejected**: External dependency violates single-binary deployment goal; unnecessary complexity for local code graphs.

### PostgreSQL + pgvector
| Aspect | PostgreSQL | SQLite + LanceDB |
|--------|------------|------------------|
| Deployment | External server | Embedded files |
| Features | Full SQL + vectors | Separated |
| Size | Large | Small |

**Rejected**: Overkill for embedded use case; requires external server; larger attack surface.

### ripgrep-only
| Aspect | ripgrep | Graph-RAG |
|--------|---------|-----------|
| Speed | Very fast | Fast |
| Understanding | Text-only | Structural + Semantic |
| Relationships | None | Call graphs |

**Rejected**: No structural understanding; cannot answer "who calls this function?" or "what does this struct contain?"

### Elasticsearch
| Aspect | Elasticsearch | LanceDB |
|--------|---------------|---------|
| Deployment | Cluster | Embedded |
| Features | Full-text + vectors | Vectors only |
| Complexity | High | Low |

**Rejected**: Requires JVM; cluster management complexity; violates single-binary goal.

## Related Standards
- **IEEE 1016**: Data Design (Section 8.2)
- **MCP Specification 2024.11**: Protocol compliance for tool integration
- **tree-sitter ABI**: Parser interface stability

## Related ADRs
- ADR-001: Rust Native Implementation
- ADR-003: WASM Runtime Selection (for Brain access to Graph-RAG)

## Date
2026-03-08

## Author
Construct (Systems Architect Agent)
