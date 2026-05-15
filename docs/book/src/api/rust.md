# Rust API

The `clawdius-core` crate provides the programmatic interface to Clawdius.

## Adding the Dependency

```toml
[dependencies]
clawdius-core = "1.0.0-rc.1"
```

## Quick Start

```rust
use clawdius_core::{Config, SessionManager};
use clawdius_core::llm::{create_provider, LlmConfig, ChatMessage, ChatRole};
use clawdius_core::tools::shell::{ShellTool, ShellParams};
use clawdius_core::config::ShellSandboxConfig;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> clawdius_core::Result<()> {
    let config = Config::load_default()?;

    let llm_config = LlmConfig::from_config(&config.llm, "anthropic")?;
    let provider = create_provider(&llm_config)?;

    let messages = vec![ChatMessage {
        role: ChatRole::User,
        content: "Hello, Clawdius!".into(),
    }];

    let response = provider.chat(messages).await?;
    println!("Response: {}", response);

    Ok(())
}
```

## Core Modules

### Configuration (`config`)

```rust
use clawdius_core::Config;

let config = Config::load_default()?;
let config = Config::load(Path::new("custom.toml"))?;
let config = Config::load_or_default();
config.save(Path::new("output.toml"))?;
```

### Sessions (`session`)

```rust
use clawdius_core::SessionManager;

let manager = SessionManager::new(&config)?;
let mut session = manager.get_or_create_active()?;
manager.add_message(&mut session, msg).await?;
```

### LLM (`llm`)

```rust
use clawdius_core::llm::{create_provider, create_provider_with_retry, LlmConfig};

let config = LlmConfig::from_env("anthropic")?;
let provider = create_provider(&config)?;

let with_retry = create_provider_with_retry(&config, None)?;
```

### Context (`context`)

```rust
use clawdius_core::MentionResolver;

let resolver = MentionResolver::new(project_dir);
let items = resolver.resolve_all("@src/main.rs explain this").await?;
```

### Timeline (`timeline`)

```rust
use clawdius_core::timeline::TimelineManager;

let manager = TimelineManager::new(PathBuf::from("./project"))?;
let checkpoint = manager.create_checkpoint("before-refactor", None, None)?;
let history = manager.get_file_history("src/main.rs", Some(20))?;
```

### Completions (`completions`)

```rust
use clawdius_core::completions::{CompletionHandler, CompletionConfig, CompletionRequest};

let mut handler = CompletionHandler::new(CompletionConfig::default());
let result = handler.complete(CompletionRequest {
    code: "fn main() { let x = ".into(),
    language: "rust".into(),
    cursor_position: 26,
    file_path: None,
    context: None,
})?;
```

## Feature Flags

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `keyring` | System keyring for API keys | `keyring` |
| `vector-db` | LanceDB vector search | `lancedb`, `arrow` |
| `embeddings` | Local ML embeddings | `tokenizers`, `hf-hub` |
| `local-llm` | Local LLM inference | `candle-core`, `candle-nn` |
| `browser` | Browser automation | `chromiumoxide` |
| `crash-reporting` | Sentry crash reports | `sentry` |
| `postgres` | PostgreSQL sessions | `tokio-postgres` |
| `mariadb` | MariaDB sessions | `mysql_async` |
| `redis-queue` | Redis orchestrator queue | `redis` |

## Error Handling

```rust
use clawdius_core::Error;

match result {
    Ok(data) => { /* ... */ },
    Err(Error::RateLimited { retry_after_ms }) => {
        println!("Wait {}ms", retry_after_ms);
    },
    Err(Error::ContextLimit { current, limit }) => {
        println!("Context full: {}/{}", current, limit);
    },
    Err(e) => eprintln!("Error: {}", e.user_message()),
}
```

Check retryability:

```rust
if error.is_retryable() {
    if let Some(ms) = error.retry_after_ms() {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
}
```

## Docs

API documentation is available at [docs.rs/clawdius-core](https://docs.rs/clawdius-core).
