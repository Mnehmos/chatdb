# Provenance Red Record

Audit date: 2026-02-27
Audited against: `PROVENANCE.md` (ChatDB DAG Provenance Standard)

## Status Key

- **FIXED** — Resolved in this session
- **DEFERRED** — Planned for a later sprint

---

## FIXED (Session 1 — DAG Wiring)

### RED-001: parent_ids always None on proof_nodes
- **File**: `verification/mod.rs:130` (was)
- **Violation**: Every proof_node was created with `parent_ids: None`
- **Standard**: parent_ids must resolve from parent_step_id via `get_node_by_step_id()`
- **Fix**: Added `get_node_by_step_id()` to `db/proof_nodes.rs`, wired parent resolution into `validate_and_record()`
- **Status**: FIXED

### RED-002: Conclusions bypass proof_nodes entirely
- **File**: `loop_engine/mod.rs:332-399` (was)
- **Violation**: Both gated-rejection and accepted-conclusion paths recorded to `steps` table only, no proof_node created
- **Standard**: Every step MUST produce a proof_node via dual-write
- **Fix**: Added `create_node()` calls for both gated conclusions (status=rejected) and accepted conclusions (status=verified)
- **Status**: FIXED

### RED-003: obligation.parent_node_id receives step IDs not node IDs
- **File**: `loop_engine/mod.rs:547` (was)
- **Violation**: `chain.last()` returns step IDs from `get_verified_chain()`, but `create_obligation()` expects a proof_node ID
- **Standard**: parent_node_id must be a valid proof_node UUID
- **Fix**: Added `get_node_by_step_id()` resolution before obligation creation
- **Status**: FIXED

### RED-004: close_obligation receives stale step ID instead of current node ID
- **File**: `loop_engine/mod.rs:638-643` (was)
- **Violation**: `closure_node_id` was set from `verified.last()` (loaded before current step), yielding the PREVIOUS step's ID — and it was a step_id, not a node_id
- **Standard**: closure_node_id must be the proof_node that actually closes the obligation
- **Fix**: Changed to use `current_node_id` captured from `validate_and_record()` result
- **Status**: FIXED

### RED-005: Temperature never sent to LLM API
- **File**: `api/llm_client.rs` (throughout)
- **Violation**: `ModelConfig.temperature` exists (default 0.3) but `LlmClient` never included it in API requests
- **Standard**: Model configuration must be faithfully transmitted to API
- **Fix**: Added `temperature` field to `LlmClient`, `with_temperature()` builder, injected into all 4 API paths (Anthropic + OpenAI, streaming + non-streaming). Correctly excluded for thinking models (Anthropic) and reasoning models (OpenAI o-series).
- **Status**: FIXED

### RED-006: stream_options.include_usage missing for non-reasoning OpenAI models
- **File**: `api/llm_client.rs:326-334` (was)
- **Violation**: `stream_options: {include_usage: true}` was only set for reasoning models, meaning token usage was never reported for standard OpenAI models in streaming mode
- **Standard**: Token usage must be tracked for all models for training data provenance
- **Fix**: Moved `stream_options` to the base body (set for all OpenAI streaming calls)
- **Status**: FIXED

### RED-007: dag_events never appended during proof runs
- **File**: `dag_events.rs` (schema existed, never called)
- **Violation**: `append_dag_event()` and `get_dag_events()` existed but were never called from the loop engine or verification pipeline
- **Standard**: Every DAG mutation must produce a corresponding dag_event
- **Fix**: Wired 8 event types: attempt_started, node_verified, node_rejected, conclusion_gated, conclusion_verified, obligation_opened, obligation_closed, attempt_finished
- **Status**: FIXED

### RED-008: technique_registry queried but never injected into solver
- **File**: `db/technique_registry.rs` + `loop_engine/solver.rs`
- **Violation**: 78 seed entries loaded into technique_registry, `get_techniques_for_class()` existed but was never called. Solver prompt had no technique guidance.
- **Standard**: Available knowledge should be injected into solver context
- **Fix**: Added technique query in loop engine (by `problem.domain`), added `techniques: &[TechniqueEntry]` parameter to `build_solver_prompt()`, injects top 10 techniques with success/failure ratios
- **Status**: FIXED

---

## FIXED (Session 2 — Provenance Violations)

### RED-009: "root" sentinel used as parent_node_id
- **File**: `loop_engine/mod.rs:609` (was)
- **Violation**: When `get_node_by_step_id()` returns None for the audit chain's last step, `parent_node_id` falls back to `"root"` — a sentinel string, not a UUID
- **Standard**: Never use sentinel strings as IDs
- **Fix**: Now skips obligation creation entirely with `tracing::error!()` log when parent node unresolvable. Audit findings still recorded.
- **Status**: FIXED

### RED-010: obligation.confidence hardcoded to 0.7
- **File**: `loop_engine/mod.rs:615` (was)
- **Violation**: Every obligation created from audit gets `confidence = 0.7` regardless of audit's actual confidence assessment
- **Standard**: Confidence should come from audit output, not be hardcoded
- **Fix**: Added `confidence` field to `ObligationProposal` struct in `audit.rs`. Audit prompt now requests confidence. `create_obligation()` uses `ob.confidence` from parsed audit output.
- **Status**: FIXED

