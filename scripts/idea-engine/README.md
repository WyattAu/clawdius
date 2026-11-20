# Idea Engine (clawdius harvest loop)

Drives the `idea-loop` pipeline (`clawdius pipeline idea-loop "<goal>"`):
a deterministic `SourceScan` gate runs [`idea_digest.py`](./idea_digest.py),
then LLM stages triage the digest against the pattern rubric, verify
load-bearing claims against dated sources, and append scored cards to
[`IDEAS-LEDGER.md`](./IDEAS-LEDGER.md).

## Layout

- `idea_digest.py` — stdlib-only scraper: RSS + HN Algolia feeds, five
  keyword buckets, dedup store (`.seen.json`), dated digests in
  `digests/`. Run manually with `python3 scripts/idea-engine/idea_digest.py --days 7`.
- `PLAYBOOK.md` — the pattern spec, source map, and the rules the loop
  must follow.
- `IDEAS-LEDGER.md` — the durable card index (committed state).

Ephemeral state (`digests/`, `.seen.json`, `__pycache__/`) is
gitignored; the ledger is not.

## Provenance

Vendored 2026-11-20 from the WyattAu dev tree (`reports/idea-engine/`),
where the canonical copy and the first harvest batch live. Update both
copies together until one is retired.
