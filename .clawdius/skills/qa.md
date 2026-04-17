---
name: qa
description: Run quality assurance checks on code including tests, linting, and type checking
version: 1.0.0
tags: [testing, quality, workflow]
arguments:
  - name: focus
    description: QA focus area
    required: false
    default: all
examples:
  - "/qa"
  - "/qa focus=security"
  - "/qa focus=performance"
---

# QA Skill

Run comprehensive quality assurance checks on the current codebase state.

## Instructions

1. Run the project's test suite and capture results
2. Run the linter/type checker (cargo check, eslint, mypy, etc. based on project)
3. Check for common issues:
   - Unused imports and variables
   - Missing error handling
   - Security vulnerabilities (hardcoded secrets, SQL injection, XSS)
   - Performance anti-patterns (N+1 queries, unnecessary allocations)
   - Dead code paths
4. Verify code coverage if tools are available
5. Check dependency vulnerabilities if lockfile exists
6. Present a QA report:
   - Pass/Fail summary per category
   - Critical issues (must fix before shipping)
   - Warnings (should fix soon)
   - Suggestions (nice to have)

## Constraints

- Never modify any files during QA
- Report exact file paths and line numbers for all issues
- Distinguish between blocking issues and advisory warnings
- If tests fail, include the full test output for the first 3 failures
- Respect the project's existing CI configuration (don't run checks the project doesn't use)
