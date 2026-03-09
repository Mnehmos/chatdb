# ChatDB Code Review Audit Sweeps (Post TDD+Gitflow Redesign)

- Run ID: `run-code-review-audits-002`
- Task ID: `audit-sweeps-002`
- Rubric used: `docs/code-review-checklist-chatdb.md`
- Review mode: static code and document audit only
- Scope boundary: `./` with `*.rs`, `*.py`, `*.ts`, `*.tsx`, `*.md`

## Executive Summary

**Overall audit verdict: Fail** because one unresolved Critical finding remains.

- Sweeps executed: **5 of 5**
- Findings: **12 total**
  - Critical: **1**
  - High: **7**
  - Medium: **4**
- Primary blockers:
  1. Unsandboxed symbolic parsing path can evaluate hostile input.
  2. SQL interpolation at IPC boundary can enable query injection.
  3. Fail open acceptance paths can degrade correctness when reviewer or challenger calls fail.

---

## Sweep 1 - Security and secrets handling

### Scope scanned
- `sidecar/src/validation/router.py`
- `sidecar/src/validation/sympy_validator.py`
- `src-tauri/src/api/commands/problem.rs`
- `src-tauri/src/db/mod.rs`
- `src-tauri/capabilities/default.json`
- `src-tauri/src/main.rs`
- `.gitignore`

### Checklist score summary
| Control | Yes | No | NA |
|---|---:|---:|---:|
| SEC-01 Trust boundaries validated | 0 | 1 | 0 |
| SEC-02 Secrets handling | 1 | 0 | 0 |
| SEC-03 Injection and unsafe deserialization prevention | 0 | 1 | 0 |
| SEC-04 Two person rule for sensitive operations | 0 | 0 | 1 |

### Findings

#### BG-AUDIT-SWEEPS-002-SEC-001 (Critical)
- Rule violated: SEC-03
- Impact: Potential code execution through unsandboxed symbolic parse path.
- Evidence:
  - `sidecar/src/validation/router.py:74-75` routes untrusted `formal` input to SymPy validator.
  - `sidecar/src/validation/sympy_validator.py:38` calls `parse_expr(...)` on that input.
- Remediation actions:
  - Replace free form parser path with strict grammar or allowlist parser.
  - Enforce token and type allowlist before parse.
  - Add hostile payload tests under sidecar validation tests.

#### BG-AUDIT-SWEEPS-002-SEC-002 (High)
- Rule violated: SEC-03 and SEC-01
- Impact: SQL injection surface at frontend to IPC to DB boundary.
- Evidence:
  - `src-tauri/src/api/commands/problem.rs:23-27` forwards caller controlled `status`.
  - `src-tauri/src/db/mod.rs:186-190` builds SQL using `format!(...)` interpolation.
- Remediation actions:
  - Parameterize status query.
  - Validate status against strict enum before DB call.

#### BG-AUDIT-SWEEPS-002-SEC-003 (High)
- Rule violated: SEC-01
- Impact: Broad shell execution permission increases blast radius if IPC path is compromised.
- Evidence:
  - `src-tauri/capabilities/default.json:7-9` includes `shell:allow-spawn` and `shell:allow-execute`.
  - `src-tauri/src/main.rs:37` enables shell plugin.
- Remediation actions:
  - Remove broad shell permissions from default capability.
  - Move shell use to explicit allowlisted commands only.

### Sweep verdict
**Fail** due to unresolved Critical finding.

---
## Sweep 2 - Reliability and concurrency plus failure handling

### Scope scanned
- `src-tauri/src/loop_engine/mod.rs`
- `src-tauri/src/api/sidecar.rs`
- `src-tauri/src/api/llm_client.rs`
- `src-tauri/src/api/commands/loop_cmd.rs`

### Checklist score summary
| Control | Yes | No | NA |
|---|---:|---:|---:|
| REL-01 Cancellation retry idempotency explicit | 1 | 1 | 0 |
| REL-02 Shared resource race safety | 1 | 0 | 0 |
| REL-03 Timeout error backoff observable | 0 | 1 | 0 |
| REL-04 Deadlock and blocking risk | 1 | 0 | 0 |

### Findings

#### BG-AUDIT-SWEEPS-002-REL-001 (High)
- Rule violated: REL-03
- Impact: Conclusion can be accepted when reviewer call fails, creating correctness fail open behavior.
- Evidence:
  - `src-tauri/src/loop_engine/mod.rs:821-825` accepts conclusion after reviewer failure.
