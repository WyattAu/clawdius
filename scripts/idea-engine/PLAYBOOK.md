# Idea Engine — Playbook

**Purpose:** continuously surface business ideas matching the verified Model-1 signature (structural unlock + skill fit + capital-light + no customer cold-start), instead of finding them one brainstorm at a time.

**Last seeded:** 2026-11-20 · **Owner:** you · **Cadence:** weekly, ~20 min

---

## 1. The pattern spec

Every keeper idea must match most of this signature. Score 0–2 per dimension; **promote ≥ 8/12, park ≤ 6/12**.

| Dimension | 2 points | 0 points |
|---|---|---|
| **Day-1 unit economics** | Market pays skill directly; no customer needed | Requires signed customers before first euro |
| **Unlock freshness** | Structural access/regulatory/venue change < 24 months old, verified by dated source | "Big market" only, no recent change |
| **Competition softness** | Incumbents trade as an operational byproduct (utility desks, asset managers) | Dedicated prop/HFT firms already there |
| **Skill fit** | MFT execution, stochastic control, Rust stack is the moat | Edge unrelated to your stack |
| **Capital & regulatory friction** | ≤ €300k, no license or registration-only (REMIT-style notification) | Institutional license, or > €1M / 12 months to first trade |
| **Optionality** | Produces track record / data / product evidence that unlocks other models | Standalone, opens only its own door |

A **watchlist** is a valid status: instrument not live yet, but a dated trigger exists (e.g., "first registry launches hourly GO trading").

## 2. Source map (what to scrape/check weekly)

| Group | Sources | Signal |
|---|---|---|
| **Venue/product launches** | epexspot.com/en/news · nordpoolgroup.com · eex.com news · nasdaqcommodities.com · euronext.com derivatives fee guides · ICE Endex circulars · fidesc/predict: FIA market structure news | New instruments, LP/MM programs with stipends (stipend = exchange subsidizing early entrants = your signal) |
| **Regulators** | acer.europa.eu/news · ESMA consultations · ACM.nl nieuws · afm.nl nieuws · EUR-Lex OJ L-series | Rule changes that move barriers; REMIT/MiCA/DORA implementation noise |
| **TSO/grid** | tennet.eu newsroom · energystoragenl.nl · Flexcon conference program · JAO auctions | Physical-market structure changes (TDTR, GOPACS, balancing product redesigns) |
| **Trade press** | energy-storage.news · pv-magazine · carbon-pulse (paid) · montelnews (paid, optional) | Deployments = future counterparties and flow |
| **Research/blogs** | modoenergy.com/research · energytag.org · ember-climate.org · arXiv q-fin.TR · hyperliquidresearch.xyz · 0xarchive.io | Benchmarks (like Modo's €102k/MW/yr) and venue-decay data (like the HL long-tail study) |
| **HN Algolia** | hn.algolia.com api, queries: prediction market / market making / electricity trading / power exchange | Early-stage venues and tools |
| **Conferences** | FLEXCON, Energy Storage Global Conference, OMC/GO Symposium, Commodity Markets Week | Programs are future-roadmap leaks (EEX told us granular GO plans there) |

The `idea_digest.py` script automates the RSS/JSON slice of this; venue fee-guide PDFs and regulator pages stay manual (monthly).

## 3. Weekly workflow

1. `python3 scripts/idea-engine/idea_digest.py --days 7` (or ask any agent session to run it and summarize the digest).
2. Skim `digests/YYYY-MM-DD.md` (~10 min). Anything matching the §1 pattern → open a card.
3. New card = one block in `IDEAS-LEDGER.md`: pattern-fit scores, capital estimate, **cheapest falsifying test**, status (`research | watchlist | pilot | live | dead`).
4. Monthly: re-score open cards. A card whose unlock is >24 months old without you acting on it → `dead` (window closed).
5. Quarterly: promote the highest-scoring card to a full Model-N report (see `reports/model1-eu-intraday-power-trading.md` as the template: verdict → verified facts → economics → risk register with kill gates → phased plan → sources).

## 4. Standing agent prompt

Paste this into any future session to run the engine:

> Read `scripts/idea-engine/PLAYBOOK.md` and `IDEAS-LEDGER.md`. Run `idea_digest.py --days 7`, read the new digest, and for each item that scores ≥8/12 on the playbook rubric, append a card to the ledger with scores, capital estimate, cheapest falsifying test, and status. Report only the cards you added (≤15 lines) plus ledger deltas. Do not re-litigate dead cards.

## 5. Rules that keep this honest

- **No idea enters the ledger without a dated source or a stated [U] flag.** The Hyperliquid long-tail idea looked great in prose and was negative-EV in Nov-2026 data — verification is the product.
- **Stipends are signals.** When an exchange pays you to quote (Euronext LP: €10k/month), it is explicitly advertising an immature order book. Immature order book = the Model-1 pattern in a different wrapper.
- **B2B cards must name the distribution channel** (partner, inbound trigger, or anchor customer) or they inherit the cold-start disease the original 8-model list died of.
- Kill gates go in the card *before* any money moves — same discipline as Model 1's §7.
