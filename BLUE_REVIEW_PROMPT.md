# Blue Review Prompt

Reusable prompt templates for Blue-phase review, refactor, and bug-hunting passes in ChatDB.

Use this file when handing work to the next agent after Red and Green are already in place, or when you need a disciplined review of what is actually green versus what is only assumed to be green.

## Full Prompt

```text
You are performing a comprehensive Blue-phase code review and bug-hunting pass on ChatDB.

Mission:
- establish the real green baseline first
- separate environment and tooling failures from code defects
- identify correctness bugs, crash risks, and architectural violations
- implement only safe Blue-phase refactors or tightly scoped confirmed bug fixes
- re-run verification and report findings with evidence

Repository constraints:
- Follow AGENTS.md strictly.
- ChatDB architecture is non-negotiable:
  - SQLite and the DAG are the source of truth
  - Rust owns orchestration and DB writes
  - Python sidecar is stateless compute and validation
  - frontend observes state and never mutates core system state directly
- TDD order matters: Red -> Green -> Blue
- If you cannot establish green, document why and do not pretend a repo-wide Blue phase happened.

Primary review targets:
- `src-tauri/src/db/*`
- `src-tauri/src/loop_engine/*`
- `src-tauri/src/main.rs`
- `sidecar/src/validation/*`
- `sidecar/src/mcp/*`
- `src/services/*`
- `src/stores/*`

Known high-risk themes:
- panic paths in production Rust code (`unwrap`, `expect`)
- mutex poisoning and DB lock handling
- startup and bootstrap hard failures
- duplicated prompt logic that can silently drift
- validator availability checks that lie about runtime readiness
- test and lint scripts that are broken as configured
- sandbox or environment issues that masquerade as code failures
- warning-producing code that signals interface drift or dead paths

Required process:
1. Baseline
   - inspect repo state
   - identify all test, build, and lint entry points
   - run the strongest feasible verification in Rust, Python, and TypeScript
2. Classify failures
   - true product bug
   - flaky or incomplete test
   - infra or sandbox problem
   - repo configuration problem
3. Review for correctness first
   - crashes
   - data integrity
   - invalid state transitions
   - validator false positives or false negatives
   - broken tool contracts
4. Review for Blue-phase cleanup second
   - extract duplicated logic
   - tighten visibility
   - remove warnings
   - improve error propagation
   - preserve behavior
5. Implement only the smallest justified set of changes
6. Re-run focused verification
7. Report findings first, then changes

Mandatory searches:
- `unwrap(` and `expect(` in non-test production code
- warning-producing imports, types, and functions
- duplicated prompt sections in solver code
- subprocess environment assumptions in Lean and validator code
- missing or invalid ESLint or Vitest config
- temp-dir and path handling in Python tests
- direct architectural boundary violations

Decision rules:
- If a failure is caused by the environment, do not "fix" product code to paper over it unless the product code is making a false claim about availability or readiness.
- If a production path panics, treat that as a real finding even if tests pass.
- If a build, lint, or test gate is declared in `package.json` but cannot run as configured, treat that as a real finding.
- If a refactor is not clearly behavior-preserving, do not do it in Blue phase.

Deliverable format:
1. Findings
   - ordered by severity
   - every item includes file and line references
   - explain impact and why it matters
2. Open questions or assumptions
3. Changes made
   - concise
   - only landed changes
4. Verification
   - exact commands run
   - what passed
   - what failed
   - what was blocked
5. Remaining blockers to a true repo-wide Blue phase

Quality bar:
- concise, not vague
- findings before summary
- no generic advice
- every claim tied to evidence
- do not say "looks good overall" if key gates are broken
```

## One-Page Checklist

Use this when you want the shortest possible review brief without losing discipline.

```text
Blue-phase review checklist for ChatDB

Goal:
- verify what is actually green
- hunt bugs and crash risks
- apply only safe, behavior-preserving refactors

Check baseline:
- inspect repo state
- identify Rust, Python, and TypeScript test and lint entry points
- run the strongest feasible checks

Classify failures:
- product bug
- test bug
- config bug
- environment or sandbox blocker

Review priority:
- production panics (`unwrap`, `expect`)
- DB locking and error propagation
- startup and bootstrap failure paths
- validator correctness and runtime readiness
- prompt-builder duplication and interface drift
- broken lint, test, or build scripts
- architecture boundary violations

Hot paths:
- `src-tauri/src/db/*`
- `src-tauri/src/loop_engine/*`
- `src-tauri/src/main.rs`
- `sidecar/src/validation/*`
- `sidecar/src/mcp/*`
- `src/services/*`
- `src/stores/*`

Blue-phase rules:
- no new features
- no speculative redesign
- no behavior changes unless fixing a confirmed bug
- refactors must be verified

Always search for:
- `unwrap(`
- `expect(`
- compiler warnings
- unused imports
- duplicated logic
- stale availability checks
- broken config files

Output:
- findings first, severity ordered, file and line cited
- then assumptions
- then landed changes
- then verification results
- then remaining blockers
```

## Findings-Only Variant

Use this when you want a strict review with no code changes unless a blocker makes that unavoidable.

```text
Perform a findings-only Blue-phase review of ChatDB.

Rules:
- do not make code changes by default
- focus on identifying bugs, risks, regressions, and broken quality gates
- establish the real green baseline first
- distinguish code defects from environment and tooling failures
- findings must be the primary output

Required review focus:
- production panic paths
- DB integrity and locking risks
- validator correctness
- startup and bootstrap robustness
- test, lint, and build gates that cannot succeed as configured
- architecture violations against AGENTS.md

Mandatory evidence:
- every finding must include file and line references
- every finding must explain impact
- verification commands must be listed
- blocked checks must be called out explicitly

Output format:
1. Findings
   - ordered by severity
   - file and line cited
   - impact explained
2. Open questions or assumptions
3. Verification summary
4. Residual risks

Do not pad the review with compliments or generic summaries.
If no findings are discovered, say that explicitly and list residual testing gaps.
```

## Suggested Verification Commands

Use the strongest feasible subset based on the current environment.

```text
Rust:
- `cargo test`
- `cargo fmt --check`

TypeScript / frontend:
- `npx tsc --noEmit`
- `npm run lint`
- `npm test -- --run`

Python:
- `python -m pytest`
- `python -m pytest tests/test_lean_validator.py`
- targeted pytest runs for failing areas
```

## Review Notes

- A repo-wide Blue phase is only real if the meaningful quality gates are green or the blockers are explicitly documented.
- Environment failures still matter when product code misreports them as correctness failures.
- The highest-value Blue work in this repo is usually in error handling, runtime readiness checks, warning cleanup, and duplication reduction in the orchestration path.
