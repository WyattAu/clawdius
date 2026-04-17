---
name: sprint
description: Run a full development sprint: think → plan → build → review → test → ship → reflect
version: 1.0.0
tags: [workflow, sprint, autonomous]
arguments:
  - name: task
    description: What to build or accomplish
    required: true
  - name: phases
    description: Comma-separated phases to run (default: all)
    required: false
  - name: auto
    description: Run without human approval between phases (true/false)
    required: false
    default: "true"
examples:
  - "/sprint task=Add user authentication with JWT"
  - "/sprint task=Fix login timeout phases=think,build,test"
  - "/sprint task=Refactor database layer auto=false"
---

# Sprint Skill

Execute a complete development sprint through 7 phases, each building on the previous.

## Instructions

1. Parse the task description and configuration from arguments
2. Initialize a SprintEngine with:
   - The task description
   - Current project root
   - Auto-approve mode (from `auto` argument)
   - Phases to run (from `phases` argument, or all if not specified)
3. Execute each phase in order:
   - **Think**: Product thinking, requirements analysis
   - **Plan**: Create detailed execution plan with file paths
   - **Build**: Write/modify code following the plan
   - **Review**: Code review with quality scoring
   - **Test**: Run tests and verify correctness
   - **Ship**: Prepare commit message and verify readiness
   - **Reflect**: Retrospective with lessons learned
4. After each phase, display the output to the user
5. If tests fail in the Test phase, automatically retry Build→Test up to 3 times
6. Present the final sprint report with:
   - Phase-by-phase summary
   - Files modified
   - Test results
   - Recommendations

## Constraints

- Always show the user what's happening (current phase, progress)
- If auto=false, pause after each phase for user confirmation
- If any phase fails critically, stop the sprint and report
- Never push to protected branches without explicit user confirmation
- The Build phase must produce actual code changes, not just descriptions
- Test phase must run real tests (cargo test, pytest, etc.) based on project type
- Accumulate context from each phase — later phases see all earlier outputs
