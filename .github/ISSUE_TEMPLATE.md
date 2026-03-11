# Clawdius Issue Templates

> Use these templates when creating GitHub issues for the Clawdius project.
> Select the appropriate template based on the issue type.

---

## 🐛 Bug Report

Use this template for reporting bugs or unexpected behavior.

```markdown
---
name: Bug Report
about: Report a bug or unexpected behavior
title: '[BUG] '
labels: bug, needs-triage
assignees: ''
---

## Bug Description
A clear and concise description of what the bug is.

## Steps to Reproduce
1. Go to '...'
2. Click on '....'
3. Scroll down to '....'
4. See error

## Expected Behavior
A clear and concise description of what you expected to happen.

## Actual Behavior
A clear and concise description of what actually happened.

## Screenshots
If applicable, add screenshots to help explain your problem.

## Environment
- OS: [e.g. Linux, macOS, Windows]
- Rust Version: [e.g. 1.75.0]
- Clawdius Version: [e.g. 0.7.2]
- Database: [e.g. PostgreSQL 15]

## Logs
```
Paste relevant logs here
```

## Additional Context
Add any other context about the problem here.

## Possible Solution
If you have suggestions for a fix, please describe them here.
```

---

## ✨ Feature Request

Use this template for requesting new features or enhancements.

```markdown
---
name: Feature Request
about: Suggest a new feature or enhancement
title: '[FEATURE] '
labels: enhancement, needs-triage
assignees: ''
---

## Feature Description
A clear and concise description of the feature you'd like to see.

## Problem Statement
What problem does this feature solve? Why is it needed?

## Proposed Solution
A clear and concise description of what you want to happen.

## Alternatives Considered
A clear description of any alternative solutions or features you've considered.

## Use Cases
Describe specific use cases for this feature:
1. Use case 1...
2. Use case 2...

## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Additional Context
Add any other context or screenshots about the feature request here.

## Priority
How important is this feature?
- [ ] Critical (blocking other work)
- [ ] High (needed soon)
- [ ] Medium (nice to have)
- [ ] Low (future consideration)
```

---

## 🔧 Technical Debt

Use this template for tracking technical debt items.

```markdown
---
name: Technical Debt
about: Document technical debt that needs to be addressed
title: '[DEBT] '
labels: technical-debt, needs-triage
assignees: ''
---

## Debt Description
A clear and concise description of the technical debt.

## Location
Where in the codebase is this debt located?
- File(s):
- Module(s):
- Function(s):

## Impact
What is the impact of this debt?
- [ ] Performance degradation
- [ ] Maintenance difficulty
- [ ] Testing challenges
- [ ] Security risks
- [ ] Scalability issues
- [ ] Other: [specify]

## Root Cause
How did this debt accumulate?
- [ ] Time pressure
- [ ] Lack of knowledge
- [ ] Changing requirements
- [ ] Legacy code
- [ ] Other: [specify]

## Proposed Solution
How should this debt be addressed?

## Effort Estimate
- Estimated hours: [e.g. 8h]
- Complexity: [Low/Medium/High]
- Risk: [Low/Medium/High]

## Priority
- [ ] Critical (address immediately)
- [ ] High (address in next sprint)
- [ ] Medium (address in next quarter)
- [ ] Low (address when convenient)

## Benefits
What benefits will addressing this debt provide?
- [ ] Improved performance
- [ ] Easier maintenance
- [ ] Better testability
- [ ] Enhanced security
- [ ] Other: [specify]

## Dependencies
Does addressing this debt depend on other work?
- [ ] No dependencies
- [ ] Yes: [list dependencies]

## Additional Context
Add any other context about the technical debt here.
```

---

## 📚 Documentation Improvement

Use this template for documentation improvements.

```markdown
---
name: Documentation Improvement
about: Suggest improvements to documentation
title: '[DOCS] '
labels: documentation, needs-triage
assignees: ''
---

## Documentation Location
Where is the documentation that needs improvement?
- File(s):
- URL(s):
- Section(s):

## Current State
Describe the current state of the documentation.

## Problem
What is wrong or missing in the current documentation?
- [ ] Outdated information
- [ ] Missing information
- [ ] Incorrect information
- [ ] Unclear explanations
- [ ] Missing examples
- [ ] Broken links
- [ ] Other: [specify]

## Proposed Improvement
Describe how the documentation should be improved.

## Suggested Content
If you have specific content suggestions, provide them here:

```markdown
[Your suggested documentation content]
```

## Target Audience
Who is the target audience for this documentation?
- [ ] End users
- [ ] Developers
- [ ] System administrators
- [ ] Contributors
- [ ] Other: [specify]

## Priority
- [ ] Critical (blocking users)
- [ ] High (frequently accessed)
- [ ] Medium (important but not urgent)
- [ ] Low (nice to have)

## Additional Context
Add any other context about the documentation improvement here.
```

---

## 🚀 Performance Issue

Use this template for performance-related issues.

