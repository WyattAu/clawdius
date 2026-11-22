# Finding: gate coverage gap in plan/research phases (2026-11-20)

## Evidence

`.loop/events.jsonl` (maintain loop, clawdius-maint):
- plan phase: `gate-pass, gate="(none)", detail="no executable gate for this phase/project"`
- research phase: same — twice
- Only the prove phase carries an executable gate.

## Why it matters

The proof-loop's value is `plan -> research -> prove` with SELF-VERIFICATION
at each phase. With 2 of 3 phases ungated, the loop can plan and research
into thin air (no artifact check, no source-citation check) and only discover
problems at prove time — the most expensive place. The RFC 6962 anchoring
research in `.loop/research.md` is exactly the kind of output a
"research gate" would have verified while fresh (e.g., every cited RFC
section resolves, every claim maps to a file in the repo).

## Recommended improvement

1. Add phase-scoped gate configs (per project, in clawdius.toml or
   `.loop/gates.toml`):
   - plan gate: `.loop/plan.md` exists + contains a "## The unit of work"
     heading + every file path it cites exists in the tree
   - research gate: every http(s) citation in research.md returns 200
     (cached); every `repo_path` cited exists; claims marked "Done in
     loop N" match the registry
2. Gate failures should route to a re-run of the phase (bounded), not a
   loop abort — same semantics as the prove gate.
3. Emit `gate-fail` events with the failing item list so the next
   iteration's prompt can self-correct.

## Where the gate engine lives

The "no executable gate" string is emitted by the built binary
(`/home/wyatt/dev/.targets/clawdius-acp/debug/clawdius`), built from the
clawdius repo (not this maint clone). Gate config plumb-through is a
clawdius-repo change; this file is the spec for it.
