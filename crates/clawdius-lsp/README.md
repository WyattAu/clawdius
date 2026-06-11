# Clawdius LSP Server

Language Server Protocol server for Clawdius, providing IDE integration with Tree-sitter symbol extraction and Graph-RAG code intelligence.

## Protocol Support

| Method | Status | Backend |
|--------|--------|---------|
| `textDocument/didOpen` | Working | Tree-sitter indexing |
| `textDocument/didChange` | Working | Incremental re-index |
| `textDocument/documentSymbol` | Working | Tree-sitter extraction |
| `textDocument/hover` | Scaffold | Graph-RAG integration pending |
| `textDocument/definition` | Scaffold | Symbol index pending |
| `textDocument/references` | Scaffold | Symbol index pending |
| `clawdius/analyze` (custom) | Working | Returns index summary |
| `clawdius/verify` (custom) | Working | Returns proof stats |

## Languages

Symbol extraction via Tree-sitter (inherited from clawdius-core):

- Rust, Python, JavaScript, TypeScript, TSX, Go, Java, C++, PHP, Ruby

## Usage

```bash
# Build
cargo build -p clawdius-lsp --release

# Run (communicates over stdio)
clawdius-lsp
```

Configure your editor to use `clawdius-lsp` as the language server binary.

## Architecture

```
IDE Client
    |
    | Language Server Protocol (JSON-RPC over stdio)
    v
clawdius-lsp binary (Rust)
    |
    | tower-lsp framework
    v
clawdius-core graph_rag module (Tree-sitter + symbol extraction)
```

## License

Apache 2.0
