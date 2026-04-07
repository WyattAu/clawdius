# Clawdius Neovim Plugin

AI-powered coding assistance for Neovim via the Clawdius server.

## Prerequisites

- Neovim 0.8+
- `curl` installed and available on `$PATH`
- Clawdius server running (default: `http://localhost:8080`)

## Installation

### Lazy.nvim

```lua
{
    'WyattAu/clawdius',
    dir = '/path/to/clawdius/plugins/neovim',
    config = function()
        require('clawdius').setup({
            host = 'localhost',
            port = 8080,
        })
    end,
}
```

### Packager / Manual

Copy `clawdius.lua` into your Neovim runtime path (e.g. `~/.local/share/nvim/site/plugin/`), then add to your config:

```lua
require('clawdius').setup({
    host = 'localhost',
    port = 8080,
    api_key = nil,  -- or set CLAWDIUS_API_KEY env var
})
```

## Configuration

| Option    | Default      | Description                        |
|-----------|-------------|------------------------------------|
| `host`    | `'localhost'` | Clawdius server hostname          |
| `port`    | `8080`       | Clawdius server port              |
| `api_key` | `nil`        | API key (falls back to `$CLAWDIUS_API_KEY`) |
| `enabled` | `true`       | Enable/disable the plugin          |
| `timeout` | `10000`      | HTTP request timeout in ms         |

## Commands

| Command            | Description                          |
|--------------------|--------------------------------------|
| `:ClawdiusChat`    | Open a floating chat window          |
| `:ClawdiusAnalyze` | Analyze the current file             |
| `:ClawdiusHealth`  | Check connection to Clawdius server  |

## Chat Window

- `:ClawdiusChat` opens a centered floating window.
- Type your question, then press `<CR>` to send it.
- The response replaces your input in the buffer.
- Press `q` to close the window.

## nvim-cmp Integration

If [nvim-cmp](https://github.com/hrsh7th/nvim-cmp) is installed, Clawdius automatically registers as a completion source. Add it to your cmp setup:

```lua
sources = {
    { name = 'clawdius' },
    -- ... other sources
}
```

Completions trigger on `.`, `:`, and `(`.

## API Endpoints Used

| Endpoint              | Method | Description          |
|-----------------------|--------|----------------------|
| `/health`             | GET    | Server health check  |
| `/api/v1/complete`    | POST   | Code completions     |
| `/api/v1/chat`        | POST   | Chat with AI         |
| `/api/v1/analyze`     | POST   | Code analysis        |
| `/api/v1/git/status`  | GET    | Git repository status|

## Lua API

```lua
local clawdius = require('clawdius')

clawdius.health()                -- -> (ok: boolean, msg: string)
clawdius.complete(line, col, cb) -- cb(completions: table)
clawdius.chat(question, cb)      -- cb(reply: string)
clawdius.analyze(cb)             -- cb(analysis: string)
clawdius.git_status(cb)          -- cb(status: table)
clawdius.open_chat()             -- opens floating chat window
```
