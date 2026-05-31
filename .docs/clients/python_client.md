# Clawdius Python Client

> Python bindings for the Clawdius agentic coding engine.
> Status: PLANNED for v1.1.0 | Last updated: 2026-05-30

## Installation

```bash
pip install clawdius
```

## Quickstart

```python
from clawdius import Client

client = Client()
response = await client.chat("Explain this code", files=["main.rs"])
print(response.content)
```

## Features (Planned)

- Async/await native API
- Session management with persistence
- Multi-provider LLM support (Anthropic, OpenAI, DeepSeek, Ollama)
- Tool execution with sandboxing
- Streaming responses via async generators
- VSCode/Jupyter integration

## API Reference (Planned)

### Client
| Method | Description |
|--------|-------------|
| `Client(api_key=None, provider="anthropic")` | Initialize client |
| `chat(prompt, files=[], tools=[])` | Send chat message |
| `stream(prompt, files=[], tools=[])` | Stream response chunks |
| `sessions.list()` | List all sessions |
| `sessions.create(title=None)` | Create new session |

### Session
| Method | Description |
|--------|-------------|
| `session.messages()` | Get message history |
| `session.add_user(text)` | Add user message |
| `session.add_assistant(text)` | Add assistant message |
| `session.save()` | Persist session |
| `session.export(format="json")` | Export session data |

## Implementation Notes

- Built on `clawdius-core` Rust library via PyO3 bindings
- Or: pure Python HTTP client communicating with local clawdius daemon
- Decision pending: PyO3 native vs HTTP client (trade-off: startup latency vs distribution complexity)
