# ChatDB Remediation Task Map (Audit Sweeps 002)

## Plan Metadata

- Run ID: `run-remediation-task-map-001`
- Task ID: `remediation-task-map-001`
- Source of findings: `docs/code-review-audit-sweeps.md`
- Scope boundary for this planning artifact: `docs/` with `*.md` only
- Execution policy: strict TDD (`red-phase` -> `green-phase` -> `blue-phase`) with Gitflow
- Gitflow base branch for all packages: `develop`

## Parsed Finding IDs (P0/P1/P2)

### P0 (release blockers)
1. `BG-AUDIT-SWEEPS-002-SEC-001`
2. `BG-AUDIT-SWEEPS-002-SEC-002`
3. `BG-AUDIT-SWEEPS-002-REL-001`
4. `BG-AUDIT-SWEEPS-002-DB-001`

### P1 (stability hardening)
1. `BG-AUDIT-SWEEPS-002-REL-002`
2. `BG-AUDIT-SWEEPS-002-DB-002`
3. `BG-AUDIT-SWEEPS-002-PRF-001`
4. `BG-AUDIT-SWEEPS-002-TST-001`

### P2 (quality and contract completeness)
1. `BG-AUDIT-SWEEPS-002-API-001`
2. `BG-AUDIT-SWEEPS-002-API-002`
3. `BG-AUDIT-SWEEPS-002-OBS-001`
4. `BG-AUDIT-SWEEPS-002-SEC-003`

## Coverage Matrix (Every Finding Mapped)

| finding_id | severity | mapped_task_id |
|---|---|---|
| BG-AUDIT-SWEEPS-002-SEC-001 | P0 | rm-wp-01-sec-parser-boundary |
| BG-AUDIT-SWEEPS-002-SEC-002 | P0 | rm-wp-02-sql-status-and-tx |
| BG-AUDIT-SWEEPS-002-REL-001 | P0 | rm-wp-03-conclusion-fail-closed |
| BG-AUDIT-SWEEPS-002-DB-001 | P0 | rm-wp-02-sql-status-and-tx |
| BG-AUDIT-SWEEPS-002-REL-002 | P1 | rm-wp-04-challenger-persistence-budget |
| BG-AUDIT-SWEEPS-002-DB-002 | P1 | rm-wp-04-challenger-persistence-budget |
| BG-AUDIT-SWEEPS-002-PRF-001 | P1 | rm-wp-04-challenger-persistence-budget |
| BG-AUDIT-SWEEPS-002-TST-001 | P1 | rm-wp-05-rust-regression-test-baseline |
| BG-AUDIT-SWEEPS-002-API-001 | P2 | rm-wp-06-api-envelope-and-analytics |
| BG-AUDIT-SWEEPS-002-API-002 | P2 | rm-wp-06-api-envelope-and-analytics |
| BG-AUDIT-SWEEPS-002-OBS-001 | P2 | rm-wp-07-sidecar-structured-logging |
| BG-AUDIT-SWEEPS-002-SEC-003 | P2 | rm-wp-08-shell-least-privilege |

## Gitflow Branch Plan

- Base branch: `develop`
- Feature branch naming convention: `feature/<task-id>-<short-kebab-description>`
- Package branch assignments:

| task_id | feature branch |
|---|---|
| rm-wp-01-sec-parser-boundary | `feature/rm-wp-01-sec-parser-boundary` |
| rm-wp-02-sql-status-and-tx | `feature/rm-wp-02-sql-status-and-tx` |
| rm-wp-03-conclusion-fail-closed | `feature/rm-wp-03-conclusion-fail-closed` |
| rm-wp-04-challenger-persistence-budget | `feature/rm-wp-04-challenger-persistence-budget` |
| rm-wp-05-rust-regression-test-baseline | `feature/rm-wp-05-rust-regression-test-baseline` |
| rm-wp-06-api-envelope-and-analytics | `feature/rm-wp-06-api-envelope-and-analytics` |
| rm-wp-07-sidecar-structured-logging | `feature/rm-wp-07-sidecar-structured-logging` |
| rm-wp-08-shell-least-privilege | `feature/rm-wp-08-shell-least-privilege` |

