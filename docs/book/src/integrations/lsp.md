# LSP Server

Clawdius includes a built-in Language Server Protocol server (`clawdius-lsp`) that provides code intelligence to any LSP-compatible editor.

## Installation

```bash
# Build from source
cargo build --release -p clawdius-lsp

# Binary location
./target/release/clawdius-lsp
```

## Supported Features

| LSP Method | Status | Backend |
|-----------|--------|---------|
| `textDocument/didOpen` | Working | Tree-sitter indexing |
| `textDocument/didChange` | Working | Incremental re-index |
| `textDocument/documentSymbol` | Working | Tree-sitter extraction |
| `textDocument/hover` | Planned | Graph-RAG integration |
| `textDocument/definition` | Planned | Symbol index |
| `textDocument/references` | Planned | Symbol index |
| `clawdius/analyze` (custom) | Working | Returns index summary |
| `clawdius/verify` (custom) | Working | Returns proof stats |

## Languages

Symbol extraction via Tree-sitter (inherited from clawdius-core):

- Rust, Python, JavaScript, TypeScript, TSX, Go, Java, C++, PHP, Ruby

## Editor Configuration

### Neovim (nvim-lspconfig)

```lua
require('lspconfig').clawdius_lsp.setup {
  cmd = { 'clawdius-lsp' },
  filetypes = { 'rust', 'python', 'javascript', 'typescript', 'go' },
}
```

### VSCode

Install the Clawdius extension from `extensions/clawdius/`. The extension communicates with `clawdius-code` (JSON-RPC over stdio), which delegates to the same core library.

### Emacs (lsp-mode)

```elisp
(lsp-register-client
 (make-lsp-client
  :new-connection (lsp-stdio-connection "clawdius-lsp")
  :major-modes '(rust-mode python-mode go-mode js-mode typescript-mode)
  :server-id 'clawdius-lsp))
```

### Helix

```toml
# .helix/languages.toml
[language-server.clawdius-lsp]
command = "clawdius-lsp"
```

## Architecture

```
Editor (LSP client)
    |
    | JSON-RPC over stdio
    v
clawdius-lsp (tower-lsp)
    |
    | clawdius-core graph_rag module
    v
Tree-sitter parsers (10 languages) + symbol index
```

The LSP server is a thin layer over clawdius-core's existing Tree-sitter parsing infrastructure. No additional indexing step is required -- documents are parsed on open and change events.

## Custom Methods

### `clawdius/analyze`

Returns a summary of the current symbol index:

```json
{
  "documents": 15,
  "symbols": 342
}
```

### `clawdius/verify`

Returns Lean4 proof verification status:

```json
{
  "status": "ok",
  "theorems": 318,
  "proof_files": 22,
  "lake_jobs": "21 libs + 1 exe"
}
```
