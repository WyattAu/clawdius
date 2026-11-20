#!/usr/bin/env python3
"""Idea digest scraper.

Pulls market-structure, energy, carbon and prediction-market feeds,
filters entries by keyword buckets, deduplicates against a state file,
and writes a dated digest to digests/YYYY-MM-DD.md.

Usage:
    python3 idea_digest.py            # default: --days 3
    python3 idea_digest.py --days 7   # wider window
    python3 idea_digest.py --all      # ignore seen-state, include everything

Stdlib only. Per-feed failures are logged in the digest, never fatal.
Run weekly (cron / systemd timer, or hand the PLAYBOOK.md prompt to any agent).
"""

import argparse
import hashlib
import json
import re
import sys
import urllib.request
import xml.etree.ElementTree as ET
from datetime import datetime, timedelta, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
STATE = HERE / ".seen.json"
OUTDIR = HERE / "digests"
UA = {"User-Agent": "idea-digest/0.1 (personal research)"}

RSS_FEEDS = {
    "energy-mkt": [
        "https://pv-magazine.com/feed/",
        "https://www.energy-storage.news/feed/",
        "https://www.energytag.org/feed/",
        "https://www.tennet.eu/nl-en/rss.xml",
        "https://www.reddit.com/r/energytrading/.rss",
    ],
    "mkt-structure": [
        "https://www.reddit.com/r/algotrading/.rss",
        "https://www.reddit.com/r/quant/.rss",
        "https://www.fow.com/rss.xml",
    ],
    "carbon": [
        "https://www.carbon-pulse.com/feed/",
    ],
    "events-crypto": [
        "https://www.reddit.com/r/PredictionMarkets/.rss",
    ],
    "research": [
        "http://export.arxiv.org/rss/q-fin.TR",
    ],
}

HN_QUERIES = {
    "events-crypto": ["prediction market", "polymarket", "kalshi"],
    "mkt-structure": ["market making", "market microstructure", "exchange launch"],
    "energy-mkt": ["electricity trading", "power exchange", "battery trading", "grid congestion"],
}

BUCKET_KEYWORDS = {
    "energy-mkt": re.compile(
        r"(intraday|imbalance|balanc|negative price|curtail|ancillary|BESS|battery|grid fee|TDTR|GOPACS"
        r"|EPEX|Nord Pool|TenneT|market coupl|capacity auction|interconnector|guarantee of origin|granular"
        r"|REMIT|aFRR|fCR|localflex|CfD|day-ahead|within-day|wind lull|storage optimi)", re.I),
    "mkt-structure": re.compile(
        r"(market mak|liquidity provid|liquidity provider|new exchange|launch|microstructure|order book"
        r"|venue|listing|stipend|rebate|market maker program|designated market maker|EPAD|system price)", re.I),
    "carbon": re.compile(
        r"(EUA|CBAM|carbon|EU ETS|MSR|emissions trading|allowance)", re.I),
    "events-crypto": re.compile(
        r"(prediction market|polymarket|kalshi|event contract|hyperliquid|orderly|CLOB|perp|binary)", re.I),
    "research": re.compile(
        r"(market microstructure|limit order book|liquidity|adverse selection|informed trad|auction)", re.I),
}

def http_get(url: str, timeout: int = 20) -> bytes:
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read()

def entry_id(link: str, title: str) -> str:
    key = link or title
    return hashlib.sha1(key.encode("utf-8", "replace")).hexdigest()[:16]

def parse_rss(url: str) -> list[dict]:
    raw = http_get(url)
    root = ET.fromstring(raw)
    items = root.iter()
    out = []
    for item in root.iter("item"):
        title = (item.findtext("title") or "").strip()
        link = (item.findtext("link") or "").strip()
        date_raw = (item.findtext("pubDate") or (item.findtext("{http://purl.org/dc/elements/1.1/}date") or "")).strip()
        date = None
        for fmt in ("%a, %d %b %Y %H:%M:%S %z", "%Y-%m-%dT%H:%M:%S%z", "%Y-%m-%dT%H:%M:%SZ"):
            try:
                date = datetime.strptime(re.sub(r"\s+", " ", date_raw), fmt)
                break
            except ValueError:
                continue
        out.append({"id": entry_id(link, title), "title": re.sub(r"<[^>]+>", "", title),
                    "link": link, "date": date, "source": url})
    return out

