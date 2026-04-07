# Clawdius Emacs Integration

AI-powered coding assistant for Emacs, powered by the [Clawdius](https://github.com/WyattAu/clawdius) server.

## Requirements

- Emacs 27.1 or later
- Clawdius server running at `http://localhost:8080`
- `company-mode` (optional, for inline completions)

## Installation

### Manual

Add `clawdius.el` to your load path and require it:

```elisp
(add-to-list 'load-path "/path/to/clawdius/editors/emacs")
(require 'clawdius)
(clawdius-setup)
```

### use-package

```elisp
(use-package clawdius
  :load-path "/path/to/clawdius/editors/emacs"
  :commands (clawdius-setup
             clawdius-chat
             clawdius-analyze
             clawdius-explain
             clawdius-refactor
             clawdius-fix
             clawdius-diff
             clawdius-complete-at-point
             clawdius-health
             clawdius-add-context
             clawdius-checkpoint)
  :config
  (clawdius-setup))
```

## Configuration

| Variable | Default | Description |
|---|---|---|
| `clawdius-host` | `"localhost"` | Server hostname |
| `clawdius-port` | `8080` | Server port |
| `clawdius-api-key` | `nil` | API key for authentication |
| `clawdius-enable-completion` | `t` | Enable inline completions |
| `clawdius-completion-trigger-chars` | `'. ?\( ?\s'` | Characters that trigger completion |
| `clawdius-completion-debounce` | `0.5` | Debounce delay (seconds) |
| `clawdius-model` | `nil` | LLM model (nil = server default) |
| `clawdius-request-timeout` | `30` | HTTP request timeout (seconds) |

### Example configuration

```elisp
(setq clawdius-host "localhost"
      clawdius-port 8080
      clawdius-enable-completion t
      clawdius-completion-trigger-chars '(?. ?\( ?\s ?\n)
      clawdius-model "deepseek")
```

## Commands

| Command | Key | Description |
|---|---|---|
| `clawdius-chat` | — | Open the AI chat buffer |
| `clawdius-analyze` | — | Analyze the current file |
| `clawdius-explain` | — | Explain code at point |
| `clawdius-explain-region` | — | Explain the selected region |
| `clawdius-refactor` | — | Suggest refactoring for region |
| `clawdius-fix` | — | Fix issues in the current file |
| `clawdius-complete-at-point` | — | Trigger inline completion |
| `clawdius-health` | — | Check server connection |
| `clawdius-diff` | — | Show git diff |
| `clawdius-add-context` | — | Add a file as chat context |
| `clawdius-checkpoint` | — | Create a session checkpoint |

## Chat Buffer Keybindings

| Key | Action |
|---|---|
| `RET` | Send message |
| `C-c C-c` | Abort current request |
| `C-c C-k` | Kill chat buffer |
| `C-c C-f` | Add file context |
| `C-c C-p` | Create checkpoint |

## company-mode Integration

When `clawdius-enable-completion` is non-nil and `clawdius-setup` has been called, the `company-clawdius` backend is automatically registered. It provides AI-powered completions triggered by the characters in `clawdius-completion-trigger-chars`.

To verify it is active:

```elisp
;; Check that the backend is registered
(member 'company-clawdius company-backends)
```

To disable for specific modes:

```elisp
(add-hook 'org-mode-hook
          (lambda ()
            (setq-local company-backends
                        (remove 'company-clawdius company-backends))))
```

## Troubleshooting

**Server not available**

Run `M-x clawdius-health` to verify the server is reachable. If the server is not running, start it before using the plugin:

```bash
clawdius-server --port 8080
```

**Completions not appearing**

1. Ensure `company-mode` is active in the current buffer (`M-x company-mode`).
2. Verify `clawdius-enable-completion` is `t`.
3. Check the health status with `M-x clawdius-health`.
4. Confirm `company-clawdius` is in `company-backends`.

**API authentication**

If your server requires authentication, set the API key:

```elisp
(setq clawdius-api-key "your-api-key-here")
```

Alternatively, set it via an environment variable in your init:

```elisp
(setq clawdius-api-key (getenv "CLAWDIUS_API_KEY"))
```
