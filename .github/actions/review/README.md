# Clawdius Code Review Action

A GitHub Action for running AI-powered code review with Clawdius. This action integrates seamlessly into your CI/CD pipeline to provide automated code review using Claude AI.

## Features

- **Automated PR Reviews**: Automatically review pull requests with AI-powered analysis
- **Flexible Scope**: Review changed files, entire repository, or specific paths
- **Multiple Output Formats**: Post as PR comments, GitHub Checks, or upload as artifacts
- **Configurable**: Customize review behavior through configuration files
- **Caching Support**: Faster execution with dependency and binary caching
- **Error Handling**: Graceful degradation with comprehensive error reporting

## Usage

### Basic Usage

Add this action to your workflow file (e.g., `.github/workflows/code-review.yml`):

```yaml
name: Code Review

on:
  pull_request:
    types: [opened, synchronize, reopened]

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        
      - name: Run Clawdius Review
        uses: ./.github/actions/review
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          anthropic-api-key: ${{ secrets.ANTHROPIC_API_KEY }}
```

### Advanced Usage

```yaml
name: Code Review

on:
  pull_request:
    types: [opened, synchronize, reopened]
  push:
    branches: [main]

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0
      
      - name: Run Clawdius Review
        uses: ./.github/actions/review
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          anthropic-api-key: ${{ secrets.ANTHROPIC_API_KEY }}
          config-path: '.clawdius'
          review-scope: 'changed-files'
          output-format: 'comment'
          fail-on-issues: 'false'
          rust-toolchain: 'stable'
          cache-key: 'clawdius-v1'
```

## Inputs

| Input | Description | Required | Default |
|-------|-------------|----------|---------|
| `github-token` | GitHub token for API access | Yes | - |
| `anthropic-api-key` | Anthropic API key for Claude | Yes | - |
| `config-path` | Path to .clawdius/config.toml | No | `.clawdius` |
| `review-scope` | Review scope: `changed-files`, `full-repo`, or specific paths | No | `changed-files` |
| `output-format` | Output format: `comment`, `check`, or `artifact` | No | `comment` |
| `fail-on-issues` | Fail workflow if issues are found | No | `false` |
| `rust-toolchain` | Rust toolchain version | No | `stable` |
| `cache-key` | Custom cache key prefix | No | `clawdius` |

## Outputs

| Output | Description |
|--------|-------------|
| `issues-found` | Number of issues found during review |
| `review-status` | Review completion status (`success`, `failed`, or `skipped`) |
| `report-path` | Path to the generated report |

## Configuration

Create a `.clawdius/config.toml` file in your repository to customize review behavior:

```toml
[review]
enabled = true
severity = "medium"

[review.rules]
security = true
performance = true
best_practices = true
code_style = false

[review.exclude]
paths = ["vendor/*", "generated/*"]
file_types = ["lock", "sum"]
```

## Examples

### Review Only Changed Files

```yaml
- name: Review Changed Files
  uses: ./.github/actions/review
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    anthropic-api-key: ${{ secrets.ANTHROPIC_API_KEY }}
    review-scope: 'changed-files'
```

### Review Specific Paths

```yaml
- name: Review Specific Paths
  uses: ./.github/actions/review
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    anthropic-api-key: ${{ secrets.ANTHROPIC_API_KEY }}
    review-scope: 'src/,lib/,tests/'
```

### Full Repository Review

```yaml
- name: Full Repository Review
  uses: ./.github/actions/review
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    anthropic-api-key: ${{ secrets.ANTHROPIC_API_KEY }}
    review-scope: 'full-repo'
```

### Create GitHub Check

```yaml
- name: Create Check
  uses: ./.github/actions/review
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    anthropic-api-key: ${{ secrets.ANTHROPIC_API_KEY }}
    output-format: 'check'
```

### Upload as Artifact

```yaml
- name: Upload Review Artifact
  uses: ./.github/actions/review
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    anthropic-api-key: ${{ secrets.ANTHROPIC_API_KEY }}
    output-format: 'artifact'
```

### Fail on Issues

```yaml
- name: Review and Fail on Issues
  uses: ./.github/actions/review
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    anthropic-api-key: ${{ secrets.ANTHROPIC_API_KEY }}
    fail-on-issues: 'true'
```

## Required Secrets

Add the following secrets to your repository:

1. **GITHUB_TOKEN**: Automatically provided by GitHub Actions
2. **ANTHROPIC_API_KEY**: Your Anthropic API key for Claude access

To add the Anthropic API key:
1. Go to your repository settings
2. Navigate to "Secrets and variables" → "Actions"
3. Click "New repository secret"
4. Name: `ANTHROPIC_API_KEY`
5. Value: Your Anthropic API key

## Caching Strategy

The action implements a multi-level caching strategy:

1. **Cargo Registry**: Caches Cargo's registry and git dependencies
2. **Clawdius Binary**: Caches the compiled Clawdius binary
3. **Custom Cache Key**: Allows cache invalidation through custom keys

## Workflow Integration

### With PR Checks

```yaml
name: PR Checks

on:
  pull_request:
    types: [opened, synchronize, reopened]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm run lint
      
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm test
      
  review:
    runs-on: ubuntu-latest
    needs: [lint, test]
    steps:
      - uses: actions/checkout@v4
      - name: Code Review
        uses: ./.github/actions/review
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          anthropic-api-key: ${{ secrets.ANTHROPIC_API_KEY }}
```

### Scheduled Full Review

```yaml
name: Weekly Code Review

on:
  schedule:
    - cron: '0 0 * * 0'  # Every Sunday at midnight

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: Full Repository Review
        uses: ./.github/actions/review
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          anthropic-api-key: ${{ secrets.ANTHROPIC_API_KEY }}
          review-scope: 'full-repo'
          output-format: 'artifact'
```

## Troubleshooting

### Action Fails with API Error

- Verify your `ANTHROPIC_API_KEY` is correctly set
- Check your API key has sufficient credits
- Ensure the key has the necessary permissions

### Review Not Posting Comments

- Ensure `GITHUB_TOKEN` has write permissions for pull requests
- Check if the workflow has the necessary permissions in repository settings
- Verify the PR is from the same repository (not a fork)

### Caching Issues

If you encounter caching issues:
1. Try using a different `cache-key` value
2. Clear the GitHub Actions cache manually
3. Set `cache-key` to include a timestamp for cache busting

## Best Practices

1. **Start Small**: Begin with `changed-files` scope before enabling full repository reviews
2. **Configure Wisely**: Use `.clawdius/config.toml` to exclude generated files and dependencies
3. **Review Frequency**: Run on PRs rather than every push to save API calls
4. **Fail Policy**: Set `fail-on-issues` to `false` initially, then enable as you tune the rules
5. **Monitor Costs**: Keep track of your Anthropic API usage

## Support

For issues and feature requests:
- GitHub Issues: [clawdius/issues](https://github.com/your-org/clawdius/issues)
- Documentation: [docs.clawdius.dev](https://docs.clawdius.dev)

## License

MIT License - See LICENSE file for details
