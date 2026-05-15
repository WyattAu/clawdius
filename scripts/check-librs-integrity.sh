#!/usr/bin/env bash
set -euo pipefail

errors=0
files=()

for f in crates/*/src/lib.rs; do
    [ -f "$f" ] || continue
    files+=("$f")
done

if [ ${#files[@]} -eq 0 ]; then
    echo "No lib.rs files found under crates/*/src/"
    exit 0
fi

for f in "${files[@]}"; do
    crate=$(basename "$(dirname "$(dirname "$f")")")
    line_count=$(wc -l < "$f")
    pub_mod_count=$(grep -c 'pub mod' "$f" || true)
    pub_item_count=$(grep -cE 'pub (mod|fn|struct|enum|trait|type|const|static|use)' "$f" || true)
    has_inner_attr=$(grep -q '#!\[' "$f" && echo "yes" || echo "no")

    failed=()
    # Strict checks only for clawdius-core (known clobbering target)
    if [ "$crate" = "clawdius-core" ]; then
        if [ "$line_count" -lt 100 ]; then
            failed+=("line count $line_count < 100 (core crate)")
        fi
        if [ "$pub_mod_count" -lt 10 ]; then
            failed+=("pub mod count $pub_mod_count < 10 (core crate)")
        fi
    else
        if [ "$line_count" -lt 5 ]; then
            failed+=("line count $line_count < 5")
        fi
    fi
    if [ "$pub_item_count" -lt 1 ]; then
        failed+=("pub item count $pub_item_count < 1")
    fi
    if [ "$has_inner_attr" = "no" ]; then
        failed+=("missing #![ inner attribute")
    fi

    if [ ${#failed[@]} -gt 0 ]; then
        echo "FAIL: $f ($crate)"
        for reason in "${failed[@]}"; do
            echo "  - $reason"
        done
        errors=$((errors + 1))
    else
        echo "OK:   $f ($crate)"
    fi
done

if [ "$errors" -gt 0 ]; then
    echo ""
    echo "$errors file(s) failed integrity checks."
    exit 1
fi

echo ""
echo "All ${#files[@]} lib.rs files passed integrity checks."
exit 0
