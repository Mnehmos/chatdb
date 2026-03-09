# ChatDB Code Review Checklist (Audit-Executable)

- Run ID: `run-code-review-checklist-001`
- Task ID: `checklist-design-001`
- Scope: docs-only checklist design for repository audits
- Source baseline: `docs/code-review-techniques-research.md`
- Architecture context: `AGENTS.md` (Rust orchestration, Python sidecar validators/agents, React frontend, SQLite as system-of-record)

## TDD Enforcement Prompt Tag

```text
No implementation before failing tests (Red)
Minimal implementation to pass tests (Green)
Refactor only after green tests (Blue)
PRs without Red evidence are non-compliant
```

## 1) Purpose and Use Contract

This checklist is designed for **operational code review audits**, not just pre-merge commentary.  
Every line item is:

- **Scoreable**: `Yes / No / NA`
- **Risk-weighted**: `Critical / High / Medium / Low`
- **Evidence-backed**: link to file(s), test output, logs, ADR, or schema/migration artifact

### 1.1 Scoring Fields (Required Per Line Item)

Use this record shape for each checklist item:

| Field | Allowed Values | Required | Notes |
|---|---|---|---|
| Result | Yes / No / NA | Yes | `Yes` = control satisfied, `No` = finding, `NA` = not applicable with reason |
| Severity | Critical / High / Medium / Low | Yes (if Result=No) | Risk of violation |
| Evidence | URL/path/command output ref | Yes | Must point to concrete artifact |
| Reviewer Note | free text | Yes | Short rationale and context |

### 1.2 Severity Model

- **Critical**: Could cause incorrect proof outcomes, security breach, irreversible data corruption, or system-wide outage.
- **High**: Significant correctness/reliability/security risk with high user or operational impact.
- **Medium**: Meaningful quality degradation, incident potential under load or edge conditions.
- **Low**: Maintainability/readability/process nonconformance with limited immediate runtime risk.

---

## 2) Checklist Matrix (Core Dimensions)

> Auditor instruction: mark each item `Yes/No/NA`, assign severity for `No`, and attach evidence.

### A. Correctness

| ID | Rule | Result | Severity | Evidence | Reviewer Note |
|---|---|---|---|---|---|
| COR-01 | Changed logic preserves intended mathematical/validation behavior under stated constraints. |  |  |  |  |
| COR-02 | Edge cases and failure paths for changed logic are explicitly handled (not implied). |  |  |  |  |
| COR-03 | Rejections from validators are handled deterministically (no silent success path). |  |  |  |  |
| COR-04 | Any claim extraction/parsing changes include guards against malformed model output. |  |  |  |  |

### B. Security

| ID | Rule | Result | Severity | Evidence | Reviewer Note |
|---|---|---|---|---|---|
| SEC-01 | Trust boundaries are explicit (frontend → Rust IPC → sidecar HTTP) with validation at each boundary. |  |  |  |  |
| SEC-02 | Secrets/tokens are not hardcoded, leaked in logs, or exposed through debug outputs. |  |  |  |  |
| SEC-03 | Input handling prevents injection and unsafe deserialization in Rust/Python interfaces. |  |  |  |  |
| SEC-04 | Authz-sensitive or high-risk operations require independent reviewer confirmation (two-person rule where applicable). |  |  |  |  |

### C. Reliability & Concurrency

| ID | Rule | Result | Severity | Evidence | Reviewer Note |
|---|---|---|---|---|---|
| REL-01 | Async/task orchestration changes define cancellation/retry/idempotency behavior explicitly. |  |  |  |  |
| REL-02 | Shared resource access (worker pool, DB writes, sidecar calls) avoids race-prone patterns. |  |  |  |  |
| REL-03 | Failure handling is explicit: timeout/error/backoff behavior is implemented and observable. |  |  |  |  |
| REL-04 | No deadlock-prone lock ordering or blocking calls on critical paths. |  |  |  |  |

### D. Database & Invariants (SQLite-first Contract)

