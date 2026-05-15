# Graph-RAG

Clawdius uses a Graph-RAG (Retrieval-Augmented Generation) system to provide the LLM with deep codebase context.

## Overview

Graph-RAG combines two complementary approaches:

1. **Structural understanding** via AST (Abstract Syntax Tree) parsing with tree-sitter
2. **Semantic search** via vector embeddings stored in LanceDB

This gives the LLM both precise code structure knowledge and fuzzy semantic matching capability.

## Components

### AST Index (SQLite)

tree-sitter parses source files into ASTs, which are stored in a SQLite database at `.clawdius/graph/index.db`.

Supported languages:

| Language | Parser |
|----------|--------|
| Rust | `tree-sitter-rust` |
| Python | `tree-sitter-python` |
| JavaScript | `tree-sitter-javascript` |
| TypeScript | `tree-sitter-typescript` |
| Go | `tree-sitter-go` |

The AST index enables structural queries like "find all function definitions" or "list imports in this module."

### Vector Store (LanceDB)

Code is embedded into vector representations and stored in LanceDB at `.clawdius/graph/vectors.lance`.

Semantic search allows queries like "find error handling patterns" or "locate database connection code" without exact keyword matching.

### Graph Analysis

The `petgraph` crate powers the code relationship graph, tracking:

- Function call relationships
- Import/dependency chains
- Module hierarchies
- Type usage patterns

## Configuration

```toml
[storage]
database_path = ".clawdius/graph/index.db"
vector_path = ".clawdius/graph/vectors.lance"
```

## Feature Flags

Graph-RAG has two levels of activation:

| Feature | Description |
|---------|-------------|
| `vector-db` | Enable LanceDB vector search |
| `embeddings` | Enable local ML embeddings (tokenizers + HuggingFace) |
| `local-llm` | Full local inference (Candle + embeddings + vector-db) |

## Indexing

Index your workspace:

```bash
# Requires vector-db feature
clawdius index .
clawdius index . --watch    # Watch for changes and re-index
```

## Context Queries

Query the indexed workspace:

```bash
# Requires vector-db feature
clawdius context "how does authentication work"
clawdius context "error handling patterns" --max-tokens 8000
```

## Performance

Graph-RAG queries are optimized for speed:

| Operation | Target |
|-----------|--------|
| AST parse (per file) | < 10ms |
| Vector search | < 50ms |
| Graph traversal | < 5ms |

## How It Works in Practice

When you ask Clawdius a question about your codebase:

1. The query is analyzed to determine what context is needed
2. The AST index is queried for structural matches
3. The vector store is searched for semantic matches
4. Results are ranked and merged into a context window
5. The context is sent to the LLM along with your question

This produces more accurate, grounded responses compared to sending raw file contents.
