# Clawdius JetBrains Plugin

Clawdius plugin for JetBrains IDEs (IntelliJ Platform) providing AI-powered code assistance.

## Features

- **AI Chat Interface**: Interactive chat window for AI assistance
- **Code Completion**: AI-powered code completion
- **Code Explanation**: Explain selected code
- **Refactoring**: AI-assisted code refactoring
- **Test Generation**: Generate unit tests for code
- **Diagnostics**: Real-time code analysis
- **Multi-Provider Support**: Works with Anthropic, OpenAI, DeepSeek, Ollama, etc- **Formal Verification**: Integration with Lean4 proofs
- **Multi-Language Support**: Works with Rust, Python, JavaScript, TypeScript, Go, Java, etc.

## Installation

1. Build the plugin:
   ```bash
   cd plugins/jetbrains/clawdius-plugin
   ./gradlew buildPlugin
   ```

2. Install manually:
   - Go to Settings > Plugins > Install Plugin from Disk
   - Select the built plugin file

3. Configure:
   - Go to Settings > Tools > Clawdius
   - Set your server URL and API key
   - Choose your provider and model

## Usage

### Chat Window
- Open: View > Tool Windows > Clawdius
- Type your question and code
- Press Enter to send
- Shift+Enter for new line

### Code Completion
- Start typing, suggestions appear automatically
- Tab to accept suggestion

### Actions
- Right-click in editor > Clawdius > Explain
- Right-click in editor > Clawdius > Refactor
- Right-click in editor > Clawdius > Generate Tests

### Keyboard Shortcuts
- `Alt+Shift+E`: Explain code
- `Alt+Shift+R`: Refactor code
- `Alt+Shift+T`: Generate tests
- `Alt+Shift+F`: Fix issues
- `Alt+Shift+C`: Open chat

## Configuration

### Settings
| Setting | Description | Default |
|--------|-------------|---------|
| Server URL | Clawdius server URL | http://localhost:3000 |
| API Key | Your API key | (empty) |
| Provider | LLM provider | anthropic |
| Model | Model to use | (provider default) |
| Enable Auto Complete | Toggle code completion | true |
| Enable Inline Hints | Toggle inline hints | true |
| Max Tokens | Maximum tokens per request | 2048 |
| Temperature | Generation temperature | 0.7 |

### Supported Providers
- Anthropic (Claude)
- OpenAI (GPT-4, etc- DeepSeek
- Ollama (local)
- OpenRouter
- ZAI

