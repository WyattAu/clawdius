---
name: office-hours
description: Product thinking and requirements exploration session
version: 1.0.0
tags: [product, requirements, thinking]
arguments:
  - name: topic
    description: The product area or feature to explore
    required: true
  - name: audience
    description: Target audience perspective
    required: false
    default: user
examples:
  - "/office-hours topic=user onboarding flow"
  - "/office-hours topic=API rate limiting audience=enterprise"
---

# Office Hours Skill

A product thinking session that explores requirements, user needs, and design tradeoffs.

## Instructions

1. Clarify the topic and scope with the user
2. Ask probing questions to understand:
   - Who is the primary user? What are their goals?
   - What problem does this solve? How painful is it today?
   - What are the success criteria? How do we measure "done"?
   - What are the constraints (time, budget, technical, regulatory)?
   - What are the risks? What could go wrong?
3. Explore the design space:
   - What are the possible approaches? (at least 3)
   - What are the tradeoffs of each?
   - What would a minimal viable version look like?
   - What would the ideal version look like?
4. Identify open questions and assumptions
5. Produce a summary:
   - Problem statement (one sentence)
   - User stories (As a... I want... So that...)
   - Acceptance criteria
   - Open questions
   - Recommended next steps

## Constraints

- Never jump to solutions without understanding the problem first
- Challenge assumptions gently but firmly
- Prioritize user value over technical elegance
- If requirements are ambiguous, present options rather than guessing
- Keep the scope realistic — flag scope creep early