## Ordered Work Packages (Dependency-Aware)

> Mode sequence for every package is fixed: `red-phase` -> `green-phase` -> `blue-phase`.

### 1) rm-wp-01-sec-parser-boundary

- objective: Eliminate unsafe symbolic parsing path and enforce allowlisted parser boundary.
- mode: `red-phase` -> `green-phase` -> `blue-phase`
- severity: P0
- finding_id(s): `BG-AUDIT-SWEEPS-002-SEC-001`
- dependencies: []
- parallelizable: true
- safe_file_scope:
  - red-phase:
    - workspace_path: `sidecar/tests/`
    - file_patterns: `test_sympy_sandbox*.py`, `test_validators.py`
  - green/blue-phase:
    - workspace_path: `sidecar/src/validation/`
    - file_patterns: `router.py`, `sympy_validator.py`, `safe_*.py`
- acceptance_criteria:
  - red-phase:
    - Hostile payload tests fail deterministically for parser safety gap.
    - Failure evidence recorded with command and failing assertion signatures.
  - green-phase:
    - Allowlist/strict parser path implemented; hostile payload tests pass.
    - Existing validator compatibility tests remain green.
  - blue-phase:
    - Parsing pipeline simplified (deduplicated token validation, clear error taxonomy).
    - All targeted and regression tests green post-refactor.
  - standing_order:
    - Any touched file must include or update a local TDD enforcement prompt comment/tag appropriate to file language; for comment-hostile formats, update a sibling provenance tag file.
- blue_phase_opportunities:
  - Consolidate parsing guardrails into reusable helper(s).
  - Normalize parser error messages for observability and triage.

### 2) rm-wp-02-sql-status-and-tx

- objective: Remove SQL interpolation risk and transactionalize step/counter write bundle.
- mode: `red-phase` -> `green-phase` -> `blue-phase`
- severity: P0
- finding_id(s): `BG-AUDIT-SWEEPS-002-SEC-002`, `BG-AUDIT-SWEEPS-002-DB-001`
- dependencies: []
- parallelizable: true
- safe_file_scope:
  - red-phase:
    - workspace_path: `src-tauri/`
    - file_patterns: `tests/**/*problem*`, `tests/**/*db*`, `src/**/mod.rs` (test modules only)
  - green/blue-phase:
    - workspace_path: `src-tauri/src/`
    - file_patterns: `api/commands/problem.rs`, `db/mod.rs`
- acceptance_criteria:
  - red-phase:
    - Injection-attempt status tests fail against current behavior.
    - Partial-write simulation tests fail when write bundle is non-transactional.
  - green-phase:
    - Parameterized query + strict status enum validation implemented.
    - `record_step` write bundle executes atomically in single transaction.
  - blue-phase:
    - Query and transaction code paths refactored for shared helper reuse and clearer errors.
    - Targeted DB + command contract tests remain green.
  - standing_order:
    - Any touched file must include or update a local TDD enforcement prompt comment/tag appropriate to file language; for comment-hostile formats, update a sibling provenance tag file.
- blue_phase_opportunities:
  - Extract typed status parsing utility for other command handlers.
  - Reduce duplicate SQL boilerplate and centralize transaction boundary handling.

### 3) rm-wp-03-conclusion-fail-closed

- objective: Remove fail-open conclusion acceptance path and require explicit reviewer success or bounded degraded-mode gate.
- mode: `red-phase` -> `green-phase` -> `blue-phase`
- severity: P0
- finding_id(s): `BG-AUDIT-SWEEPS-002-REL-001`
- dependencies: [rm-wp-02-sql-status-and-tx]
- parallelizable: false
- safe_file_scope:
  - red-phase:
    - workspace_path: `src-tauri/`
    - file_patterns: `tests/**/*loop*`, `tests/**/*review*`
  - green/blue-phase:
    - workspace_path: `src-tauri/src/loop_engine/`
    - file_patterns: `mod.rs`, `review.rs`
