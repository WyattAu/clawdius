# Documentation Site Status

## Date: 2026-05-14

## Existing Docs Structure

### `docs/` (root-level docs)
- 15+ standalone markdown files (API_STABILITY.md, GETTING_STARTED.md, SECURITY_AUDIT.md, etc.)
- `docs/book/` — full mdBook structure with `book.toml` and `src/` directory

### `docs/book/` (mdBook)
- **book.toml**: Fully configured with HTML output, search, folding, git links, cname (docs.clawdius.dev)
- **SUMMARY.md**: Comprehensive 119-line TOC with 12 sections (Getting Started, Core Concepts, LLM Providers, Features, Sandboxing, Enterprise, Plugins, Integrations, API Reference, Advanced, Reference, Community)
- **Existing pages** (4 of ~50+ referenced in SUMMARY.md):
  - `src/intro.md` — Introduction with feature comparison, quick start, architecture diagram
  - `src/getting-started/installation.md`
  - `src/getting-started/configuration.md`
  - `src/getting-started/first-chat.md`
- **Missing pages**: ~46 pages referenced in SUMMARY.md but not yet created (concepts/, providers/, features/, sandbox/, enterprise/, plugins/, integrations/, api/, advanced/, reference/, community/ directories and their files)

### `.docs/` (alternative docs)
- 15 standalone markdown files including getting_started.md, architecture_overview.md, api_reference.md, user_guide.md, benchmarks.md

### `docs/index.md` (landing page)
- **Created** — Links to mdBook site, API reference, architecture overview, and contributing guide

## mdBook Build Status

- **mdbook CLI**: NOT installed in this environment (`which mdbook` failed)
- **Cannot verify build** — The book.toml is well-formed and references valid markdown for existing files, but ~46 pages are missing so `mdbook build` would produce warnings for missing files
- **Note**: `book.toml` has `create-missing = true` which will auto-create empty placeholder pages

## Deployment Configuration

### `netlify.toml` (repo root)
- **Updated** — Added `NODE_VERSION = "18"` to build environment
- Existing config:
  - Build command: `cd docs/book && mdbook build`
  - Publish directory: `docs/book/book`
  - RUST_VERSION: 1.75
  - SPA redirect: `/*` → `/index.html`
  - Security headers (X-Frame-Options, X-XSS-Protection, X-Content-Type-Options, Referrer-Policy)
  - Cache headers for HTML (must-revalidate) and static assets (1 year immutable)

### Deployment Readiness
| Item | Status |
|------|--------|
| mdBook book.toml | Complete |
| netlify.toml | Complete |
| Landing page (docs/index.md) | Created |
| Intro page | Complete |
| Getting Started (3 pages) | Complete |
| Remaining ~46 content pages | Missing — need to be written or populated from .docs/ content |
| mdbook CLI for build | Not installed in env — works on Netlify (builds in CI) |

## What's Needed for Full Deployment

1. **Install mdbook**: `cargo install mdbook` (only needed locally; Netlify handles this)
2. **Populate missing pages**: Copy/adapt content from `.docs/` standalone files into the mdBook structure, or write new content for the 46 missing pages
3. **Verify build**: Run `mdbook build docs/book` to confirm no broken links
4. **Push to trigger Netlify**: Once content is ready, pushing to the default branch will trigger automatic deployment

## Files Modified
- `docs/index.md` — Created (docs landing page)
- `netlify.toml` — Updated (added NODE_VERSION)
