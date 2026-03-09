# ChatDB DAG Provenance Standard

Every node, obligation, and event in the DAG must carry traceable lineage.
This document defines the contract. Code that violates it is a bug.

## Core Invariant

**Every datum written to a DAG table must answer: who created it, why, from what parent, and with what evidence.**

---

## 1. proof_nodes — Provenance Contract

| Field | Required | Source | Notes |
|-------|----------|--------|-------|
| `id` | yes | uuid::new_v4() | Never reused |
| `attempt_id` | yes | Loop engine | FK to attempts |
| `branch_id` | yes | Orchestrator | 0 = main branch (only value until Sprint 4) |
| `node_type` | yes | Proposal mapping | closure, claim, construction, bound, case_split, reduction |
| `parent_ids` | yes* | Resolved from parent step's proof_node via `get_node_by_step_id()` | *None only for root node (step_number=1). All others MUST resolve. Silent None is a bug. |
| `content` | yes | LLM proposal_natural | Human-readable step |
| `formal_content` | optional | LLM proposal_formal | SymPy/Lean expression |
| `technique_class` | deferred | Layer 1 self-tagging | Sprint 2. Currently None. |
| `construction_family` | deferred | Layer 1 self-tagging | Sprint 2. Currently None. |
| `status` | yes | Validator pipeline | verified, rejected |
| `validator_used` | yes* | Pipeline | *None only for conclusions (no validator) |
| `validator_result` | yes* | Pipeline JSON | *None only for accepted conclusions |
| `model_id` | yes | LlmClient::model_name() | provider/model format |
| `obligation_ref` | when applicable | Loop engine | Must be set when node was generated to close a specific obligation |
| `step_id` | yes | record_step() return value | FK to legacy steps table |
| `token_cost` | optional | LLM response usage | tokens_in from API |
| `sequence_number` | yes | step_number counter | Monotonically increasing per attempt |
| `created_at` | yes | chrono::Utc::now() | ISO 8601 |
| `verified_at` | when verified | chrono::Utc::now() | Set only when status=verified |

### parent_ids Resolution Rule

```
if step_number == 1:
    parent_ids = None  (root node)
else:
    parent_ids = get_node_by_step_id(parent_step_id).id
    if resolution fails:
        log WARN (not silent)
        parent_ids = None (degraded, not fatal)
```

---

## 2. obligations — Provenance Contract

| Field | Required | Source | Notes |
|-------|----------|--------|-------|
| `id` | yes | uuid::new_v4() | |
| `attempt_id` | yes | Loop engine | FK to attempts |
| `branch_id` | yes | Orchestrator | 0 = main branch |
| `parent_node_id` | yes | **Must be a valid proof_node UUID** | Never a step_id. Never "root" sentinel. If unresolvable, use the attempt's first node ID or fail. |
| `description` | yes | Audit LLM output | |
| `obligation_type` | yes | Audit LLM output | |
| `priority` | yes | Audit LLM output | 0.0–1.0 |
| `confidence` | yes | Audit LLM output | Should come from audit confidence, not hardcoded |
| `source_layer` | yes | Integer | 1=self-tag, 2=audit, 3=classifier, 4=validator |
| `status` | yes | Lifecycle | open → assigned → closed_proved / closed_refuted / superseded / retracted / demoted |
| `closure_node_id` | when closed | The proof_node that closes it | Must be a proof_node UUID, not a step_id |
| `closure_type` | when closed | proved, refuted, timeout | |

### parent_node_id Resolution Rule (NEVER use step IDs)

```
chain = get_verified_chain(attempt_id)
parent_step_id = chain.last().step_id
parent_node = get_node_by_step_id(parent_step_id)
if parent_node.is_some():
    parent_node_id = parent_node.id
else:
    log ERROR "Cannot create obligation: no proof_node for step"
    skip obligation creation (do not use sentinel)
```

---

