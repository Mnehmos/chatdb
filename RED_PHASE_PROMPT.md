# Red Phase Prompt

Reusable prompt templates for Red-phase test design and failing-test evidence in ChatDB.

Use this file when the next agent's job is to define behavior precisely, add tests first, and prove the repo is red for the right reason before any implementation work begins.

## Full Prompt

```text
You are performing a Red-phase pass on ChatDB.

Mission:
- understand the requested behavior precisely
- identify the smallest set of tests that prove the behavior is missing or incorrect
- add or refine failing tests before any production implementation
- ensure failures are meaningful and tied to the intended behavior
- produce clear Red evidence

Repository constraints:
- Follow AGENTS.md strictly.
- ChatDB architecture is non-negotiable:
  - SQLite and the DAG are the source of truth
  - Rust owns orchestration and DB writes
  - Python sidecar is stateless compute and validation
  - frontend observes state and never mutates core system state directly
- TDD order matters: Red -> Green -> Blue
- No implementation before failing tests.
- If the requested behavior cannot be tested cleanly yet, create the narrowest test seam needed without solving the feature.

Primary Red-phase goal:
- write tests that fail for the missing behavior itself, not for unrelated setup noise

Required process:
1. Clarify the requested behavior from local context
   - read relevant code, schema, models, and existing tests
   - identify the precise contract to enforce
2. Choose the right test layer
   - Rust unit/integration test
   - Python pytest
   - TypeScript or component/store test
   - avoid broad end-to-end tests when a narrower seam will prove the behavior
3. Add the smallest sufficient failing test set
   - cover the expected success path
   - cover at least one key failure or edge case where relevant
   - prefer deterministic tests
4. Run the targeted tests
   - confirm they fail
   - confirm the failure message matches the intended missing behavior
5. Stop after Red
   - do not implement the feature
   - do not sneak Green work into test helpers

Red-phase checklist:
- the behavior is specified in executable tests
- tests are placed in the correct stack and layer
- failures are deterministic
- failures are caused by missing behavior, not unrelated infrastructure
- no production behavior was implemented
- any required scaffolding is minimal and still leaves tests failing

Bug-avoidance rules:
- do not write vague snapshot-style tests for core proof logic
- do not overfit tests to current implementation details unless the contract itself requires it
- do not add broad mocks that hide DB, validator, or orchestration behavior the test is supposed to prove
- do not leave tests failing due to sandbox quirks when a narrower deterministic seam exists

ChatDB-specific focus areas:
- schema and migration behavior should be tested at the DB boundary
- loop-engine behavior should be tested with the narrowest pure or integration seam available
- validator behavior should be tested against typed contracts, not prose expectations
- frontend tests should verify render/store behavior, not backend internals

Deliverable format:
1. What behavior the tests specify
2. Tests added or modified
3. Red evidence
   - commands run
   - failing output summarized
4. Assumptions or blockers

Quality bar:
- minimal but decisive failing tests
- no implementation drift
- no fake Red caused by unrelated setup breakage
- no transition to Green in the same pass unless explicitly requested
```

## One-Page Checklist

```text
Red-phase checklist for ChatDB

Goal:
- make the missing behavior explicit in failing tests

Before writing tests:
- inspect relevant code, schema, and existing tests
- identify the exact contract
- pick the narrowest correct test layer

Write tests that:
- are deterministic
- fail for the intended reason
- cover the main behavior
- cover at least one important edge or failure path where relevant

Avoid:
- implementation before tests
- overly broad integration tests when a unit or focused integration test is enough
- failures caused by broken setup instead of missing behavior
- mocks that bypass the real contract under test

Verify:
- run targeted tests
- confirm they fail
- confirm the failure proves the missing behavior

Output:
- behavior specified
- tests changed
- Red evidence
- blockers or assumptions
```

## Test-Spec Variant

Use this when you want a strict Red-only handoff with no implementation.

```text
Perform a strict Red-phase pass on ChatDB.

Rules:
- do not implement production behavior
- do not refactor production code unless needed to create a test seam
- any seam-creation change must be minimal and must leave the tests failing
- the output must prove the repo is red for the requested behavior

Required output:
1. Behavior contract
2. Tests added or modified
3. Exact failing evidence
4. Why the failures are the correct Red signal

Stop after the failing tests are in place.
```

## Suggested Verification Commands

```text
Rust:
- `cargo test <target>`

TypeScript / frontend:
- `npx tsc --noEmit`
- `npm test -- --run <target>`

Python:
- `python -m pytest <target>`
```

## Review Notes

- Red is successful only if the tests fail for the intended reason.
- A noisy environment failure is not good Red evidence unless the task is specifically about that environment contract.
- Prefer the smallest test set that forces the future Green implementation to be honest.