- Remediation actions:
  - Switch conclusion acceptance to fail closed when reviewer call fails.
  - Or require bounded retry and explicit degraded mode signal before acceptance.

#### BG-AUDIT-SWEEPS-002-REL-002 (High)
- Rule violated: REL-03
- Impact: Verified step can survive challenger outage with no compensating control.
- Evidence:
  - `src-tauri/src/loop_engine/mod.rs:1209-1213` allows step survival when challenger call fails.
- Remediation actions:
  - Add retry policy plus confidence downgrade or reject path after challenger failure budget is exceeded.

#### BG-AUDIT-SWEEPS-002-REL-003 (Medium)
- Rule violated: REL-03
- Impact: HTTP client builder failure silently falls back to default client configuration.
- Evidence:
  - `src-tauri/src/api/sidecar.rs:13-17` uses `.build().unwrap_or_default()`.
  - `src-tauri/src/api/llm_client.rs:53-57` uses `.build().unwrap_or_default()`.
- Remediation actions:
  - Propagate builder errors and fail startup early with explicit diagnostics.

### Sweep verdict
**Conditional Pass** (no Critical, unresolved High findings).

---

## Sweep 3 - Database invariants and state integrity

### Scope scanned
- `src-tauri/src/db/mod.rs`
- `src-tauri/src/db/dag_events.rs`
- `src-tauri/src/db/proof_nodes.rs`
- `src-tauri/src/loop_engine/mod.rs`

### Checklist score summary
| Control | Yes | No | NA |
|---|---:|---:|---:|
| DB-01 DB remains source of truth | 1 | 0 | 0 |
| DB-02 Invariants preserved | 0 | 1 | 0 |
| DB-03 Migration safety | 0 | 0 | 1 |
| DB-04 Atomic and error propagating writes | 0 | 1 | 0 |

### Findings

#### BG-AUDIT-SWEEPS-002-DB-001 (High)
- Rule violated: DB-04
- Impact: Partial write risk where `steps` insert can succeed but counters can fail due non transactional sequence.
- Evidence:
  - `src-tauri/src/db/mod.rs:271-297` performs three dependent writes outside a transaction.
- Remediation actions:
  - Wrap `record_step` insert and dependent counter updates in a single SQL transaction.

#### BG-AUDIT-SWEEPS-002-DB-002 (High)
- Rule violated: DB-02 and DB-04
- Impact: Silent persistence failures can produce missing or empty linkage IDs in DAG provenance.
- Evidence:
  - `src-tauri/src/loop_engine/mod.rs:550,563,855,868` uses `unwrap_or_default()` on persisted IDs.
  - `src-tauri/src/loop_engine/mod.rs:645,799` ignores `record_step` errors with `let _ = ...`.
- Remediation actions:
  - Make write failures hard errors for step node event persistence.
  - Reject empty IDs before creating dependent records.

### Sweep verdict
**Conditional Pass** (no Critical, unresolved High findings).

---
## Sweep 4 - API contracts and boundary enforcement (Rust to Python to Frontend)

### Scope scanned
- `src/services/tauri.ts`
- `src/types/index.ts`
- `src/App.tsx`
- `src-tauri/src/api/commands/problem.rs`
- `src-tauri/src/api/commands/analytics.rs`
- `sidecar/src/validation/router.py`

### Checklist score summary
| Control | Yes | No | NA |
|---|---:|---:|---:|
| API-01 Command contracts mirrored in frontend | 1 | 0 | 0 |
| API-02 Sidecar contract compatibility | 1 | 0 | 0 |
| API-03 Error semantics machine actionable | 0 | 1 | 0 |
| API-04 Consumer impact and migration notes | 0 | 1 | 0 |

### Findings

#### BG-AUDIT-SWEEPS-002-API-001 (Medium)
- Rule violated: API-03
- Impact: Structured failures collapse into free text across IPC boundary, reducing automated handling quality.
- Evidence:
  - `src-tauri/src/api/commands/problem.rs:11,19,27` uses `map_err(|e| e.to_string())`.
  - Same pattern appears broadly in command handlers.
- Remediation actions:
  - Introduce typed error envelope with fields like `code`, `kind`, `retryable`, and contextual payload.