def hn_search(query: str, days: int) -> list[dict]:
    url = (f"https://hn.algolia.com/api/v1/search_by_date?query={urllib.request.quote(query)}"
           f"&tags=story&numericFilters=created_at_i>{int((datetime.now(timezone.utc).timestamp() - days * 86400))}&hitsPerPage=20")
    data = json.loads(http_get(url))
    out = []
    for h in data.get("hits", []):
        link = h.get("url") or f"https://news.ycombinator.com/item?id={h.get('objectID')}"
        out.append({"id": entry_id(link, h.get("title", "")), "title": h.get("title", ""),
                    "link": link, "date": None,
                    "source": f"hn:{query}"})
    return out

def load_state() -> set:
    if STATE.exists():
        try:
            return set(json.loads(STATE.read_text())["seen"])
        except Exception:
            return set()
    return set()

def save_state(seen: set) -> None:
    keep = sorted(seen)[-5000:]
    STATE.write_text(json.dumps({"updated": datetime.now(timezone.utc).isoformat(), "seen": keep}))

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--days", type=int, default=3, help="max age for dated entries / HN window")
    ap.add_argument("--all", action="store_true", help="include already-seen entries")
    args = ap.parse_args()
    now = datetime.now(timezone.utc)
    cutoff = now - timedelta(days=args.days)
    seen = set() if args.all else load_state()

    buckets: dict[str, list[dict]] = {b: [] for b in BUCKET_KEYWORDS}
    errors: list[str] = []

    for bucket, feeds in RSS_FEEDS.items():
        for url in feeds:
            try:
                for e in parse_rss(url):
                    if not BUCKET_KEYWORDS[bucket].search(e["title"]):
                        continue
                    if e["date"] and e["date"].replace(tzinfo=e["date"].tzinfo or timezone.utc) < cutoff:
                        continue
                    buckets[bucket].append(e)
            except Exception as exc:
                errors.append(f"rss {url}: {type(exc).__name__}: {exc}")

    for bucket, queries in HN_QUERIES.items():
        for q in queries:
            try:
                for e in hn_search(q, args.days):
                    # Algolia fuzzy-matches broadly; re-filter by bucket keywords
                    if BUCKET_KEYWORDS[bucket].search(e["title"]):
                        buckets[bucket].append(e)
            except Exception as exc:
                errors.append(f"hn {q}: {type(exc).__name__}: {exc}")

    fresh_total = 0
    lines = [f"# Idea digest {now:%Y-%m-%d %H:%M UTC}", "",
             f"Window: {args.days} day(s) · buckets: {', '.join(buckets)}", ""]
    for bucket, entries in buckets.items():
        fresh = [e for e in entries if e["id"] not in seen]
        fresh_total += len(fresh)
        seen.update(e["id"] for e in fresh)
        lines.append(f"## {bucket} ({len(fresh)} new)")
        if not fresh:
            lines.append("(nothing new)")
        for e in sorted(fresh, key=lambda x: x["date"] or cutoff, reverse=True)[:25]:
            when = e["date"].strftime("%Y-%m-%d") if e["date"] else "n.d."
            lines.append(f"- [{when}] {e['title']}  <{e['link']}>")
        lines.append("")
    if errors:
        lines.append("## feed errors (non-fatal)")
        lines.extend(f"- {e}" for e in errors)
        lines.append("")

    OUTDIR.mkdir(exist_ok=True)
    out = OUTDIR / f"{now:%Y-%m-%d}.md"
    out.write_text("\n".join(lines), encoding="utf-8")
    if not args.all:
        save_state(seen)
    print(f"{fresh_total} new entries -> {out}")
    if errors:
        print(f"{len(errors)} feed error(s); see digest", file=sys.stderr)
    return 0

if __name__ == "__main__":
    sys.exit(main())
