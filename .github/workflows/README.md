# Clawdius GitHub Action Examples

This directory contains example workflows for using the Clawdius GitHub Action in CI/CD pipelines.

## Basic Usage

```yaml
name: AI Code Review
on:
  pull_request:
    types: [opened, synchronize]

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: AI Code Review
        uses: clawdius/clawdius/.github/actions/clawdius@main
        with:
          task: 'Review the changes in this PR and suggest improvements'
          provider: 'anthropic'
          mode: 'review'
          api-key: ${{ secrets.ANTHROPIC_API_KEY }}
```

## Automated Bug Fixes

```yaml
name: Auto-Fix Bugs
on:
  issues:
    types: [labeled]

jobs:
  fix:
    if: contains(github.event.issue.labels.*.name, 'auto-fix')
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Fix Bug
        uses: clawdius/clawdius/.github/actions/clawdius@main
        with:
          task: ${{ github.event.issue.title }} - ${{ github.event.issue.body }}
          provider: 'anthropic'
          mode: 'debug'
          run-tests: 'true'
          auto-commit: 'true'
          api-key: ${{ secrets.ANTHROPIC_API_KEY }}
```

## Test Generation

```yaml
name: Generate Tests
on:
  workflow_dispatch:
    inputs:
      file:
        description: 'File to generate tests for'
        required: true

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Generate Tests
        uses: clawdius/clawdius/.github/actions/clawdius@main
        with:
          task: 'Generate comprehensive unit tests for ${{ github.event.inputs.file }}'
          provider: 'anthropic'
          mode: 'test'
          run-tests: 'true'
          fail-on-test-failure: 'true'
          api-key: ${{ secrets.ANTHROPIC_API_KEY }}
```

## Refactoring

```yaml
name: Refactor Code
on:
  workflow_dispatch:
    inputs:
      description:
        description: 'Refactoring description'
        required: true

jobs:
  refactor:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Refactor
        uses: clawdius/clawdius/.github/actions/clawdius@main
        with:
          task: ${{ github.event.inputs.description }}
          provider: 'anthropic'
          mode: 'refactor'
          run-tests: 'true'
          auto-commit: 'true'
          api-key: ${{ secrets.ANTHROPIC_API_KEY }}
```

## Scheduled Code Review

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
      
      - name: Weekly Review
        uses: clawdius/clawdius/.github/actions/clawdius@main
        with:
          task: 'Review all code changes from the past week and create a summary report'
          provider: 'anthropic'
          mode: 'architect'
          api-key: ${{ secrets.ANTHROPIC_API_KEY }}
```

## Multi-Provider Setup

```yaml
name: Multi-Provider Analysis
on: [push]

jobs:
  analyze:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        provider: [anthropic, openai]
    steps:
      - uses: actions/checkout@v4
      
      - name: Analyze with ${{ matrix.provider }}
        uses: clawdius/clawdius/.github/actions/clawdius@main
        with:
          task: 'Analyze code quality and suggest improvements'
          provider: ${{ matrix.provider }}
          mode: 'review'
          api-key: ${{ matrix.provider == 'anthropic' && secrets.ANTHROPIC_API_KEY || secrets.OPENAI_API_KEY }}
```