| ID | Rule | Result | Severity | Evidence | Reviewer Note |
|---|---|---|---|---|---|
| DB-01 | Database remains system-of-record; no hidden in-memory authoritative state introduced. |  |  |  |  |
| DB-02 | Schema/query changes preserve key invariants (obligation lifecycle, DAG consistency, event ordering). |  |  |  |  |
| DB-03 | Migrations are reversible or explicitly justified as irreversible with mitigation and backup plan. |  |  |  |  |
| DB-04 | Write paths are atomic and error-propagating (no swallowed DB failures). |  |  |  |  |

### E. API Contracts (IPC, HTTP, Model Contracts)

| ID | Rule | Result | Severity | Evidence | Reviewer Note |
|---|---|---|---|---|---|
| API-01 | Tauri command contract changes are mirrored in frontend service types and consumers. |  |  |  |  |
| API-02 | Sidecar request/response model changes are versioned or backward-compatible by contract. |  |  |  |  |
| API-03 | Error semantics are stable and machine-actionable across layer boundaries. |  |  |  |  |
| API-04 | Contract change includes consumer impact analysis and migration notes. |  |  |  |  |

### F. Test Quality

| ID | Rule | Result | Severity | Evidence | Reviewer Note |
|---|---|---|---|---|---|
| TST-01 | Changed behavior is covered by tests that would fail without the change. |  |  |  |  |
| TST-02 | Negative/failure-mode tests exist for critical paths (validation failures, sidecar errors, DB errors). |  |  |  |  |
| TST-03 | Assertions are specific (not weak truthy checks) and tied to contract outcomes. |  |  |  |  |
| TST-04 | Flaky/concurrency-sensitive tests include stabilization strategy or deterministic guardrails. |  |  |  |  |

### G. Observability & Logging

| ID | Rule | Result | Severity | Evidence | Reviewer Note |
|---|---|---|---|---|---|
| OBS-01 | Key decisions/failures are logged with enough context for audit reconstruction. |  |  |  |  |
| OBS-02 | Log lines avoid sensitive payload leakage while preserving debuggability. |  |  |  |  |
| OBS-03 | Event emission to frontend remains consistent with status transitions. |  |  |  |  |
| OBS-04 | Post-incident traceability is preserved (run IDs, attempt IDs, node/obligation references). |  |  |  |  |

### H. Performance & Cost

| ID | Rule | Result | Severity | Evidence | Reviewer Note |
|---|---|---|---|---|---|
| PRF-01 | Hot path complexity does not regress without explicit budget/rationale. |  |  |  |  |
| PRF-02 | DB query patterns avoid obvious N+1 and unbounded scans on large datasets. |  |  |  |  |
| PRF-03 | LLM/sidecar call volume changes include cost-impact assessment. |  |  |  |  |
| PRF-04 | Retry/timeouts/concurrency settings are tuned to avoid runaway spend or latency spikes. |  |  |  |  |

### I. Architecture Boundaries

| ID | Rule | Result | Severity | Evidence | Reviewer Note |
|---|---|---|---|---|---|
| ARC-01 | Frontend remains observational (no direct sidecar/database writes). |  |  |  |  |
| ARC-02 | Rust backend remains orchestration/control plane, Python sidecar remains compute/validation plane. |  |  |  |  |
| ARC-03 | New dependencies or boundary shifts are justified against architecture docs/ADRs. |  |  |  |  |
| ARC-04 | DAG/obligation-driven loop semantics are not bypassed by ad-hoc control flow. |  |  |  |  |

### J. Maintainability & Documentation

| ID | Rule | Result | Severity | Evidence | Reviewer Note |
|---|---|---|---|---|---|
| MNT-01 | Names, module boundaries, and responsibilities are clear and consistent with existing patterns. |  |  |  |  |
| MNT-02 | Non-obvious decisions are documented where future maintainers need rationale. |  |  |  |  |
| MNT-03 | Diff does not mix unrelated concerns (feature+refactor+cleanup) without clear partitioning. |  |  |  |  |
| MNT-04 | Documentation/contracts are updated for behavior or interface changes. |  |  |  |  |

---

## 3) Risk Triage and Required Review Depth

