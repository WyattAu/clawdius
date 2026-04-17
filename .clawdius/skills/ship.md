---
name: ship
description: Stage, commit, and push changes with a generated or provided commit message
version: 1.0.0
tags: [git, deploy, workflow]
arguments:
  - name: message
    description: Optional commit message override
    required: false
  - name: branch
    description: Target branch to push to
    required: false
examples:
  - "/ship"
  - "/ship message=fix: resolve login bug"
  - "/ship message=feat: add user auth branch=develop"
---

# Ship Skill

Automatically stages all changes, generates or uses a commit message,
commits, and pushes to the remote repository.

## Instructions

1. Run `git status` to see what changed
2. Run `git diff --staged` and `git diff` to review all changes
3. If no message provided, analyze the diff and generate a concise conventional commit message
4. Run `git add -A` to stage all changes
5. Run `git commit -m "{message}"` to commit
6. Run `git push origin {branch}` to push (default: current branch)
7. Report the result including the commit hash

## Constraints

- Never push to protected branches (main, master, production) without explicit confirmation
- Always show the user what will be committed before pushing
- If there are no changes, report "nothing to ship" and exit
- Generate conventional commit prefixes: feat:, fix:, docs:, refactor:, test:, chore:
