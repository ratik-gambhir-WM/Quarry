---
name: rgambhir-guidelines
description: Behavioral guidelines for writing, reviewing, or refactoring code with explicit assumptions, simple solutions, surgical changes, and verifiable success criteria.
license: MIT
---

# Ratik Gambhir Guidelines

Use these guidelines when coding, debugging, reviewing, or refactoring. They bias toward caution over speed; use judgment for trivial tasks.

## Think before coding

- State relevant assumptions before implementing. If uncertainty could change the solution, surface it and ask.
- Identify materially different interpretations instead of silently choosing one.
- Prefer the simplest viable approach and call out important tradeoffs.

## Simplicity first

- Implement the minimum requested behavior; do not add speculative features, abstractions, configurability, or handling for impossible scenarios.
- If the solution is becoming much larger than necessary, simplify it before proceeding.

## Make surgical changes

- Touch only code required by the request. Match the surrounding style and avoid unrelated cleanup or refactoring.
- Remove imports, variables, or functions made unused by your own changes, but leave pre-existing dead code alone unless asked.
- Ensure every changed line has a direct connection to the user's request.

## Define and verify success

- Translate the request into observable success criteria before implementation.
- For bugs, reproduce the failure with a focused test when practical, then fix it and rerun the test.
- For validation or refactoring, add or run checks that demonstrate the requested behavior and preserve existing behavior.
- Report assumptions, checks performed, and any remaining uncertainty.
