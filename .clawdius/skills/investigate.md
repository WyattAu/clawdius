---
name: investigate
description: Diagnose bugs, errors, and unexpected behavior in the codebase
version: 1.0.0
tags: [debug, diagnose, workflow]
arguments:
  - name: issue
    description: Description of the bug or error to investigate
    required: true
  - name: scope
    description: Investigation scope (file, module, or full)
    required: false
    default: auto
examples:
  - "/investigate issue=login fails with 500 error"
  - "/investigate issue=memory leak in worker pool scope=module"
---

# Investigate Skill

Systematically diagnose bugs, errors, and unexpected behavior.

## Instructions

1. Parse the issue description to identify symptoms and context
2. Search the codebase for relevant code using grep and codebase search
3. Trace the execution path from entry point to failure
4. Identify the root cause (not just the symptom)
5. Check recent git history for changes that may have introduced the issue
6. Verify the hypothesis by examining related tests
7. Present findings with:
   - Root cause (specific file, line, and explanation)
   - Impact assessment (how severe, how widespread)
   - Recommended fix approach
   - Related code that should be checked for similar issues

## Constraints

- Always verify assumptions by reading actual code, never guess
- Check test files for existing coverage of the failing path
- Look for similar patterns elsewhere in the codebase
- If the issue spans multiple files, map the dependency chain
- Prioritize theories that explain ALL symptoms, not just the primary one
