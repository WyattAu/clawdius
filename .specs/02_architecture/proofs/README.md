# Proofs moved to `WyattAu/clawdius-spec`

The Lean proof tree that lived here (23 modules, ~5.3k LOC) is now the
standalone package [WyattAu/clawdius-spec](https://github.com/WyattAu/clawdius-spec)
— reusable across the estate, Reservoir-indexable, consumed by this repo
via the root `lakefile.toml` at a pinned release tag.

- Proof changes: PR against `clawdius-spec`, never here.
- The proof gate (`lean4-proofs` job) still runs on every push — it
  builds the pinned package through the `require` in `lakefile.toml`.
- Rigor contract (zero sorries, postulate inventory, import-free
  modules): see `clawdius-spec` README + AGENTS.md.
- Extraction provenance: byte-identical from this directory at
  clawdius `90d8b37`, verified under `leanprover/lean4:4.28.0`.
