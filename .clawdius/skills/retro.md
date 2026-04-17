---
name: retro
description: Conduct a sprint retrospective on recent changes
version: 1.0.0
tags: [retrospective, workflow, review]
arguments:
  - name: since
    description: Git ref to start from (commit hash, branch, or relative like HEAD~10)
    required: false
    default: HEAD~10
examples:
  - "/retro"
  - "/retro since=main"
  - "/retro since=v2.0.0"
---

# Retro Skill

Analyze recent work and produce a structured retrospective.

## Instructions

1. Run `git log {since}..HEAD --oneline` to list recent commits
2. Run `git diff --stat {since}..HEAD` to see change volume
3. Analyze each commit for:
   - Type (feature, fix, refactor, docs, test, chore)
   - Scope (which modules/files affected)
   - Complexity (lines changed, files touched)
4. Identify patterns:
   - Most active modules
   - Types of changes (more bugs than features?)
   - Commit message quality
   - Change size distribution
5. Look for issues:
   - Large commits that should have been split
   - Missing test coverage for new features
   - Inconsistent naming or patterns
   - Technical debt introduced
6. Produce a retro report:
   - What went well
   - What could be improved
   - Action items for next sprint
   - Metrics summary (commits, lines added/removed, files changed)

## Constraints

- Be constructive, not critical
- Focus on actionable improvements
- Quantify observations where possible (e.g., "7 of 12 commits were bug fixes")
- Highlight positive patterns too, not just problems
- Keep the report concise and scannable
