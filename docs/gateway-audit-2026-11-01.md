# Gateway Seamlessness Audit: Telegram + Multi-Platform

**Date:** 2026-11-01
**Scope:** `crates/clawdius-gateway` (10,317 LOC, 9 platform adapters + mock)
vs the robustness bar set by OpenClaw's 23 hardened channel adapters.

## Verdict

The basics are seamless: every platform adapter translates to one unified
message type, streaming edits exist, the gateway fans out with per-adapter
health. The Telegram adapter has since been hardened (see "Status" per
finding); the other 8 adapters still carry platform-specific variants of
findings 3-6. Each finding cites the file and the fix.

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

Status legend: FIXED (this audit cycle), OPEN (still to do).

### 1. Message loss window at startup (callback race) - FIXED
Was `telegram.rs`: `set_message_callback` set the callback inside a spawned
tokio task and `start()` papered over the race with a 50ms sleep; if the
spawn landed after the clone, the whole polling session ran with no
callback and dropped every message. Now: a synchronous `std::sync::Mutex`
with the sleep removed.

### 2. Duplicate/stale replies after restart (no persisted offset) - FIXED
Was `telegram.rs`: `last_update_id` restarted at 0, so Telegram re-delivered
its queued updates and the bot re-answered old messages. Now: the confirmed
offset persists to `.clawdius/gateway/telegram-offset.json` (atomic write,
versioned envelope) after every batch and at shutdown, overridable via
`settings["offset_path"]`; shared primitive in `src/robust.rs`
(`load_offset`/`store_offset`).

### 3. One slow chat starves all chats (inline dispatch) - FIXED
Was `telegram.rs`: the gateway handler ran inline inside the polling loop.
Now each inbound message is delivered on a spawned task bounded by a
64-permit semaphore, keeping the poll loop hot while LLM turns drain.

### 4. Tight retry loops under platform outages - FIXED
Was `telegram.rs`: fixed 2s/5s sleeps, no jitter, no cap. Now a shared
`robust::Backoff` (2s to 30s, factor 1.8, jitter 0.25, reset on success).
Still OPEN: a stall watchdog that force-recycles a hung poller.

### 5. MarkdownV2 send failures are terminal - FIXED
Was `telegram.rs`: a parse error failed the send outright. Now the send
falls back to plain text on the first failure (logged).

### 6. Media silently dropped - OPEN
Photos/voice arrive with empty text and no caption; `convert_update` returns
`None` and the message vanishes. Attachments are always `Vec::new()`.
Fix: map Telegram media to the existing `Attachment` type (the struct is
already there, `adapter.rs:144-158`) and download via
`download_attachment`.

### 7. `edit_message` edits a nonexistent chat - OPEN (new finding)
`adapters/telegram.rs` `edit_message` calls
`bot.edit_message_text(ChatId(0), MessageId(mid), ...)`; Telegram requires
the real chat id, so streaming edits via this path fail with message-not-
found. The `PlatformAdapter::edit_message` trait only passes `message_id`,
so the chat id must be threaded through (a trait change across adapters) or
the adapter must remember the chat per message id.

### 8. Same patterns across the other 8 adapters - OPEN
Discord/Slack/Matrix/Teams/WhatsApp/Signal/RocketChat/Webhook each carry
platform-specific variants of findings 3-6. The shared fix is now
available in `src/robust.rs` (`Backoff`, offset store); adopt it per
adapter, then add `lanes.rs` (per-chat delivery lanes) and a stall
watchdog.

## Platform coverage note

vs the competitive field (see `competitor-analysis-2026-11-01.md`):
clawdius covers 9 platforms natively against picoclaw's 17+ and OpenClaw's
23. Missing if the roadmap needs them: WeCom, DingTalk, Feishu/Lark, QQ,
IRC, Line, Twitch, Nostr, Google Chat, iMessage. The adapter trait makes
each a contained addition.