#### BG-AUDIT-SWEEPS-002-API-002 (Medium)
- Rule violated: API-04
- Impact: Analytics contract exposes multi layer counters but currently returns placeholder zeros for several layers.
- Evidence:
  - `src-tauri/src/api/commands/analytics.rs:15-21` sets non step metrics to `0`.
- Remediation actions:
  - Implement real counters from DB tables or deprecate fields until live.

### Sweep verdict
**Pass** (no unresolved High or Critical findings).

---

## Sweep 5 - Test quality plus observability and performance signals

### Scope scanned
- `src-tauri/src/**`
- `sidecar/tests/**`
- `src/components/diagnostics/DiagnosticPanel.tsx`
- `src/stores/diagnosticStore.ts`
- `src-tauri/src/models/agents.rs`
- `src-tauri/src/loop_engine/mod.rs`

### Checklist score summary
| Control area | Yes | No | NA |
|---|---:|---:|---:|
| TST (TST-01..04) | 0 | 2 | 2 |
| OBS (OBS-01..04) | 1 | 1 | 2 |
| PRF (PRF-01..04) | 1 | 1 | 2 |

### Findings

#### BG-AUDIT-SWEEPS-002-TST-001 (High)
- Rule violated: TST-01 and TST-02
- Impact: Core Rust orchestration and DB invariants have no detected Rust test coverage in `src-tauri/src`, raising regression risk.
- Evidence:
  - Static search found no `#[test]` or `#[tokio::test]` within Rust source scope.
- Remediation actions:
  - Add Rust unit and integration tests for loop gates, DB atomicity, and command error contracts.

#### BG-AUDIT-SWEEPS-002-PRF-001 (High)
- Rule violated: PRF-03 and PRF-04
- Impact: Configured total budget exists but lacks runtime enforcement in loop path, allowing potential cost overrun.
- Evidence:
  - `src-tauri/src/models/agents.rs:14` defines `max_total_cost`.
  - Static search in loop path found no enforcement against cumulative spend.
- Remediation actions:
  - Enforce cumulative budget checks in outer loop with deterministic stop and diagnostics event.

#### BG-AUDIT-SWEEPS-002-OBS-001 (Medium)
- Rule violated: OBS-01 and OBS-04
- Impact: Sidecar warmup and lifecycle logs are emitted with `print(...)` and lack structured correlation metadata.
- Evidence:
  - `sidecar/src/main.py:14,21` uses `print(...)`.
  - `sidecar/src/validation/lean_validator.py:243-270` uses `print(...)` for warmup lifecycle.
- Remediation actions:
  - Replace prints with structured logging fields including `run_id`, `attempt_id`, component, severity.

### Sweep verdict
**Conditional Pass** (no Critical, unresolved High findings).

---

## Prioritized remediation backlog (P0 P1 P2)

### P0 (release blockers)
1. `BG-AUDIT-SWEEPS-002-SEC-001` - sandbox or replace unsafe symbolic parser path.
2. `BG-AUDIT-SWEEPS-002-SEC-002` - parameterize `list_problems` SQL and validate status enum.
3. `BG-AUDIT-SWEEPS-002-REL-001` - remove fail open conclusion acceptance path.
4. `BG-AUDIT-SWEEPS-002-DB-001` - transactionalize `record_step` write bundle.

### P1 (stability hardening)
1. `BG-AUDIT-SWEEPS-002-REL-002` - challenger failure policy with retry and downgrade controls.
2. `BG-AUDIT-SWEEPS-002-DB-002` - fail fast DB persistence and ID integrity checks.
3. `BG-AUDIT-SWEEPS-002-PRF-001` - enforce `max_total_cost` in runtime loop.
4. `BG-AUDIT-SWEEPS-002-TST-001` - baseline Rust tests for high risk orchestration paths.

### P2 (quality and contract completeness)
1. `BG-AUDIT-SWEEPS-002-API-001` - typed IPC error envelope.
2. `BG-AUDIT-SWEEPS-002-API-002` - real analytics counters or explicit field deprecation.
3. `BG-AUDIT-SWEEPS-002-OBS-001` - structured sidecar logging with correlation IDs.
4. `BG-AUDIT-SWEEPS-002-SEC-003` - least privilege shell capability policy.

## Residual risk statement
Until all P0 items are closed, release risk remains above acceptable threshold due combined correctness and local attack surface exposure.
