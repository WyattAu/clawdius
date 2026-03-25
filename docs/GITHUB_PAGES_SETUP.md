# GitHub Pages Setup Guide

## Enable GitHub Pages for docs.clawdius.dev

### Step 1: Enable GitHub Pages

1. Go to https://github.com/WyattAu/clawdius/settings/pages
2. Under "Build and deployment":
   - Source: Select "GitHub Actions"
3. Click "Save"

### Step 2: Configure Custom Domain (Optional)

1. Add CNAME record in DNS:
   ```
   docs.clawdius.dev → wyattau.github.io
   ```

2. In GitHub Pages settings:
   - Custom domain: `docs.clawdius.dev`
   - Enforce HTTPS: ✅

### Step 3: Trigger Deployment

After enabling Pages, trigger the docs workflow:

```bash
gh workflow run docs.yml
```

Or manually:
1. Go to https://github.com/WyattAu/clawdius/actions/workflows/docs.yml
2. Click "Run workflow"
3. Select branch: main
4. Click "Run workflow"

### Step 4: Verify

- Check build status: https://github.com/WyattAu/clawdius/actions
- Once complete, docs available at: https://docs.clawdius.dev (or https://wyattau.github.io/clawdius)

## Alternative: Manual Build

If GitHub Actions fails, build locally:

```bash
# Install mdBook
cargo install mdbook

# Build
cd docs/book
mdbook build

# The output is in docs/book/book/
# Can be deployed to any static hosting (Netlify, Vercel, etc.)
```

## Troubleshooting

### "Page build failed"
- Check the Actions tab for error logs
- Ensure all markdown files have valid syntax
- Verify SUMMARY.md links are correct

### "Custom domain not working"
- DNS propagation can take up to 48 hours
- Verify CNAME record is correct
- Check GitHub Pages settings show domain as "DNS check successful"

### "Workflow not triggering"
- Ensure workflow file is on main branch
- Check workflow has correct paths filter
- Try manual trigger via GitHub UI
