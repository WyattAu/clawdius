#!/usr/bin/env bash
# Proof-loop phase gates for this repo (wired via .opencode/proof-loop.json).
#
# Deterministic artifact checks for the plan and research phases, which have
# no auto-detected gate in the proof-loop plugin ("soft phases"). Nonzero
# exit + a failing-item list = gate failure; the loop retries the phase with
# this output as the failure context, same semantics as the prove gate.
#
# Usage: proof-loop-gate.sh plan|research
#
# plan:     .loop/plan.md exists, carries "## The unit of work", and every
#           file path it cites resolves in the tree (directly, or as a
#           basename under the proof tree).
# research: .loop/research.md exists, every http(s) citation returns HTTP 200
#           (24h TTL cache), cited file paths resolve, and every "Done in
#           loop N" claim that names a YP-<ID> has that ID in the registry.
set -uo pipefail

MODE="${1:?usage: proof-loop-gate.sh plan|research}"
PLAN=".loop/plan.md"
RESEARCH=".loop/research.md"
REGISTRY=".specs/01_research/yellow_paper_registry.toml"
PROOF_DIR=".specs/02_architecture/proofs"
CITE_CACHE="${TMPDIR:-/tmp}/clawdius-cite-cache"

# Print the path-like backticked tokens of the given files (one per line).
# A token counts as a cited path when it ends in a known source/doc
# extension (optionally under a slash path). The strict charset excludes
# commands, globs, and ~ paths that happen to sit in backticks; URLs are
# excluded (the research gate checks those separately).
cited_paths() {
	grep -hoE '`[A-Za-z0-9_][A-Za-z0-9_./-]*`' "$@" |
		tr -d '`' |
		grep -E '(^|/)[A-Za-z0-9_-]+\.(lean|rs|md|toml|json|ts|tsx|sh|ya?ml)$' |
		sort -u
}

# Print cited paths that do not resolve. A path resolves when it exists as
# written, or its basename exists under the proof tree or anywhere in
# .specs/ (plans cite e.g. `audit.lean` bare). A nonexistent path preceded
# by a write verb is a deliverable the phase will create, not a broken
# citation ("Write `LOOP-2026-11-21-REPORT.md` ...").
unresolved_cited_paths() {
	local p
	cited_paths "$@" | while IFS= read -r p; do
		[ -e "$p" ] && continue
		[ -e "$PROOF_DIR/$(basename "$p")" ] && continue
		[ -n "$(find .specs -name "$(basename "$p")" -print -quit 2>/dev/null)" ] && continue
		if grep -qiE '(write|create|ship|add|produce)[^`]*`'"$p"'`' "$@" 2>/dev/null; then
			continue
		fi
		echo "$p"
	done
}

gate_plan() {
	if [ ! -f "$PLAN" ]; then
		echo "plan gate: missing $PLAN — write the iteration plan first"
		return 1
	fi
	if ! grep -q '^## The unit of work' "$PLAN"; then
		echo "plan gate: $PLAN lacks a '## The unit of work' heading"
		return 1
	fi
	local missing
	missing=$(unresolved_cited_paths "$PLAN")
	if [ -n "$missing" ]; then
		echo "plan gate: cited file paths not found in the tree:"
		echo "$missing"
		return 1
	fi
	return 0
}

gate_research() {
	if [ ! -f "$RESEARCH" ]; then
		echo "research gate: missing $RESEARCH — resolve the plan's unknowns first"
		return 1
	fi
	# 1. every http(s) citation returns 200 (cached for 24h per URL)
	local u key code bad=""
	mkdir -p "$CITE_CACHE"
	while IFS= read -r u; do
		[ -n "$u" ] || continue
		key=$(printf '%s' "$u" | cksum | cut -d' ' -f1)
		if [ -f "$CITE_CACHE/$key" ]; then
			continue
		fi
		code=$(curl -sS -L -o /dev/null -w '%{http_code}' --max-time 20 "$u" 2>/dev/null || echo 000)
		if [ "$code" = "200" ]; then
			: >"$CITE_CACHE/$key"
		else
			bad="$bad
  $u (HTTP $code)"
		fi
	done < <(grep -hoE 'https?://[^[:space:]`)>+,"]+' "$RESEARCH" | sed 's/[.)]*$//' | sort -u)
	if [ -n "$bad" ]; then
		echo "research gate: citations not returning HTTP 200:$bad"
		return 1
	fi
	# 2. cited file paths resolve (same rule as the plan gate)
	local missing
	missing=$(unresolved_cited_paths "$RESEARCH")
	if [ -n "$missing" ]; then
		echo "research gate: cited file paths not found in the tree:"
		echo "$missing"
		return 1
	fi
	# 3. "Done in loop N" claims naming a paper ID (YP-<NAME>-<NUM>; YP-1..YP-7
	#    alone are section markers) must have that ID in the registry
	local unregistered
	unregistered=$(grep -h 'Done in loop' "$PLAN" "$RESEARCH" 2>/dev/null |
		grep -oE 'YP-[A-Z]{2,}-[0-9]+' | sort -u |
		while IFS= read -r id; do
			grep -q "$id" "$REGISTRY" || echo "$id"
		done)
	if [ -n "$unregistered" ]; then
		echo "research gate: 'Done in loop N' claims citing IDs absent from the registry:"
		echo "$unregistered"
		return 1
	fi
	return 0
}

case "$MODE" in
	plan) gate_plan ;;
	research) gate_research ;;
	*) echo "usage: proof-loop-gate.sh plan|research" >&2; exit 2 ;;
esac
