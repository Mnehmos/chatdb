# Green Phase Prompt

Reusable prompt templates for Green-phase implementation passes in ChatDB.

Use this file when the next agent's job is to make Red tests pass with the smallest correct implementation, while preserving architecture and avoiding premature refactors.

## Full Prompt

```text
You are performing a Green-phase pass on ChatDB.

Mission:
- start from failing Red tests
- implement the smallest correct production change that makes the tests pass
- preserve ChatDB architecture and contracts
- avoid opportunistic refactors
- prove the targeted scope is green

Repository constraints:
- Follow AGENTS.md strictly.
- ChatDB architecture is non-negotiable:
  - SQLite and the DAG are the source of truth
  - Rust owns orchestration and DB writes
  - Python sidecar is stateless compute and validation
  - frontend observes state and never mutates core system state directly
- TDD order matters: Red -> Green -> Blue
- Green means minimal implementation to satisfy failing tests.
- Refactoring belongs to Blue unless required for correctness.

Primary Green-phase goal:
- make the failing tests pass with the narrowest correct change

Required process:
1. Re-read the Red tests
   - treat them as the contract
   - do not broaden scope unless the tests expose a real supporting requirement
2. Identify the smallest implementation seam
   - prefer localized changes
   - do not redesign subsystems
3. Implement the minimum correct production code
   - preserve existing behavior outside the tested scope
   - keep error handling honest
   - avoid speculative abstractions
4. Run the targeted tests until green
5. Run a small safety net around touched code
   - nearby tests
   - typecheck or compile checks
6. Stop after Green
   - do not start a cleanup refactor pass unless explicitly requested

Green-phase checklist:
- Red tests now pass
- implementation is minimal
- no unrelated behavior was changed
- no speculative abstractions were introduced
- architecture boundaries are preserved
- touched code compiles or typechecks
- no obvious nearby regressions were introduced

Bug-avoidance rules:
- do not solve more than the tests require unless required for correctness
- do not hide failures with loose assertions, broad catches, or over-mocking
- do not move business logic into the wrong layer just because it is expedient
- do not convert Green into Blue by extracting helpers or redesigning modules unless strictly necessary

ChatDB-specific focus areas:
- DB state must remain authoritative
- validator endpoints should stay typed and deterministic
- loop-engine changes should preserve obligation-driven execution
- frontend should consume typed state and events rather than invent state locally

Deliverable format:
1. What was implemented
2. Why that is the minimal change
3. Verification
   - commands run
   - tests now passing
   - any broader checks run
4. Deferred cleanup for Blue phase

Quality bar:
- smallest correct implementation
- targeted, verified, and architecture-safe
- no hidden refactor disguised as Green
```

## One-Page Checklist

```text
Green-phase checklist for ChatDB

Goal:
- make the Red tests pass with the smallest correct implementation

Before coding:
- reread the failing tests
- identify the narrowest production change
- keep scope fixed

While coding:
- implement only what the tests require
- preserve architecture boundaries
- avoid cleanup refactors
- avoid speculative abstractions

Verify:
- targeted tests now pass
- run nearby compile, typecheck, or adjacent tests
- confirm no obvious regressions in touched areas

Avoid:
- redesigning modules
- broad error swallowing
- moving logic into the wrong layer
- mixing Blue cleanup into Green

Output:
- implementation summary
- why it is minimal
- verification results
- cleanup deferred to Blue
```

## Minimal-Implementation Variant

Use this when you want a strict Green-only handoff.

```text
Perform a strict Green-phase pass on ChatDB.

Rules:
- start from existing failing Red tests
- implement the minimum production change needed to make them pass
- do not refactor unless required for correctness
- do not expand scope beyond the tested contract

Required output:
1. Minimal production changes made
2. Red tests now passing
3. Extra safety checks run
4. Cleanup explicitly deferred to Blue

Stop after the targeted scope is green.
```

## Suggested Verification Commands

```text
Rust:
- `cargo test <target>`
- `cargo test`

TypeScript / frontend:
- `npx tsc --noEmit`
- `npm test -- --run <target>`

Python:
- `python -m pytest <target>`
- focused pytest runs around touched modules
```

## Review Notes

- Green is not "the feature seems implemented"; Green is "the Red tests now pass."
- If a broader change feels tempting, defer it to Blue unless the code is otherwise incorrect.
- Minimality matters because it isolates correctness from cleanup.