- acceptance_criteria:
  - red-phase:
    - Tests prove reviewer-failure path currently accepts invalid conclusion (expected fail).
  - green-phase:
    - Reviewer failure now fails closed or enters explicitly signaled bounded degraded mode.
    - Deterministic retry policy enforced with observable terminal states.
  - blue-phase:
    - Reviewer gate logic refactored for readability and reduced branch complexity.
    - All loop gate tests remain green with no behavior drift.
  - standing_order:
    - Any touched file must include or update a local TDD enforcement prompt comment/tag appropriate to file language; for comment-hostile formats, update a sibling provenance tag file.
- blue_phase_opportunities:
  - Extract reviewer decision state machine helper.
  - Improve diagnostics payload ergonomics for failure gate analysis.

### 4) rm-wp-04-challenger-persistence-budget

- objective: Harden challenger outage handling, enforce persistence ID integrity, and enforce max total cost in runtime loop.
- mode: `red-phase` -> `green-phase` -> `blue-phase`
- severity: P1
- finding_id(s): `BG-AUDIT-SWEEPS-002-REL-002`, `BG-AUDIT-SWEEPS-002-DB-002`, `BG-AUDIT-SWEEPS-002-PRF-001`
- dependencies: [rm-wp-03-conclusion-fail-closed]
- parallelizable: false
- safe_file_scope:
  - red-phase:
    - workspace_path: `src-tauri/`
    - file_patterns: `tests/**/*challenger*`, `tests/**/*persistence*`, `tests/**/*budget*`
  - green/blue-phase:
    - workspace_path: `src-tauri/src/`
    - file_patterns: `loop_engine/mod.rs`, `models/agents.rs`, `api/commands/loop_cmd.rs`
- acceptance_criteria:
  - red-phase:
    - Failing tests demonstrate challenger outage survival without controls, ID defaulting on persistence failures, and budget overrun.
  - green-phase:
    - Challenger retries + downgrade/reject policy implemented with deterministic failure budget.
    - Persistence failures become hard errors; empty IDs rejected before dependent writes.
    - Cumulative spend enforcement stops loop at max total cost with diagnostic event.
  - blue-phase:
    - Shared resiliency utilities deduplicated across reviewer/challenger paths.
    - Runtime diagnostics naming normalized; all new and existing tests green.
  - standing_order:
    - Any touched file must include or update a local TDD enforcement prompt comment/tag appropriate to file language; for comment-hostile formats, update a sibling provenance tag file.
- blue_phase_opportunities:
  - Unify retry/backoff primitives for reviewer + challenger.
  - Consolidate cost accounting/reporting into a single helper for observability.

### 5) rm-wp-05-rust-regression-test-baseline

- objective: Establish durable Rust test baseline for loop invariants, DB atomicity, and command error contracts.
- mode: `red-phase` -> `green-phase` -> `blue-phase`
- severity: P1
- finding_id(s): `BG-AUDIT-SWEEPS-002-TST-001`
- dependencies: [rm-wp-04-challenger-persistence-budget]
- parallelizable: false
- safe_file_scope:
  - red-phase:
    - workspace_path: `src-tauri/`
    - file_patterns: `tests/**/*.rs`
  - green/blue-phase:
    - workspace_path: `src-tauri/`
    - file_patterns: `tests/**/*.rs`, `src/**/mod.rs` (test-only module wiring)
- acceptance_criteria:
  - red-phase:
    - Missing-coverage tests added and fail for at least loop gates, DB atomicity, and command contract regressions.
  - green-phase:
    - Baseline suite passes and is wired into default CI/local check command set.
  - blue-phase:
    - Test fixtures/helpers deduplicated; flaky patterns removed; execution time documented.
  - standing_order:
    - Any touched file must include or update a local TDD enforcement prompt comment/tag appropriate to file language; for comment-hostile formats, update a sibling provenance tag file.