## 3. dag_events — Provenance Contract

Every mutation to proof_nodes or obligations MUST produce a corresponding dag_event.

| Event Type | Trigger | Required Payload Fields |
|------------|---------|------------------------|
| `attempt_started` | Loop begins | problem_id, model, branch_id |
| `node_verified` | Step passes validation | node_id, step_id, node_type, parent_ids |
| `node_rejected` | Step fails validation | node_id, step_id, node_type, parent_ids |
| `conclusion_gated` | Conclusion blocked by obligations | node_id, open_obligations |
| `conclusion_verified` | Conclusion accepted | node_id, step_number |
| `conclusion_premature` | Premature conclusion rejected | node_id, verified_count |
| `audit_started` | Exploration audit begins | step_number, chain_length, branch_id |
| `audit_completed` | Exploration audit finishes | breadth, should_branch, obligations_proposed, session_id |
| `review_started` | Post-attempt review begins | steps_processed |
| `review_completed` | Post-attempt review finishes | findings, coverage, label |
| `patterns_extracted` | Patterns extracted from proof | count, pattern_names |
| `obligation_opened` | Audit creates obligation | obligation_id, parent_node, description, priority, audit_session_id |
| `obligation_closed` | Step resolves obligation | obligation_id, closure_node_id, closure_type |
| `branch_created` | Orchestrator forks at plateau | branch_id, parent_branch, fork_step, reason, direction |
| `branch_closed` | Branch completes or is abandoned | branch_id, status |
| `attempt_finished` | Loop ends | steps_processed, proof_complete, stopped_by_user |

### Planned Event Types (not yet wired)

| Event Type | Trigger | Sprint |
|------------|---------|--------|
| `obligation_escalated` | Obligation escalation triggered | 3 |
| `obligation_demoted` | Obligation priority lowered | 3 |
| `branch_merged` | Branch reintegrated after council review | 5 |

### dag_events Invariants

1. `sequence_number` is per-attempt, monotonically increasing
2. `agent_role` must be one of: loop_engine, verification_pipeline, audit, critic, reviewer
3. `payload` must be valid JSON
4. Events are append-only (never updated or deleted)

---

## 4. steps (Legacy) — Provenance Notes

The `steps` table predates the DAG architecture. It remains the primary execution ledger
during Sprint 1. Every step written here MUST also produce a proof_node via dual-write.

| Invariant | Status |
|-----------|--------|
| Every step has a proof_node | ✓ Enforced (conclusions now included) |
| step.id = proof_node.step_id | ✓ Linked |
| Conclusions recorded in steps | ✓ Both gated-rejection and acceptance |
| Parse failures don't create steps | ✓ Correct (silent retry) |

---

## 5. technique_registry — Provenance Notes

| Field | Source |
|-------|--------|
| `source` | "seed" for initial entries, future: "extracted", "manual" |
| `success_count` / `failure_count` | Updated by pattern feedback loop |
| `last_used_at` | Updated on each use |

Currently read-only in the loop (queried for solver prompt, not mutated by proof execution).
`record_technique_use()` exists but is not yet called from the loop engine.

---

## 6. ID Namespace Rules

| Table | ID Type | Format |
|-------|---------|--------|
| proof_nodes | UUID v4 | `xxxxxxxx-xxxx-4xxx-xxxx-xxxxxxxxxxxx` |
| obligations | UUID v4 | Same |
| steps | UUID v4 | Same |
| attempts | UUID v4 | Same |
| dag_events | Auto-increment i64 | 1, 2, 3, ... |
| technique_registry | Auto-increment i64 | 1, 2, 3, ... |

**NEVER use a step_id where a node_id is expected.** They are different namespaces
pointing to different tables. The only legitimate cross-reference is `proof_nodes.step_id`.

**NEVER use sentinel strings** ("root", "unknown", "") as IDs. If resolution fails,
the field must be None/NULL with a logged warning.
