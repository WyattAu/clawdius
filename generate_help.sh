#!/bin/bash
mkdir -p .help_output
cargo run --bin clawdius -- --help > .help_output/clawdius.txt
for cmd in chat auto init setup sessions refactor action test doc verify verify-image consolidate pipeline maintain briefing acp metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config; do
    cargo run --bin clawdius -- $cmd --help > .help_output/clawdius_$cmd.txt
done
