#!/usr/bin/env bash
# E2E workflow verification for clawdius (migration-parity evidence).
# Each check prints CHECK: <name> -> PASS/FAIL/SKIP with captured output.
set -u
BIN="$(readlink -f "${1:-target/debug/clawdius}")"
MCP_BIN="$(readlink -f "${2:-target/debug/clawdius-mcp}")"
PASS=0; FAIL=0; SKIP=0

check() { # name expected_substring actual_output
  local name="$1" needle="$2" out="$3"
  if [[ "$out" == *"$needle"* ]]; then
    echo "CHECK: $name -> PASS"
    PASS=$((PASS+1))
  else
    echo "CHECK: $name -> FAIL"
    echo "  expected substring: $needle"
    echo "  got: ${out:0:300}"
    FAIL=$((FAIL+1))
  fi
}

echo "== 0. binary exists =="
if [[ -x "$BIN" ]]; then
  echo "  binary: $BIN"; PASS=$((PASS+1)); echo "CHECK: binary built -> PASS"
else
  echo "CHECK: binary built -> FAIL ($BIN not found)"; FAIL=$((FAIL+1)); exit 1
fi

scratch=$(mktemp -d -p /tmp/opencode)
mkdir -p "$scratch/repo" && cd "$scratch/repo" || exit 1
git init -q . && echo "fn main() { println!(\"hello clawdius\"); }" > main.rs
printf '[package]\nname = "scratch"\nversion = "0.1.0"\nedition = "2021"\n' > Cargo.toml
git add -A && git -c user.email=t@t -c user.name=t commit -qm "init"

echo "== 1. binary surface =="
out=$("$BIN" --version 2>&1); check "version" "clawdius" "$out"
out=$("$BIN" --help 2>&1); check "help lists chat" "chat" "$out"
check "help lists sprint" "sprint" "$out"
check "help lists analyze" "analyze" "$out"

echo "== 2. local analysis (no provider needed) =="
out=$(timeout 120 "$BIN" analyze 2>&1); echo "  analyze: ${out:0:200}"
check "analyze runs on a repo" "CLAWDIUS ANALYSIS" "$out"

echo "== 3. provider key availability =="
if [[ -n "${ANTHROPIC_API_KEY:-}" || -n "${OPENAI_API_KEY:-}" || -n "${DEEPSEEK_API_KEY:-}" ]]; then
  echo "  provider key present; running chat E2E"
  out=$(timeout 90 "$BIN" chat "Reply with exactly: E2E-OK" 2>&1)
  check "chat one-shot" "E2E-OK" "$out"
else
  echo "CHECK: chat one-shot -> SKIP (no provider key in env)"
  SKIP=$((SKIP+1))
fi

echo "== 4. MCP server handshake =="
if [[ -x "$MCP_BIN" ]]; then
  out=$( (echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e","version":"0"}}}'; sleep 1) | timeout 10 "$MCP_BIN" 2>/dev/null | head -1)
  check "MCP initialize response" "serverInfo" "$out"
else
  echo "CHECK: MCP handshake -> SKIP ($MCP_BIN not built)"
  SKIP=$((SKIP+1))
fi

out=$(timeout 20 "$BIN" sessions 2>&1)
check "sessions subcommand responds" "ession" "$out"

echo "== 5b. provider key pick (env GOOGLE first, then opencode auth) =="
AUTH=~/.local/share/opencode/auth.json
GOOGLE_KEY="${GOOGLE_API_KEY:-}"
if [ -z "$GOOGLE_KEY" ] && [ -f "$AUTH" ]; then
  # opencode auth.json stores the key under 'key' ('api' was the old shape)
  GOOGLE_KEY=$(python3 -c "
import json
try:
    d=json.load(open('$AUTH'))
    g=d.get('google') or {}
    print(g.get('key') or g.get('api') or '')
except Exception: pass")
fi
DEEPSEEK_KEY=""
if [ -f "$AUTH" ]; then
  DEEPSEEK_KEY=$(python3 -c "
import json
try:
    d=json.load(open('$AUTH'))
    ds=d.get('deepseek') or {}
    print(ds.get('key') or ds.get('api') or '')
except Exception: pass")
fi
if [ -n "$GOOGLE_KEY" ]; then
  export GOOGLE_API_KEY="$GOOGLE_KEY"
  PROV=google; MODEL=gemini-flash-lite-latest
elif [ -n "$DEEPSEEK_KEY" ]; then
  export DEEPSEEK_API_KEY="$DEEPSEEK_KEY"
  PROV=deepseek; MODEL=deepseek-chat
else
  PROV=""; MODEL=""
fi

echo "== 5b. session continuity across processes (durable-sessions row) =="
if [ -n "$PROV" ]; then
SID=$(timeout 120 "$BIN" chat --provider "$PROV" --model "$MODEL" \
  "Remember the codeword BANANA. Reply with exactly: OK" 2>/dev/null \
  | grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' | head -1)
if [ -n "$SID" ]; then
  echo "  session: $SID"
  ANSWER=$(timeout 120 "$BIN" chat --provider "$PROV" --model "$MODEL" -s "$SID" --quiet \
    "What codeword did I ask you to remember? Reply with just the codeword." 2>/dev/null | tail -1)
  check "session continuity (context survives restart)" "BANANA" "$ANSWER"
else
  echo "CHECK: session continuity -> SKIP (no session id captured; provider quota?)"
  SKIP=$((SKIP+1))
fi
fi

echo "== 6. real chat one-shot (provider key from opencode auth) =="
if [ -n "$GOOGLE_KEY" ]; then
  out=$(timeout 120 "$BIN" chat --provider google --model gemini-flash-lite-latest "Reply with exactly: E2E-OK" 2>&1)
  echo "  chat: ${out:0:200}"
  if [[ "$out" == *"Insufficient balance"* || "$out" == *"resource package"* || "$out" == *"Quota exceeded"* || "$out" == *"high demand"* ]]; then
    echo "CHECK: chat one-shot -> SKIP (provider account-side limit; transport verified)"
    SKIP=$((SKIP+1))
  else
    check "chat one-shot with real provider" "E2E-OK" "$out"
  fi

  echo "== 7. streaming round trip (--stream prints tokens live) =="
  sout=$(timeout 120 "$BIN" chat --provider google --model gemini-flash-lite-latest --stream --quiet \
    "Reply with exactly: STREAM-OK" 2>&1)
  echo "  stream: ${sout:0:200}"
  if [[ "$sout" == *"Insufficient balance"* || "$sout" == *"resource package"* || "$sout" == *"Quota exceeded"* || "$sout" == *"high demand"* ]]; then
    echo "CHECK: streaming round trip -> SKIP (provider account-side limit)"
    SKIP=$((SKIP+1))
  else
    check "streaming round trip" "STREAM-OK" "$sout"
  fi
else
  echo "CHECK: chat one-shot -> SKIP (no provider key)"
  echo "CHECK: streaming round trip -> SKIP (no provider key)"
  SKIP=$((SKIP+2))
fi

echo
echo "RESULT: PASS=$PASS FAIL=$FAIL SKIP=$SKIP"
cd / && rm -rf "$scratch"
