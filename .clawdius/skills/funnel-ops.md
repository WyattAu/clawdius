# funnel-ops

Operate the Kingston Peptides / Homebite outreach funnel conversationally — the SLA board, the fulfillment queue, the reply drafts, the boarding, the canary.

## When to use

- "who's waiting?" — the SLA board (unanswered leads, oldest first)
- "what's in fulfillment?" — accepted-seeding leads awaiting shipment
- "draft the reply for <lead>" — the WhatsApp-referral draft for that lead
- "did anyone reply?" — the funnel reply sweep (both funnels)
- "how's the canary?" — the per-send open-rate board
- "board this contact" — add an agent/operator/supplier row with provenance
- "walk the planning applications" — the paced browser walk (30s+ spacing)

## The ops

All funnel ops live in `/home/wyatt/creator-scraper/ops/` on the server
(`wyatt@192.168.1.191`). Run via `ssh wyatt@192.168.1.191` + python3:

- **SLA board + canary**: `python3 /home/wyatt/creator-scraper/digest.py --dry-run`
- **Fulfillment queue**: `python3 /home/wyatt/creator-scraper/ops/seeding_queue.py`
- **Reply drafts**: `python3 /home/wyatt/creator-scraper/ops/reply_draft.py <email>` (or `--all`)
- **Mark answered**: `python3 /home/wyatt/creator-scraper/ops/mark_answered.py <email>`
- **Per-send canary**: `python3 /home/wyatt/creator-scraper/ops/canary.py 7`
- **Boarding**: `INSERT INTO creators ... 'operator', 40, 'discovered', 'planning:<ref> | ...'` (homebite) or the trade stamp (outreach)
- **Seeding stages**: `python3 /home/wyatt/creator-scraper/ops/seeding_queue.py --stamp <email> <stage>`
- **Reply sweep (read inboxes)**: `/tmp/readargs.py` pattern (IMAP migadu, sales@)

## The rules

- **Never auto-send without approval** — drafts first, send second
- **The SLA board order**: oldest unanswered first; link-offers and competitors excluded
- **D12**: national operators refuse wave-1 qualification, always
- **Stop-requests auto-suppress** (the classifier's UNSUB_PAT) — never auto-replied
- **Provenance on every board**: planning:<ref> / hunter:<domain> / permute:<domain> / user-provided
- **The funnel's currencies**: per-send opens (72h attributed), replies, acceptances, boxes shipped

## The leads that matter

Cory (box ready), Leila (Glow + NAD+), Berit (wholesale price list), Sarah,
Cici, Vaibhav, Corner Flat, Frith, Evolution Training, Abiola, permatan,
Nitrogym — plus the 12 trade contacts and the homebite buildings
(Base Glasgow, Platform.life, Rettie, Maddox, George).