- blue_phase_opportunities:
  - Build reusable test harness factory for DB + loop integration tests.
  - Improve assertion ergonomics and failure message clarity.

### 6) rm-wp-06-api-envelope-and-analytics

- objective: Implement typed IPC error envelope and replace placeholder analytics counters with real DB-backed values or explicit deprecation.
- mode: `red-phase` -> `green-phase` -> `blue-phase`
- severity: P2
- finding_id(s): `BG-AUDIT-SWEEPS-002-API-001`, `BG-AUDIT-SWEEPS-002-API-002`
- dependencies: [rm-wp-02-sql-status-and-tx, rm-wp-05-rust-regression-test-baseline]
- parallelizable: true
- safe_file_scope:
  - red-phase:
    - workspace_path: `src-tauri/`
    - file_patterns: `tests/**/*api*`, `tests/**/*analytics*`
  - green/blue-phase:
    - workspace_path: `src-tauri/src/`
    - file_patterns: `api/commands/problem.rs`, `api/commands/analytics.rs`, `api/commands/mod.rs`, `models/agents.rs`
  - frontend contract validation:
    - workspace_path: `src/`
    - file_patterns: `types/index.ts`, `services/tauri.ts`
- acceptance_criteria:
  - red-phase:
    - Contract tests fail on string-only error mapping and placeholder analytics fields.
  - green-phase:
    - Typed error envelope (`code`, `kind`, `retryable`, context payload) implemented end-to-end.
    - Analytics fields are DB-backed or explicitly deprecated with machine-actionable contract semantics.
  - blue-phase:
    - Shared command error mapping deduplicated and documented.
    - Frontend typing ergonomics improved without contract drift.
  - standing_order:
    - Any touched file must include or update a local TDD enforcement prompt comment/tag appropriate to file language; for comment-hostile formats, update a sibling provenance tag file.
- blue_phase_opportunities:
  - Create unified IPC error conversion utility.
  - Align analytics type names across Rust and TypeScript for lower translation overhead.

### 7) rm-wp-07-sidecar-structured-logging

- objective: Replace sidecar lifecycle `print(...)` calls with structured logging including correlation identifiers.
- mode: `red-phase` -> `green-phase` -> `blue-phase`
- severity: P2
- finding_id(s): `BG-AUDIT-SWEEPS-002-OBS-001`
- dependencies: [rm-wp-01-sec-parser-boundary]
- parallelizable: true
- safe_file_scope:
  - red-phase:
    - workspace_path: `sidecar/tests/`
    - file_patterns: `test_*logging*.py`, `test_lean_validator.py`
  - green/blue-phase:
    - workspace_path: `sidecar/src/`
    - file_patterns: `main.py`, `validation/lean_validator.py`
- acceptance_criteria:
  - red-phase:
    - Tests fail when lifecycle events are unstructured or missing correlation fields.
  - green-phase:
    - Structured logger emits `run_id`, `attempt_id`, component, severity for warmup/lifecycle flows.
  - blue-phase:
    - Logging helpers centralized and message schema normalized across sidecar modules.
    - Logging tests and validator tests remain green.
  - standing_order:
    - Any touched file must include or update a local TDD enforcement prompt comment/tag appropriate to file language; for comment-hostile formats, update a sibling provenance tag file.
- blue_phase_opportunities:
  - Create shared sidecar logging adapter.
  - Reduce log noise while improving signal density for operational dashboards.

### 8) rm-wp-08-shell-least-privilege

- objective: Remove broad shell privileges and constrain shell execution to explicit allowlisted commands.
- mode: `red-phase` -> `green-phase` -> `blue-phase`
- severity: P2
- finding_id(s): `BG-AUDIT-SWEEPS-002-SEC-003`
- dependencies: [rm-wp-05-rust-regression-test-baseline]
- parallelizable: true
- safe_file_scope:
  - red-phase:
    - workspace_path: `src-tauri/`
    - file_patterns: `tests/**/*capability*`, `tests/**/*shell*`
  - green/blue-phase:
    - workspace_path: `src-tauri/`
    - file_patterns: `capabilities/default.json`, `src/main.rs`