### RED-011: technique_class always None on proof_nodes
- **File**: `verification/mod.rs:151` (was)
- **Violation**: Every proof_node had `technique_class = None`
- **Standard**: technique_class should be populated when known
- **Fix**: Layer 0 mapping: `proposal_type` → `technique_class` (algebraic, tactic, computation, lemma, etc.). Conclusions get `None` (closures don't have a technique). Layer 1 self-tagging will override with finer-grained classification in Sprint 2.
- **Status**: FIXED

### RED-012: obligation_ref always None on proof_nodes
- **File**: `verification/mod.rs:157` (was)
- **Violation**: When a proof_node was generated to close an obligation, `obligation_ref` was not set
- **Standard**: obligation_ref must be set when node targets a specific obligation
- **Fix**: Added `obligation_ref` field to `StepContext`. Loop engine sets it to the highest-priority open obligation ID (solver prompt directs LLM to address this obligation). Passed through to `create_node()`.
- **Status**: FIXED

### RED-014: Missing dag_event types
- **Violation**: Multiple mutation paths had no dag_events
- **Fix**: Added 5 new event types:
  - `audit_started` / `audit_completed` — before/after exploration audit LLM call
  - `review_started` / `review_completed` — before/after post-attempt review
  - `patterns_extracted` — after successful pattern extraction
  - `conclusion_premature` — when premature conclusion is rejected
- Total wired event types: **13** (was 8 after session 1, was 0 before)
- **Status**: FIXED

### RED-015: record_technique_use() never called from loop engine
- **File**: `db/technique_registry.rs:75` (was dead code)
- **Violation**: `record_technique_use(id, success)` existed but was never called
- **Fix**: Added technique feedback loop after pattern feedback. All domain-matched techniques get `record_technique_use(id, proof_complete)` at end of attempt.
- **Status**: FIXED

### RED-016: Premature conclusions don't create proof_nodes or steps
- **File**: `loop_engine/mod.rs:483` (was)
- **Violation**: Premature conclusions existed only as in-memory failure entries
- **Fix**: Now records to steps table (`record_step`), creates rejected proof_node (`create_node` with `validator_result: {premature: true}`), and emits `conclusion_premature` dag_event.
- **Status**: FIXED

### RED-017: No audit_session linkage in obligations
- **Violation**: Obligations created from audit had no field linking to audit source
- **Fix**: Audit is now recorded as a council session via `record_council_session("exploration_audit", ...)`. The resulting `session_id` is included in:
  - `audit_completed` dag_event payload
  - Each `obligation_opened` dag_event payload (`audit_session_id` field)
- Linkage: `obligation → dag_event(obligation_opened).payload.audit_session_id → council_sessions.id`
- **Status**: FIXED

---

## FIXED (Session 3 — Branching Architecture)

### RED-013: branch_id hardcoded to 0 everywhere
- **File**: All `create_node()` and `create_obligation()` calls
- **Violation**: Every node and obligation was created with `branch_id = 0`
- **Standard**: branch_id must reflect actual branch
- **Fix**: Full branching architecture implemented:
  - `db/branches.rs` — Branch CRUD (create, query, close, branch-aware verified chain)
  - `Branch` model in `models/dag.rs`
  - `current_branch_id` tracked in loop engine, propagated to all `create_node()` and `create_obligation()` calls
  - `branch_id` added to `StepContext`, passed through verification pipeline
  - Branch-aware queries: `get_branch_verified_chain()`, `get_branch_open_obligations()`, `count_branch_open_obligations()`
  - Orchestrator expanded with plateau detection (technique tunnel, closure rate, steps without closure)
  - Fork logic: audit `should_branch` + orchestrator agreement triggers branch creation
  - Branch lifecycle: active → completed | abandoned, with dag_events for branch_created/branch_closed
  - Context policy: branch mode resets ephemeral state (failures, audit, plateau counters)
  - Branch abandonment on failure threshold + automatic switch to next active branch
- **Status**: FIXED

---

## Summary

| Status | Count |
|--------|-------|
| FIXED (Session 1) | 8 |
| FIXED (Session 2) | 8 |
| FIXED (Session 3) | 1 |
| **Total Violations** | **17** |

### Resolution Rate: 17/17 (100%)

All provenance violations are resolved.

### DAG Event Coverage

| Event Type | Agent | Wired |
|------------|-------|-------|
| attempt_started | loop_engine | ✓ |
| node_verified | verification_pipeline | ✓ |
| node_rejected | verification_pipeline | ✓ |
| conclusion_gated | loop_engine | ✓ |
| conclusion_verified | loop_engine | ✓ |
| conclusion_premature | loop_engine | ✓ |
| audit_started | audit | ✓ |
| audit_completed | audit | ✓ |
| obligation_opened | audit | ✓ |
| obligation_closed | loop_engine | ✓ |
| review_started | reviewer | ✓ |
| review_completed | reviewer | ✓ |
| patterns_extracted | loop_engine | ✓ |
| branch_created | loop_engine | ✓ |
| branch_closed | loop_engine | ✓ |
| attempt_finished | loop_engine | ✓ |
