# Clawdius v1.2.0 Demo Video Script

## Video Overview

- **Duration:** 3-4 minutes
- **Style:** Screen recording with voiceover
- **Resolution:** 1920x1080 (1080p)
- **Frame Rate:** 30fps

---

## Scene 1: Introduction (0:00 - 0:30)

### Visual
- Terminal window, dark theme
- Clear desktop background

### Script
"Meet Clawdius - the high-assurance AI coding assistant built in Rust.

Unlike other AI coding tools that run on heavy Node.js runtimes, Clawdius is a native binary that boots in under 20 milliseconds.

It combines the power of large language models with formal verification, secure sandboxing, and enterprise-grade features."

### Actions
```bash
clawdius --version
# Output: clawdius 1.2.0
```

---

## Scene 2: Interactive Setup (0:30 - 1:15)

### Visual
- Terminal window
- Run `clawdius setup`

### Script
"New in version 1.2.0 is our interactive setup wizard.

Just run `clawdius setup` and the wizard guides you through configuration."

### Actions
```bash
clawdius setup
```

**Show the wizard:**
1. Welcome screen → Press Enter
2. Provider selection → Select "Anthropic"
3. API key entry → Enter key (masked)
4. Preset selection → Select "Balanced"
5. Connectivity check → Show success
6. Quick start examples displayed

### Script
"Choose your LLM provider, enter your API key securely using your system keyring, and select a preset that matches your workflow.

The wizard even checks if Ollama is running for local LLMs."

---

## Scene 3: Interactive Chat (1:15 - 2:00)

### Visual
- Terminal window
- Run `clawdius chat`

### Script
"Now let's start a conversation.

Clawdius supports multiple providers - Anthropic, OpenAI, Ollama for local LLMs, and Zhipu AI.

Let's ask it to explain a concept."

### Actions
```bash
clawdius chat
```

**In chat:**
```
You: Explain Rust's ownership model in one paragraph
```

**Show response streaming in real-time**

### Script
"Responses stream in real-time, giving you immediate feedback.

Notice the token count and timing displayed at the end - helpful for monitoring API usage."

---

## Scene 4: Code Generation (2:00 - 2:45)

### Visual
- New terminal window
- Run `clawdius generate`

### Script
"Clawdius can generate code in three modes: single-pass, iterative refinement, or agent-based.

Let's generate a REST API endpoint."

### Actions
```bash
clawdius generate --mode agent "Create a Rust function that validates email addresses using regex"
```

**Show streaming generation**

### Script
"The agent mode autonomously plans, generates, and verifies the code.

You can see the code being generated in real-time, with proper error handling and documentation."

---

## Scene 5: Local LLMs (2:45 - 3:15)

### Visual
- Terminal window
- Show Ollama running

### Script
"For complete privacy, Clawdius works with local LLMs via Ollama.

Your code and API keys never leave your machine."

### Actions
```bash
# Show Ollama is running
ollama list

# Use local LLM
clawdius chat --provider ollama --model llama3
```

**In chat:**
```
You: What are the benefits of Rust for systems programming?
```

### Script
"Zero latency to external APIs, complete privacy, and no per-token costs."

---

## Scene 6: Security & Verification (3:15 - 3:45)

### Visual
- Show architecture diagram (optional)
- Terminal showing security features

### Script
"Clawdius was built with security as a primary concern.

Seven sandbox backends - from WASM to Firecracker microVMs.

104 formal verification proofs in Lean4.

And zero vulnerabilities in our latest security audit."

### Actions
```bash
# Show security audit
cargo audit
# Output: 0 vulnerabilities found

# Show sandbox tiers
clawdius sandbox list
```

---

## Scene 7: Call to Action (3:45 - 4:00)

### Visual
- GitHub repository page
- Installation instructions

### Script
"Clawdius is open source and available today.

Install with `cargo install clawdius` or download from GitHub.

Star us on GitHub, join our Discord, and let us know what you build."

### Actions
```bash
# Show install command
cargo install clawdius

# Show GitHub URL
# github.com/WyattAu/clawdius
```

### Text Overlay
```
🦀 clawdius
github.com/WyattAu/clawdius
docs.clawdius.dev
```

---

## Recording Tips

### Equipment
- **OS:** Linux or macOS (clean desktop)
- **Terminal:** Alacritty, iTerm2, or Windows Terminal
- **Font:** JetBrains Mono or Fira Code (14-16pt)
- **Theme:** Dark theme (matches Clawdius branding)

### Recording Software
- **Linux:** OBS Studio, Peek
- **macOS:** Screen Studio, CleanShot X
- **Windows:** OBS Studio, ShareX

### Audio
- **Microphone:** Quality USB mic or headset
- **Environment:** Quiet room, minimal echo
- **Post-processing:** Light noise reduction if needed

### Editing
- **Cut:** Remove typos, long pauses
- **Zoom:** Highlight important sections
- **Captions:** Add key points as text overlays
- **Music:** Optional subtle background music

---

## Thumbnail Ideas

1. **Split screen:** Rust logo + AI brain icon
2. **Terminal screenshot:** `clawdius setup` wizard
3. **Performance graph:** <20ms boot time vs competitors
4. **Security badges:** 7 sandboxes, 104 proofs, 0 CVEs

---

## SEO Tags

```
Clawdius, Rust, AI coding assistant, LLM, Anthropic, OpenAI, Ollama, 
formal verification, Lean4, sandboxing, security, code generation, 
developer tools, open source, 2026
```

---

## Distribution

### YouTube
- Title: "Clawdius v1.2.0 - High-Assurance AI Coding Assistant in Rust"
- Description: Link to GitHub, docs, blog post
- Tags: Rust, AI, LLM, developer tools

### Twitter/X
- Clip key moments as short videos (30-60s each)
- Thread with video embeds

### LinkedIn
- Full video or highlights
- Professional framing

### Embed
- Add to README.md
- Add to docs.clawdius.dev homepage