- acceptance_criteria:
  - red-phase:
    - Capability tests fail when broad shell permissions are present.
  - green-phase:
    - Default capability no longer grants unrestricted shell spawn/execute.
    - Any retained shell behavior is constrained by explicit allowlist checks.
  - blue-phase:
    - Capability docs and command wiring are simplified and aligned to least-privilege model.
    - Security regression tests remain green.
  - standing_order:
    - Any touched file must include or update a local TDD enforcement prompt comment/tag appropriate to file language; for comment-hostile formats, update a sibling provenance tag file.
- blue_phase_opportunities:
  - Consolidate capability policy constants.
  - Improve operator ergonomics with clear capability violation diagnostics.

## Dependency Graph (Topological View)

1. `rm-wp-01-sec-parser-boundary` (parallel lane A)
2. `rm-wp-02-sql-status-and-tx` (parallel lane B)
3. `rm-wp-03-conclusion-fail-closed` depends on `rm-wp-02-sql-status-and-tx`
4. `rm-wp-04-challenger-persistence-budget` depends on `rm-wp-03-conclusion-fail-closed`
5. `rm-wp-05-rust-regression-test-baseline` depends on `rm-wp-04-challenger-persistence-budget`
6. `rm-wp-06-api-envelope-and-analytics` depends on `rm-wp-02-sql-status-and-tx` and `rm-wp-05-rust-regression-test-baseline`
7. `rm-wp-07-sidecar-structured-logging` depends on `rm-wp-01-sec-parser-boundary`
8. `rm-wp-08-shell-least-privilege` depends on `rm-wp-05-rust-regression-test-baseline`

## Rollout Sequence and Stop/Go Gates

### Stage 0 - Intake and branch prep
- Create all feature branches from `develop`.
- Confirm package scopes are non-overlapping for any parallel execution window.
- Stop/Go Gate G0:
  - **Go** if scope map + dependency order is acknowledged by implementers.
  - **Stop** if any package has ambiguous file ownership.

### Stage 1 - P0 closure
1. Execute `rm-wp-01-sec-parser-boundary` and `rm-wp-02-sql-status-and-tx` in parallel lanes.
2. Execute `rm-wp-03-conclusion-fail-closed` after `rm-wp-02-sql-status-and-tx`.

Stop/Go Gate G1 (after each package in Stage 1):
- **Go** only if package has complete Red evidence, Green pass evidence, Blue refactor evidence, and no open Critical/P0 finding mapped to that package.
- **Stop** if any Red evidence is missing, if Green is partial, or if Blue introduces behavior drift.

### Stage 2 - P1 stabilization
1. Execute `rm-wp-04-challenger-persistence-budget`.
2. Execute `rm-wp-05-rust-regression-test-baseline`.

Stop/Go Gate G2 (after each package in Stage 2):
- **Go** only if reliability and budget controls are deterministic and regression suite remains green.
- **Stop** on any fail-open path, persistence integrity regression, or budget enforcement bypass.

### Stage 3 - P2 quality and policy hardening
1. Execute `rm-wp-06-api-envelope-and-analytics`, `rm-wp-07-sidecar-structured-logging`, and `rm-wp-08-shell-least-privilege` with dependency-respected parallelization.

Stop/Go Gate G3 (after each package in Stage 3):
- **Go** only if contract/schema changes are version-safe and observability/security regressions are absent.
- **Stop** on breaking IPC contract drift, missing correlation metadata, or broadened capability permissions.

### Final Gate - Release readiness

Stop/Go Gate G4:
- **Go** when all package acceptance criteria are met, all mapped findings are closed, and all required TDD evidence is attached to PRs.
- **Stop** if any mapped finding remains unresolved or any package lacks phase-complete evidence.