```markdown
---
name: Performance Issue
about: Report a performance problem
title: '[PERF] '
labels: performance, needs-triage
assignees: ''
---

## Performance Issue Description
A clear description of the performance issue.

## Current Performance
What is the current performance?
- Metric: [e.g. response time, throughput]
- Current value: [e.g. 500ms]
- Target value: [e.g. 100ms]

## Environment
- Hardware: [e.g. CPU, RAM]
- Dataset size: [e.g. 1M records]
- Load: [e.g. 1000 req/s]

## Profiling Data
If available, provide profiling data or benchmarks:

```
[Benchmark results or profiling output]
```

## Reproduction Steps
1. Step 1
2. Step 2
3. Step 3

## Impact
How does this performance issue affect users?
- [ ] User experience degradation
- [ ] System instability
- [ ] Resource exhaustion
- [ ] Cost increase
- [ ] Other: [specify]

## Proposed Solution
Do you have suggestions for improving performance?

## Priority
- [ ] Critical (system unusable)
- [ ] High (significant impact)
- [ ] Medium (noticeable impact)
- [ ] Low (minor impact)

## Additional Context
Add any other context about the performance issue here.
```

---

## 🔒 Security Issue

Use this template for security-related issues.

```markdown
---
name: Security Issue
about: Report a security vulnerability
title: '[SECURITY] '
labels: security, needs-triage
assignees: ''
---

⚠️ **WARNING**: For critical security vulnerabilities, please email security@example.com instead of creating a public issue.

## Security Issue Description
A clear description of the security issue.

## Vulnerability Type
- [ ] SQL Injection
- [ ] XSS (Cross-Site Scripting)
- [ ] CSRF (Cross-Site Request Forgery)
- [ ] Authentication Bypass
- [ ] Authorization Bypass
- [ ] Data Exposure
- [ ] DoS (Denial of Service)
- [ ] Other: [specify]

## Severity
- [ ] Critical
- [ ] High
- [ ] Medium
- [ ] Low

## Affected Components
Which components are affected?
- Component(s):
- Version(s):

## Reproduction Steps
1. Step 1
2. Step 2
3. Step 3

## Proof of Concept
If applicable, provide a proof of concept:

```
[Proof of concept code]
```

## Impact
What is the potential impact of this vulnerability?

## Proposed Mitigation
Do you have suggestions for fixing this vulnerability?

## Additional Context
Add any other context about the security issue here.
```

---

## 📋 Task / Chore

Use this template for general tasks and maintenance work.

```markdown
---
name: Task / Chore
about: Create a task or chore item
title: '[TASK] '
labels: chore, needs-triage
assignees: ''
---

## Task Description
A clear description of the task.

## Type of Task
- [ ] Dependency update
- [ ] Code cleanup
- [ ] Configuration change
- [ ] Build/CI improvement
- [ ] Tooling update
- [ ] Other: [specify]

## Motivation
Why is this task needed?

## Implementation Plan
How should this task be completed?
1. Step 1
2. Step 2
3. Step 3

## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Effort Estimate
- Estimated hours: [e.g. 2h]
- Complexity: [Low/Medium/High]

## Priority
- [ ] Critical (blocking other work)
- [ ] High (needed soon)
- [ ] Medium (nice to have)
- [ ] Low (when convenient)

## Dependencies
Does this task depend on other work?
- [ ] No dependencies
- [ ] Yes: [list dependencies]

## Additional Context
Add any other context about the task here.
```

---

## 🎯 Epic

Use this template for creating epics that group related issues.

```markdown
---
name: Epic
about: Create an epic to group related issues
title: '[EPIC] '
labels: epic, needs-triage
assignees: ''
---

## Epic Description
A clear description of the epic and its goals.

## Business Value
What business value does this epic deliver?

## Scope
What is included in this epic?
- [ ] Feature 1
- [ ] Feature 2
- [ ] Feature 3

## Out of Scope
What is explicitly NOT included in this epic?

## User Stories
List the user stories that make up this epic:
1. As a [user], I want [feature] so that [benefit]
2. As a [user], I want [feature] so that [benefit]

## Child Issues
List the issues that belong to this epic:
- [ ] #issue-1
- [ ] #issue-2
- [ ] #issue-3

## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Dependencies
Does this epic depend on other epics or issues?
- [ ] No dependencies
- [ ] Yes: [list dependencies]

## Timeline
- Start date: [YYYY-MM-DD]
- Target end date: [YYYY-MM-DD]
- Estimated duration: [e.g. 4 weeks]

## Resources
- Team: [e.g. Backend Team]
- Estimated effort: [e.g. 200 hours]

## Risks
Identify potential risks:
1. Risk 1 - Mitigation: [how to mitigate]
2. Risk 2 - Mitigation: [how to mitigate]

## Additional Context
Add any other context about the epic here.
```

---

## Label Reference

### Type Labels
- `bug` - Something isn't working
- `enhancement` - New feature or request
- `documentation` - Improvements or additions to documentation
- `technical-debt` - Technical debt items
- `performance` - Performance-related issues
- `security` - Security vulnerabilities
- `chore` - Maintenance tasks
- `epic` - Large feature groupings

### Priority Labels
- `priority-critical` - Must be addressed immediately
- `priority-high` - Should be addressed soon
- `priority-medium` - Normal priority
- `priority-low` - Nice to have

### Status Labels
- `needs-triage` - Needs initial review
- `in-progress` - Currently being worked on
- `blocked` - Blocked by another issue
- `ready-for-review` - Ready for code review
- `wontfix` - Will not be fixed

### Component Labels
- `component-core` - Core system
- `component-nexus-fsm` - Nexus FSM
- `component-lean4` - Lean4 integration
- `component-hft` - HFT broker feeds
- `component-tqa` - TQA system
- `component-timeline` - File timeline
- `component-ci` - CI/CD pipeline

---

*Template version: 1.0*
*Last updated: 2026-03-07*