Assign a **review tier** before applying checklist items:

| Tier | Trigger | Required Depth |
|---|---|---|
| Tier 1 (Low) | Small isolated change, low blast radius | Core checklist + 1 reviewer |
| Tier 2 (Medium) | Cross-module behavior impact | Core checklist + scenario walkthrough + test-depth review |
| Tier 3 (High) | Contract/schema/concurrency/security sensitive | Core checklist + specialist overlays (security/reliability/db/api) |
| Tier 4 (Critical) | Privilege boundaries, migration risk, core loop invariants | Tier 3 + two-person independent approval + rollback/readiness evidence |

---

## 4) Sweep Protocol (Audit Execution)

This protocol defines how to perform a repeatable audit sweep and produce findings.

### Step 0 — Define Sweep Scope

Record:
- Review window (commit range / PR set)
- Components touched (Rust, Python sidecar, React, docs, schema)
- Risk tier per change

### Step 1 — Build Evidence Pack

Collect and link:
- Diff/PR links
- Test results (unit/integration/e2e where applicable)
- Relevant logs/metrics snapshots
- Contract/schema/ADR references
- Migration/rollback artifacts (if DB touched)

### Step 2 — Execute Checklist by Dimension

For each changed unit:
1. Score each applicable control (`Yes/No/NA`)
2. For every `No`, assign severity + finding ID
3. Attach evidence link for each scored row

### Step 3 — Consolidate Findings

Group findings by:
- Severity (Critical/High/Medium/Low)
- Dimension (COR/SEC/REL/DB/API/TST/OBS/PRF/ARC/MNT)
- Component (`src-tauri/`, `sidecar/`, `src/`, `docs/`)

### Step 4 — Determine Sweep Verdict

Use deterministic gate:
- **Fail**: any Critical finding unresolved
- **Conditional Pass**: no Critical, but High findings require dated remediation plan
- **Pass**: no unresolved Critical/High, Medium/Low tracked

### Step 5 — Publish Audit Report

Publish:
- Score summary table
- Findings register
- Required remediations with owner and due date
- Residual risk statement

---

## 5) Finding Template (Required)

Use one record per issue:

```md
### Finding: <ID>
- ID: <e.g., DB-03-2026-02-28-01>
- File: <relative/path>
- Rule Violated: <checklist rule ID + short text>
- Risk: <Critical|High|Medium|Low>
- Evidence: <link/path/command output>
- Impact: <what can go wrong>
- Remediation: <specific fix>
- Owner: <team/person>
- Due Date: <YYYY-MM-DD>
- Status: <Open|Accepted Risk|In Progress|Resolved>
```

---

## 6) Score Summary Template

```md
## Sweep Summary
- Sweep ID: <identifier>
- Window: <commit/PR range>
- Auditor(s): <names>

| Dimension | Yes | No | NA | Open High/Critical |
|---|---:|---:|---:|---:|
| Correctness |  |  |  |  |
| Security |  |  |  |  |
| Reliability/Concurrency |  |  |  |  |
| Database/Invariants |  |  |  |  |
| API Contracts |  |  |  |  |
| Test Quality |  |  |  |  |
| Observability/Logging |  |  |  |  |
| Performance/Cost |  |  |  |  |
| Architecture Boundaries |  |  |  |  |
| Maintainability/Documentation |  |  |  |  |

Verdict: <Pass | Conditional Pass | Fail>
```

---

## 7) ChatDB-Specific Auditor Notes

Pay special attention to these repository-specific risk patterns:

1. **Invariant Drift Risk**: changes that bypass obligation closure semantics or event-sourced DAG updates.
2. **Boundary Drift Risk**: frontend taking on orchestration responsibilities or sidecar becoming stateful authority.
3. **Silent Failure Risk**: validator/DB/API errors logged but not surfaced to control flow.
4. **Contract Drift Risk**: Rust model/command updates not reflected in frontend types or sidecar contracts.
5. **Cost Spiral Risk**: increased model-call fan-out without explicit budget controls.

This checklist should be revised quarterly using incident/postmortem findings and audit calibration results.
