# Agent Modes

Clawdius supports custom agent modes that allow the AI assistant to behave differently based on the task at hand.

## Overview

Agent modes customize:
- **System prompt**: The instructions given to the AI
- **Temperature**: Controls randomness/creativity (0.0-1.0)
- **Tools**: Which tools are available (file, shell, git)

## Built-in Modes

### Code Mode (default)
Focus on code generation and editing.
- **Temperature**: 0.7
- **Tools**: file, shell, git
- **Use for**: Writing code, debugging, implementing features

### Architect Mode
Focus on design and structure planning.
- **Temperature**: 0.5
- **Tools**: file, git
- **Use for**: System design, API planning, migrations

### Ask Mode
Quick answers and explanations.
- **Temperature**: 0.8
- **Tools**: none
- **Use for**: Questions, documentation, explanations

### Debug Mode
Troubleshooting and diagnostics.
- **Temperature**: 0.6
- **Tools**: file, shell, git
- **Use for**: Finding bugs, analyzing errors, root cause analysis

### Review Mode
Code review and analysis.
- **Temperature**: 0.5
- **Tools**: file, git
- **Use for**: Code reviews, best practices, security analysis

### Refactor Mode
Code improvement and refactoring.
- **Temperature**: 0.6
- **Tools**: file, shell, git
- **Use for**: Improving code structure, reducing complexity

### Test Mode
Test generation.
- **Temperature**: 0.7
- **Tools**: file, shell
- **Use for**: Writing unit tests, integration tests

## CLI Usage

### List Available Modes
```bash
clawdius modes list
```

### Use a Specific Mode
```bash
# Use architect mode
clawdius chat --mode architect "Design a REST API for user management"

# Use debug mode
clawdius chat --mode debug "Why is my code throwing a null pointer exception?"

# Use review mode
clawdius chat --mode review @src/main.rs "Review this code for best practices"
```

### Show Mode Details
```bash
clawdius modes show code
```

### Create Custom Mode
```bash
clawdius modes create my-custom-mode
```

This creates a template in `.clawdius/modes/my-custom-mode.toml` that you can customize.

## TUI Usage

### Switch Modes
In the TUI, use command mode:
1. Press `:` to enter command mode
2. Type `:mode <mode-name>` (e.g., `:mode architect`)
3. Press Enter

### List Available Modes
```
:modes
```

### Status Bar
The status bar shows the current mode:
```
Clawdius | Mode: code | anthropic / claude-3-5-sonnet | Tokens: 1234
```

## Custom Modes

### Creating a Custom Mode

1. Create a TOML file in `.clawdius/modes/<name>.toml`:

```toml
name = "security-review"
description = "Security-focused code review"
system_prompt = """
You are Clawdius, a security specialist. You help with:
- Identifying security vulnerabilities
- Checking for common attack vectors
- Reviewing authentication and authorization
- Ensuring secure data handling

Be thorough and paranoid. Every line of code could be an attack vector.
"""
temperature = 0.4
tools = ["file", "git"]
```

2. Use the custom mode:
```bash
clawdius chat --mode security-review @src/auth.rs
```

### Mode Configuration Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Mode name (used with --mode flag) |
| `description` | string | No | Brief description shown in mode list |
| `system_prompt` | string | Yes | Instructions for the AI |
| `temperature` | float | No | Randomness (0.0-1.0, default: 0.7) |
| `tools` | array | No | Available tools: ["file", "shell", "git"] |

## Mode-Specific Behaviors

### Code Mode
- Higher temperature for creative solutions
- All tools enabled for maximum flexibility
- Focus on implementation details

### Architect Mode
- Lower temperature for consistency
- Planning-focused prompts
- Considers long-term maintainability

### Debug Mode
- Analytical and methodical approach
- Diagnostic tools enabled
- Step-by-step troubleshooting

### Review Mode
- Critical analysis focus
- Suggestions only (no direct edits)
- Best practices emphasis

### Refactor Mode
- Preservation focus
- Tests required before changes
- Incremental, safe modifications

### Test Mode
- Test generation specialist
- Coverage-focused
- Edge case consideration

## Best Practices

### Choosing a Mode
- **Code**: Default for most coding tasks
- **Architect**: When planning or designing
- **Debug**: When something's broken
- **Review**: When checking others' code
- **Refactor**: When improving existing code
- **Test**: When writing tests

### Creating Effective Custom Modes
1. **Be specific**: Clear, focused purpose
2. **Set appropriate temperature**: Lower for consistency, higher for creativity
3. **Limit tools**: Only enable what's needed
4. **Provide context**: Include examples in system prompt

### Temperature Guidelines
- **0.0-0.3**: Deterministic, consistent outputs
- **0.4-0.6**: Balanced, focused responses
- **0.7-0.9**: Creative, varied outputs
- **1.0**: Maximum randomness

## Examples

### Security Review Mode
```toml
name = "security-review"
description = "Security-focused code review"
system_prompt = """
You are a security specialist reviewing code for vulnerabilities.
Focus on: SQL injection, XSS, CSRF, authentication flaws, secrets in code.
Be paranoid and thorough.
"""
temperature = 0.3
tools = ["file", "git"]
```

### Documentation Mode
```toml
name = "docs"
description = "Technical documentation writer"
system_prompt = """
You are a technical writer specializing in developer documentation.
Create clear, concise, well-structured documentation.
Include code examples, usage patterns, and API references.
"""
temperature = 0.5
tools = ["file"]
```

### Performance Mode
```toml
name = "performance"
description = "Performance optimization specialist"
system_prompt = """
You are a performance optimization expert.
Analyze code for bottlenecks, memory leaks, and inefficiencies.
Suggest optimizations with benchmarks when possible.
"""
temperature = 0.6
tools = ["file", "shell"]
```

## Implementation Details

### Mode Loading Order
1. Check built-in modes (code, architect, ask, debug, review, refactor, test)
2. Check custom modes in `.clawdius/modes/*.toml`
3. Return error if mode not found

### Mode Application
1. System prompt is injected as the first message
2. Temperature is applied to LLM configuration
3. Tool availability is enforced (future feature)

### Mode Storage
- Built-in modes: Hardcoded in `clawdius-core/src/modes.rs`
- Custom modes: `.clawdius/modes/<name>.toml`

## Future Enhancements

- [ ] Tool availability enforcement
- [ ] Mode-specific keyboard shortcuts in TUI
- [ ] Mode templates for common use cases
- [ ] Mode sharing via GitHub
- [ ] Mode inheritance/composition
- [ ] Mode-specific model selection
- [ ] Mode profiles (collections of modes)
