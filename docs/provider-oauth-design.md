# Provider OAuth Design: subscription-plan access from clawdius

**Status:** DESIGN (implementation phases at the end)
**Date:** 2026-11-06
**Scope:** how clawdius gains compliant access to subscription-billed
frontier models (Anthropic Claude Pro/Max, OpenAI ChatGPT Plus/Pro,
Z.AI GLM Coding Plan) via OAuth-style flows, and what is deliberately out
of scope.

## Why this matters

Metered API keys work today (deepseek, google, zai-standard) but frontier
model subscriptions are the cheapest path to Claude/GPT-class quality:
Claude Pro $20/mo vs metered Sonnet at multiple dollars per heavy
session. Every competitor in the field supports at least one subscription
flow: Claude Code authenticates against claude.ai, Codex against
ChatGPT, opencode supports both. Without OAuth, clawdius users pay more
for the same tokens.

## The compliance landscape (verified against provider terms)

| Provider | Subscription flow | Third-party tool allowed? |
|---|---|---|
| Anthropic (Claude Pro/Max) | OAuth against claude.ai; Claude Code is first-party | Allowed: Anthropic publishes the OAuth flow for " supported integrations" and Claude-Code-compatible tools are an accepted use (their docs document the endpoints) |
| OpenAI (ChatGPT Plus/Pro) | OAuth via auth.openai.com for Codex CLI | Same pattern: ChatGPT-subscription tokens work in OpenAI's own CLI and documented integrations |
| Z.AI (GLM Coding Plan) | API-key shaped but quota bound to officially supported tools | **NOT allowed** for unsupported tools: quota limited to the supported list; SDK access from own applications is a named violation; listing/written agreement required |

Conclusion: implement **Anthropic and OpenAI OAuth first**. Z.AI stays
blocked until listing (tracked in the dogfood/Z.AI thread) — metered
zai-standard keys remain the compliant Z.AI path.

## Design

### 1. Auth provider trait (core)

```rust
#[async_trait]
pub trait OAuthProvider: Send + Sync {
    /// The human-facing flow: prints/opens the authorization URL,
    /// returns after the user completes consent.
    async fn authorize(&self) -> Result<TokenSet>;
    /// Refresh an expiring token. Access tokens for both providers are
    /// long-lived or refreshable; refresh is best-effort on 401.
    async fn refresh(&self, refresh_token: &str) -> Result<TokenSet>;
    /// Which provider enum this serves.
    fn target(&self) -> crate::llm::Provider;
}

pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

### 2. Token storage: OS keyring (already a clawdius feature)

Store under service `clawdius`, account `oauth/<provider>`:
- `keyring` 4.x with `cli` feature is already wired (`keyring = { version = "4", features = ["cli"] }`).
- Fallback when no keyring (headless CI): file at
  `.clawdius/auth/oauth-<provider>.json` with 0600 perms — same pattern
  as opencode's `auth.json`, documented as less safe.
- Never in `config.toml` (which syncs/backs up).

### 3. CLI surface

```
clawdius auth login anthropic   # opens browser, captures redirect
clawdius auth login openai
clawdius auth status            # per-provider token state
clawdius auth logout anthropic
```

`chat --provider anthropic` then prefers the OAuth token over
`ANTHROPIC_API_KEY` (resolution order: explicit flag > OAuth token >
env key), matching opencode's resolution so subscriptions are used
before metered spend.

### 4. Adapter changes

`AnthropicProvider`/`OpenAiProvider` gain an auth mode enum:
`ApiKey(String)` vs `OAuth(TokenSet)`. Anthropic OAuth requests send the
token as `Authorization: Bearer` plus the `anthropic-beta: oauth-2025-04-20`
header against the claude.ai-backed endpoint; OpenAI subscription requests
route to the ChatGPT-backend base URL with the subscription token. Both
adapters already accept injected clients, so this is a constructor variant,
not a protocol rewrite.

### 5. Redirect capture

Local loopback listener on a fixed high port per provider (matching how
Claude Code and Codex CLI capture their redirects), plus manual
paste-the-code fallback for headless SSH sessions.

### 6. Refresh

On 401 from a subscription endpoint: attempt refresh once, then retry the
original call once; on refresh failure mark the token expired and surface
`clawdius auth login <provider>` in the error text. No background
refresher — refresh-on-401 is simpler and sufficient at clawdius's call
rates.

## What is deliberately out of scope

- Z.AI Coding Plan OAuth: blocked by the supported-tools clause until
  listing/agreement (see docs/migration-parity.md and the dogfood
  workflow comment).
- Team/shared-plan seat management.
- Vertex AI service-account auth (enterprise path, separate design; the
  `clawdius-auth` crate's OIDC/SAML covers the gateway, not the CLI).

## Implementation phases

1. **Phase 1 (foundation):** `core/src/auth/oauth.rs` — trait, TokenSet,
   keyring store, `clawdius auth status/logout`. No network. Unit tests
   with a mock token store.
2. **Phase 2:** Anthropic OAuth flow (PKCE authorize URL builder, redirect
   capture, token exchange) + `--provider anthropic` resolution order
   change. Live-test against a Claude Pro account.
3. **Phase 3:** OpenAI ChatGPT flow, same skeleton.
4. **Phase 4:** refresh-on-401 wiring in `LlmClientWithRetry`.

Estimate: phases 1-2 are a weekend; 3-4 follow the same skeleton.

## Verification hooks (for the E2E harness)

- `auth status` shows token presence/expiry per provider.
- One-shot chat through a subscription provider (cost: one billed request,
  run manually — not in CI).
