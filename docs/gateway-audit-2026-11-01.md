# Gateway Seamlessness Audit: Telegram + Multi-Platform

**Date:** 2026-11-01
**Scope:** `crates/clawdius-gateway` (10,317 LOC, 9 platform adapters + mock)
vs the robustness bar set by OpenClaw's 23 hardened channel adapters.

## Verdict

The basics are seamless: every platform adapter translates to one unified
message type, streaming edits exist, the gateway fans out with per-adapter
health. But under failure the adapters are *delicate*: fixed backoffs, no
stall recovery, no crash-safe ingestion offsets, one callback race that can
silently drop messages, and inline dispatch that lets one slow chat starve
all others. Each finding below cites the file and the fix.

## What already works

| Behavior | Evidence |
|---|---|
| Long-polling (35s, 100-update batches) | `adapters/telegram.rs:161-165` |
| 429 honored on the poll loop | `adapters/telegram.rs:176-180` (`RequestError::RetryAfter`) |
| Graceful shutdown via `Notify` | `adapters/telegram.rs:156-160,251-256` |
| Token verification at start | `adapters/telegram.rs:223-230` (`get_me`) |
| Streaming edits per platform limits | `adapter.rs:211-222` (`max_message_length`) |
| Shared rate limiter | `rate_limit.rs` (389 LOC) |
| Per-adapter health counters | `adapter.rs:392-420` (`AdapterHealth`) |

## Gaps, ranked by seamlessness impact

### 1. Message loss window at startup (callback race) — `telegram.rs:210-216, 234-235`
`set_message_callback` sets the callback inside a *spawned* tokio task, and
`start()` works around the race with a 50ms sleep before cloning it. If the
spawn lands after the clone, the entire polling session runs with
`callback = None`: every incoming message hits the
"no callback is set" warn path and is dropped. Fix: store the callback in a
`std::sync::Mutex` (set synchronously, no spawn), drop the sleep.

### 2. Duplicate/stale replies after restart (no persisted offset) — `telegram.rs:153, 170`
`last_update_id` starts at 0 every process start, so Telegram re-delivers
its queued updates: the bot re-answers old messages. Fix: persist the
confirmed offset to `.clawdius/gateway/telegram-offset-<bot>.json` (atomic
write) and confirm it after processing, mirroring OpenClaw's
`update-offset-store.ts` (versioned, bot-id bound).

### 3. One slow chat starves all chats (inline dispatch) — `telegram.rs:172`
`dispatch_with(...).await` runs the gateway handler *inside* the polling
loop. An LLM turn takes seconds; while it runs, no `get_updates` happens,
Telegram queues up, and other chats wait. Fix: spawn per-message delivery
with a bounded semaphore (lane model), keep the poll loop hot.

### 4. Tight retry loops under platform outages — `telegram.rs:181-190`
Network errors retry after a fixed 2s, everything else after a fixed 5s, with
no jitter and no cap. A Telegram-side outage produces a hot loop (and 409
conflicts if a second instance polls). Fix: shared exponential backoff with
jitter (2s to 30s, factor 1.8, jitter 0.25) plus a stall watchdog that
force-recycles a hung poller (OpenClaw's `polling-session.ts` +
`stall-watchdog.ts` pattern).

### 5. MarkdownV2 send failures are terminal — `telegram.rs:261`
User text rarely escapes cleanly to MarkdownV2; a parse error fails the send
with no plain-text fallback. Fix: on `parse`-class errors, resend with no
parse mode.

### 6. Media silently dropped — `telegram.rs:84-89`
Photos/voice arrive with empty text and no caption; `convert_update` returns
`None` and the message vanishes. Attachments are always `Vec::new()`.
Fix: map Telegram media to the existing `Attachment` type (the struct is
already there, `adapter.rs:144-158`) and download via
`download_attachment`.

### 7. Same patterns across the other 8 adapters
Discord/Slack/Matrix/Teams/WhatsApp/Signal/RocketChat/Webhook each carry
platform-specific variants of gaps 3-6. The shared fix is a `gateway-core`
module with: `backoff.rs` (one retry/backoff primitive used by all
adapters), `offset_store.rs` (atomic, versioned ingestion offsets),
`lanes.rs` (per-chat delivery lanes), `watchdog.rs` (stall detection). Then
each adapter shrinks to format translation + platform API calls.

## Platform coverage note

vs the competitive field (see `competitor-analysis-2026-11-01.md`):
clawdius covers 9 platforms natively against picoclaw's 17+ and OpenClaw's
23. Missing if the roadmap needs them: WeCom, DingTalk, Feishu/Lark, QQ,
IRC, Line, Twitch, Nostr, Google Chat, iMessage. The adapter trait makes
each a contained addition.
