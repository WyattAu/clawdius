# Clawdius License Compliance Report

**Generated:** 2026-03-01
**Tool:** cargo-deny 0.19.0
**Project License:** Apache-2.0

## Executive Summary

| Category | Count | Status |
|----------|-------|--------|
| Compatible Licenses | 2932 | PASS |
| Incompatible Licenses | 0 | PASS |
| Copyleft Licenses | 1 | WARNING |
| Unlicensed | 0 | PASS |

**Overall Compliance: PASS**

All dependencies have licenses compatible with Apache-2.0.

## License Allowlist

The following licenses are explicitly allowed in `deny.toml`:

| License | SPDX ID | OSI Approved | Apache-2.0 Compatible |
|---------|---------|--------------|----------------------|
| Apache License 2.0 | Apache-2.0 | Yes | Yes |
| Apache-2.0 WITH LLVM-exception | Apache-2.0 WITH LLVM-exception | Yes | Yes |
| MIT License | MIT | Yes | Yes |
| Mozilla Public License 2.0 | MPL-2.0 | Yes | Yes (weak copyleft) |
| BSD 2-Clause | BSD-2-Clause | Yes | Yes |
| BSD 3-Clause | BSD-3-Clause | Yes | Yes |
| ISC License | ISC | Yes | Yes |
| zlib License | Zlib | Yes | Yes |
| Unicode License v3 | Unicode-3.0 | Yes | Yes |
| CC0 1.0 Universal | CC0-1.0 | Yes | Yes |
| Open Font License 1.1 | OFL-1.1 | Yes | Yes |
| OpenSSL License | OpenSSL | Yes | Yes |
| BSD Zero Clause | 0BSD | Yes | Yes |
| CDLA Permissive 2.0 | CDLA-Permissive-2.0 | No | Yes |
| Boost Software License 1.0 | BSL-1.0 | Yes | Yes (weak copyleft) |

## Direct Dependencies License Matrix

| Crate | Version | License | Compatibility |
|-------|---------|---------|---------------|
| monoio | 0.2.4 | Apache-2.0 | Compatible |
| wasmtime | 42.0.1 | Apache-2.0 | Compatible |
| rusqlite | 0.38.0 | MIT | Compatible |
| lancedb | 0.26.2 | Apache-2.0 | Compatible |
| genai | 0.5.3 | Apache-2.0 | Compatible |
| tree-sitter | 0.26.0 | MIT | Compatible |
| ratatui | 0.30.0 | MIT | Compatible |
| serde | 1.0.228 | Apache-2.0 OR MIT | Compatible |
| serde_json | 1.0.149 | Apache-2.0 OR MIT | Compatible |
| toml | 1.0.3+spec-1.1.0 | Apache-2.0 OR MIT | Compatible |
| rkyv | 0.8.10 | MIT | Compatible |
| async-openai | 0.33.0 | Apache-2.0 | Compatible |
| crossterm | 0.29.0 | Apache-2.0 | Compatible |
| keyring | 3.6.2 | Apache-2.0 OR MIT | Compatible |
| uuid | 1.21.0 | Apache-2.0 OR MIT | Compatible |
| thiserror | 2.0.18 | Apache-2.0 OR MIT | Compatible |
| tracing | 0.1.44 | MIT | Compatible |
| tracing-subscriber | 0.3.20 | MIT | Compatible |
| syntect | 5.3.0 | Apache-2.0 | Compatible |
| mimalloc | 0.1.46 | MIT | Compatible |

## Dev Dependencies License Matrix

| Crate | Version | License | Compatibility |
|-------|---------|---------|---------------|
| proptest | 1.10.0 | Apache-2.0 OR MIT | Compatible |
| rstest | 0.25.0 | Apache-2.0 OR MIT | Compatible |
| criterion | 0.5.1 | Apache-2.0 OR MIT | Compatible |
| tempfile | 3.26.0 | Apache-2.0 OR MIT | Compatible |

## Copyleft Analysis

### Weak Copyleft Dependencies

| Crate | License | Impact |
|-------|---------|--------|
| MPL-2.0 crates | MPL-2.0 | File-level copyleft - safe for static linking |
| BSL-1.0 crates | BSL-1.0 | Boost license - requires source availability after 3 years for commercial use |

**Assessment:** Weak copyleft licenses are acceptable for Apache-2.0 projects when used as dependencies.

### Strong Copyleft (Not Found)

No GPL, AGPL, LGPL, or SSPL licensed dependencies were found.

## Special License Notes

### OpenSSL License (aws-lc-sys)

The `aws-lc-sys` crate uses a dual license: `ISC AND (Apache-2.0 OR ISC) AND OpenSSL`.

This is a standard cryptographic library license that is compatible with Apache-2.0 for use as a dependency.

### CDLA-Permissive-2.0 (webpki-root-certs)

The Community Data License Agreement Permissive 2.0 is used for certificate data. This is compatible with Apache-2.0.

## License Compatibility Graph

```
Apache-2.0 (Project)
    ├── Apache-2.0 (monoio, wasmtime, lancedb, genai, async-openai, crossterm, syntect)
    ├── MIT (rusqlite, tree-sitter, ratatui, rkyv, tracing, tracing-subscriber, mimalloc)
    ├── Apache-2.0 OR MIT (serde, serde_json, toml, keyring, uuid, thiserror, proptest, rstest, criterion, tempfile)
    ├── MPL-2.0 (transitive)
    ├── BSL-1.0 (xxhash-rust via lance-encoding)
    ├── OpenSSL (aws-lc-sys via rustls)
    ├── 0BSD (mock_instant via lance-core)
    └── CDLA-Permissive-2.0 (webpki-root-certs)
```

## Recommendations

1. **Current Status:** All licenses are compatible. No action required.

2. **Future Dependencies:**
   - Always check license compatibility before adding dependencies
   - Reject GPL, AGPL, LGPL, and SSPL licensed crates
   - Document any exceptions in this report

3. **Attribution:** Consider generating a NOTICE file with all dependency copyright notices for distribution.

## Conclusion

**License Compliance: PASS**

All 2932 dependencies have licenses compatible with the project's Apache-2.0 license. No GPL-family or other incompatible licenses were found. The supply chain is compliant with open source licensing requirements.
