# ChatDB — Agent Development Guide

## What This Is

ChatDB is an autonomous proof engine that generates seven layers of verified training data. It proposes mathematical proof steps via LLMs, validates them through mechanical validators (SymPy, Pint, Lean 4), records everything to SQLite, and learns from its own failures. The training data generated is the primary product — not the proofs themselves.

## Architecture (Three Layers)

```
React Frontend (src/)          — Observes. Renders state. Never mutates directly.
     │ Tauri IPC
Rust Backend (src-tauri/src/)  — Orchestrates. Hot path. State machine. Dispatches work.
     │ HTTP :9743
Python Sidecar (sidecar/src/) — Computes. Validates. Deliberates. Latency-tolerant.
     │
SQLite (chatdb.sqlite)        — Single source of truth. WAL mode. Every agent reads/writes here.
```

**The database IS the memory.** No chat history. No model state. Every session starts fresh. Any model reconstructs state from SQL queries. Multi-model continuity comes free.

## Repository Workflow Enforcement

This repository uses Gitflow with strict TDD phase ordering for all implementation work.

- Long-lived branches: `main` (release-ready) and `develop` (integration).
- Working branches:
  - `feature/*` branch from `develop` and merge back to `develop` via PR.
  - `release/*` branch from `develop` and merge to both `main` and `develop`.
  - `hotfix/*` branch from `main` and merge to both `main` and `develop`.
- Mandatory implementation flow is **Red → Green → Blue** and cannot be reordered.

## TDD Enforcement Prompt Tag

```text
No implementation before failing tests (Red)
Minimal implementation to pass tests (Green)
Refactor only after green tests (Blue)
PRs without Red evidence are non-compliant
```

## Phase Prompt Library

Use the phase-specific prompt files at the repo root when handing work to the next agent or when you need a stricter workflow brief than this guide alone provides.

- `RED_PHASE_PROMPT.md` — Write or refine failing tests first. Use for strict Red evidence, contract definition, and test-layer selection.
- `GREEN_PHASE_PROMPT.md` — Implement the minimum production change required to make existing Red tests pass. Use for narrow, verified Green work only.
- `BLUE_REVIEW_PROMPT.md` — Perform review, bug-hunting, and behavior-preserving refactor work after Green is established. Use when the task is review-heavy or when you need findings-first output.
- `RED_GREEN_RUNBOOK.md` — Operational Red/Green workflow. Use this when you need actual commands and target examples rather than prompt text alone.

Phase usage rules:

- Start every implementation task by explicitly naming the current phase: Red, Green, or Blue.
- Red work stops at failing tests. Do not blend implementation into Red.
- Green work stops when the Red tests pass with the smallest correct change.
- Blue work is only valid after a meaningful green baseline is established, or after blockers to a repo-wide green baseline are clearly documented.
- If you are delegating or re-prompting another agent, point it to this file first and then the phase-specific prompt file for the current step.
- Preferred phase entry points:
  - `npm run tdd:red -- <stack> <target>`
  - `npm run tdd:green -- <stack> <target>`

## Design Vision

The following principles are the north star for every architectural decision. They are derived from the Four Irreducibilities research (`design philosophy/Four-Irreducibilities-v2.pdf`) and formalized in the v2.2 spec (`design philosophy/chatdb-spec-v2_2.md`).

### Separation of Storage from Attention

The linear context window is the wrong data structure for proof search. Append-only context forces chronological accumulation — every step taken sits in front of the solver whether relevant or not. This is how strategy interference operates, how decorative tokens accumulate, and why verification and exploration compete instead of compose.

**Everything persists in the DAG. Only what matters enters the context.** The DAG is the single source of truth: all obligations, all verified results, all working notes, all failed approaches. But no agent ever sees the full DAG. Each agent call receives a surgically curated context slice constructed by the orchestration layer for that specific call.

The database is the intelligence. The agents are hands. And hands are cheap.

### The Four Irreducibilities

These are mathematical barriers, not engineering bottlenecks. The architecture accommodates them rather than pretending they don't exist.

| Constraint | Statement | Architectural Response |
|------------|-----------|----------------------|
| **Obligation Opacity** | Can't enumerate proof obligations from the problem statement. They emerge recursively at every depth — fractal opacity. | Obligations must be mutable at every node. Three-layer parsing pipeline (self-tag, classifier ensemble, validator-generated). The gap between "all obligations closed" and "all identified obligations closed" is irreducible. |
| **Solver Opacity** | Verification of outputs doesn't predict process. 77% of reasoning tokens are causally decorative (Project Ariadne). | Never trust the solver's process; verify only its outputs. Stateless agent calls. Each call is independent and its output is verified, not its chain. |
| **V-E Tradeoff** | Verification depth and exploration breadth strictly trade off on a concave Pareto frontier. | Make the allocation visible and auditable. The audit trail records how budget was allocated across branches. Three-level verification matches effort to need. |
| **Strategy Interference** | Pursuing one proof path forecloses alternatives. Switching cost is strictly positive — context contamination is universal. | Default: stateless short calls with curated minimal context. Accumulation mode is the exception, triggered by sustained progress, not the default. Fresh context for each new branch. |

### Agents as Stateless Functions

Every agent call — solver, verifier, adversary, librarian, formalizer — is a stateless function over a curated slice of the database. The agent receives a context packet, produces an output, and terminates. The orchestration layer observes the output, updates the DAG, orients against the obligation graph, decides the next assignment, and constructs the next context packet.

The OODA loop runs at the orchestration layer, not inside agents. Agents never orient. They never observe their own history. They are pure functions.

### Nodes Are Not Steps

Nodes are obligation-driven, not iteration-driven. A node doesn't exist because "the LLM said something on iteration 12." A node exists because there's an open obligation that needs closing. The orchestration layer picks the highest-priority open obligation, constructs a minimal context packet, dispatches to the appropriate agent, and records the result.

Nodes come from:
- **LLM solver calls** — the primary creative source
- **CAS computations** — SymPy evaluations, divisor enumerations
- **Lean checks** — formal verification results
- **Tool invocations** — counterexample searches, enumeration tasks
- **Critic evaluations** — adversarial checks on claims

Each node carries: its obligation target (what it's trying to close), its context provenance (exactly what the agent saw), its parent edges (DAG linkage, not linear sequence), and its verification status.

### Two Context Policies

**Accumulation mode** — the solver runs with growing context, building on its own work, closing obligations. A solver deep in a number theory approach, finding the next lemma because the previous three primed it — that's generative capacity working as intended. Don't interrupt that.

**Branch mode** — triggered by plateau detection. New solver instance, fresh context, just the open obligation and the minimum definitions. Maybe the librarian's anti-context injection pointing at an orthogonal technique family. A completely different angle.

The trigger is the plateau. The tracker watches obligation closure rate. When steps are being produced but obligations aren't closing — when the solver generates verified-correct steps that don't advance the proof — that's the signal. Not "you've been running too long" but "you've stopped making progress."

### The Completeness Invariant

```
while open_obligations > 0 && budget > 0:
    assign_next_obligation()
```

The proof cannot terminate with open obligations. This replaces text-matching heuristics. A proof is complete when all obligations are closed, superseded, retracted, or demoted, AND a conclusion node has been verified.

**Honest caveat:** This provides resource-bounded epistemic closure, not mathematical completeness. Unknown unknowns — construction families not in the pattern library, techniques not in any classifier's training data — are invisible to the obligation system. The system makes ignorance legible, persistent, and expensive to ignore. That is a strong property. It is not the same as completeness.

### Muda: The Seven Wastes

| Waste | In Linear Context | In DAG Architecture |
|-------|-------------------|---------------------|
| Overproduction | Solver generates steps past plateau | Plateau detector cuts to new branch |
| Waiting | Solver blocked during irrelevant verification | Stateless parallel calls; no blocking |
| Transport | Context carries irrelevant history | Curated slices; only relevant state enters context |
| Overprocessing | All steps verified at maximum depth | Three-level verification matches effort to need |
| Inventory | Accumulated context never used again | Context trimmed at branch points; DAG stores everything |
| Motion | Strategy switching through contaminated context | Fresh context for each new branch |
| Defects | Verified-but-incomplete proofs submitted | Completeness invariant gates termination |

### Implementation Test Questions

Every implementation decision should pass these checks:

1. **Is the DAG the source of truth?** If state lives outside the DAG, it's wrong.
2. **Is context curated or accumulated?** If an agent sees more than it needs, it's waste.
3. **Are obligations driving execution?** If the loop advances by step count rather than obligation state, it's a chain pretending to be a DAG.
4. **Is the plateau detector deciding mode transitions?** If context mode is set by a timer or step count, it's not adaptive.
5. **Can you fork from any node?** If branching requires rewinding a linear history, the structure is wrong.
6. **Is every node's context provenance recorded?** If you can't reconstruct what an agent saw when it produced a result, you can't audit it.
7. **Does the system know what it hasn't verified?** If unexplored branches are treated as handled, you get c=3 instead of c=4.

> **Full implementation spec:** `design philosophy/chatdb-spec-v2_2.md` — 20 parts covering obligation parsing pipeline, DAG architecture, multi-model orchestra, context management, safety, API surface, cost model, and 6-month sprint roadmap.

## File Map

### Rust Backend — `src-tauri/src/`
```
main.rs                        — Entry point, AppState, Tauri command registration
lib.rs                         — Module exports
api/
  mod.rs
  commands/
    mod.rs
    problem.rs                 — create_problem, get_problem, list_problems
    loop_cmd.rs                — start_solve, pause_solve, stop_solve
    proof.rs                   — get_verified_chain
    patterns.rs                — search_patterns
    analytics.rs               — get_training_data_stats
  sidecar.rs                   — HTTP client to Python sidecar (:9743)
  llm_client.rs                — Anthropic/OpenAI API client
db/
  mod.rs                       — ChatDB struct, connection pool, migrations
  schema.rs                    — Full v2.0 schema (13 tables, all indexes)
  problems.rs                  — Problem CRUD
  attempts.rs                  — Attempt lifecycle
  steps.rs                     — Step recording + verified chain queries
  patterns.rs                  — Pattern search
  orchestrator.rs              — Orchestrator decision recording
  critic.rs                    — Critic evaluation recording
  council.rs                   — Council session + finding recording
  scout.rs                     — Scout query recording
models/
  mod.rs
  proof.rs                     — Problem, Step structs
  agents.rs                    — MultiAgentConfig, ModelConfig, WorkerState, TrainingDataStats
  council.rs                   — CouncilSession, CouncilFinding
  patterns.rs                  — Pattern
  research.rs                  — ScoutQuery
loop_engine/
  mod.rs                       — LoopEngine — main autonomous loop (M1 TARGET)
  solver.rs                    — Prompt builder for solver agents
  orchestrator.rs              — Worker routing, failure tracking, rotation
  critic.rs                    — Critic prompt builder
  worker_pool.rs               — Parallel LLM worker management
verification/
  mod.rs                       — VerificationPipeline — coordinates sidecar validators
```

### Python Sidecar — `sidecar/src/`
```
main.py                        — FastAPI app, router mounting, health endpoint
validation/
  router.py                    — /validate/step endpoint, result aggregation
  sympy_validator.py           — Algebraic equality, simplification checks
  pint_validator.py            — Dimensional analysis
  lean_validator.py            — Lean 4 via Pantograph (placeholder)
agents/
  router.py                    — /agents/council, /agents/scout, /agents/librarian
  council.py                   — After-action council deliberation
  scout.py                     — arXiv + Semantic Scholar search
  librarian.py                 — Pattern library curation
patterns/
  router.py                    — /patterns/extract endpoint
training_data/
  router.py                    — /export/training_data/stats, /export/training_data/export
```

### React Frontend — `src/`
```
main.tsx                       — Entry point
App.tsx                        — Layout, routing between ProblemInput and proof workspace
types/index.ts                 — All TypeScript types (mirror Rust models)
services/
  tauri.ts                     — Tauri invoke wrappers
  events.ts                    — Tauri event listener setup
stores/
  problemStore.ts              — Problem list, active problem
  loopStore.ts                 — Loop status, steps, attempt
  agentStore.ts                — Orchestrator/Critic/Council/Scout event logs
components/
  shared/Header.tsx            — App header with status dot
  shared/Sidebar.tsx           — Problem list sidebar
  problem/ProblemInput.tsx     — New problem form
  proof/ProofTree.tsx          — Step-by-step proof visualization
  loop/LoopControls.tsx        — Start/Pause/Stop controls
  agent/AgentDashboard.tsx     — Agent activity panels
  analytics/TrainingDataStats.tsx — 7-layer training data bar chart
styles/global.css              — Dark theme, industrial-scientific aesthetic
```

## Database Schema (v2.0)

13 tables. The schema is the contract.

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| problems | What to prove | statement, formal_statement, domain, status |
| attempts | One try at a problem | problem_id, strategy, status, models_used |
| steps | Individual proof steps | attempt_id, model, goal_state, proposal_type/natural/formal, verified, rejection_reason, sympy/pint/lean_passed, critic_prediction |
| patterns | Reusable proof strategies | trigger, strategy, success_count, failure_count, technique_class |
| modifications | Self-modification records | target_system, code_diff, meta_verified, active |
| orchestrator_decisions | Routing choices | decision_type, worker_states, decision, reasoning, outcome |
| critic_evaluations | Pre-validation predictions | prediction, confidence, actual_outcome, prediction_correct, cost_saved |
| council_sessions | Deliberation records | trigger_type, council_models, transcript, findings_count |
| council_findings | Structured insights | finding_type, summary, detail, consensus, dissent, target_agent |
| scout_queries | Research queries | trigger_type, query_text, sources_queried, techniques_found, helped |
| librarian_actions | Pattern curation | action_type, pattern_id, reasoning, solver_performance_delta |
| research_cache | API response cache | source, query_hash, response_json, ttl_hours |

### V4 DAG Architecture Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| proof_nodes | DAG nodes (typed propositions) | attempt_id, branch_id, node_type, parent_ids (JSON), content, formal_content, technique_class, status, validator_result, obligation_ref, step_id, sequence_number |
| obligations | Proof gates — search spaces to explore | attempt_id, parent_node_id, description, obligation_type, priority, confidence, source_layer, status, closure_node_id, escalation_level, steps_spent |
| dag_events | Event-sourced DAG mutation log | attempt_id, event_type, payload (JSON), agent_role, sequence_number |
| technique_registry | Growing technique pattern library | problem_class, technique_family, description, success_count, failure_count, source |
| branches | Proof branch tracking | attempt_id, parent_node_id, branch_reason, status |

## The Seven Training Data Layers

Every agent produces a distinct class of training data. All mechanically verified. All structured. This is the product.

| Layer | Source Agent | What It Records | Why It's Valuable |
|-------|-------------|-----------------|-------------------|
| 1 | Solvers | (goal, proposal, verdict, rejection_reason) | Step-level PRM with mechanical labels |
| 2 | Solvers | (problem, full_attempt_tree, backtracks, outcome) | Complete search trajectories |
| 3 | Orchestrator | (state, routing_decision, outcome) | Multi-agent coordination training data — nobody else has this |
| 4 | Council | (attempt_record, deliberation, findings) | Metacognitive reasoning about reasoning |
| 5 | Scout | (gap, research_query, technique_found, impact) | When-to-research training data |
| 6 | Critic | (trajectory, prediction, actual_outcome) | Evaluation calibration |
| 7 | Librarian | (finding, curation_decision, downstream_impact) | Knowledge management traces |

## Roadmap (v2.2 Sprints)

M1 (first verified proof) is complete. The roadmap now follows the v2.2 spec.

- **Sprint 1: Obligation Parsing Pipeline** (Weeks 1-4) — Three-layer obligation detection (self-tag, classifier ensemble, validator-generated). Pre-solve intelligence briefing. Merge pipeline with confidence scoring. **Validation:** P3 Step 9 generates >= 3 construction family obligations.
- **Sprint 2: Obligation Engine + Completeness Invariant** (Weeks 5-8) — DAG becomes authoritative. Event-sourced mutations. Full obligation lifecycle (superseded, retracted, tentative). Loop driven by `while open_obligations > 0 && budget > 0`. **Validation:** Cannot conclude c=3 with open obligations.
- **Sprint 3: Multi-Model + Adversary** (Weeks 9-14) — Parallel agents with distinct roles. Adversary (speculative + attack). Librarian (coverage monitoring). Worker pool with obligation assignment. Escalation ladder (6 levels). Context management with relevance filtering. **Validation:** Adversary finds ratio > 3.
- **Sprint 4: Registry + Learning + Safety** (Weeks 15-18) — Technique registry with real-time updates. OODA self-modification with in-run regression testing. Death spiral detection. Completion quality metric. Cold start bootstrapping. Dynamic confidence calibration. **Validation:** 10-20 diverse calibration problems. Registry grows.
- **Sprint 5: API + Formalization + Polish** (Weeks 19-24) — HTTP API (Axum). Webhook emitter. Incremental Lean formalization. Continuous council (3 checkpoints). MCP server wrapper. **Validation:** MCP drives full P3 run, produces c=4 with high completion quality.

## Development Conventions

### Rust
- All state in SQLite via `ChatDB` struct. No in-memory caches that aren't backed by DB.
- Every DB operation returns `Result<T, DbError>`. Never unwrap in production paths.
- Tauri commands are thin wrappers — business logic lives in `db/` and `loop_engine/`.
- New tables go in `schema.rs`. New queries go in their own `db/` file.
- Models in `models/` are serde-serializable. They cross the IPC boundary.
- Use `uuid::Uuid::new_v4()` for all IDs. `chrono::Utc::now().to_rfc3339()` for timestamps.

### Python
- FastAPI with Pydantic models. Every endpoint has typed request/response.
- Validators return `ValidatorResult(passed, message, raw_output, wall_time_ms)`.
- Agents are async. Council does multi-turn LLM calls. Scout does HTTP to external APIs.
- No state in the sidecar. It's a pure function server. State lives in SQLite.

### TypeScript/React
- Zustand stores, no Redux. Stores mirror Rust state.
- `services/tauri.ts` wraps all `invoke()` calls with types.
- `services/events.ts` sets up Tauri event listeners.
- Components are functional. No class components.
- CSS in `styles/global.css` — CSS variables, no CSS-in-JS.

### Cross-Layer
- Rust owns the database. Python reads/writes via its own SQLite connection when needed.
- Frontend NEVER talks to Python directly. Always through Rust via Tauri IPC.
- Tauri events flow: Rust → Frontend. Frontend renders. That's it.
- New agent? Add: Rust model + DB table + DB operations + Tauri command + Sidecar endpoint + React component + store.

## Critical Path to M1

The loop engine (`loop_engine/mod.rs`) has a `break` placeholder where LLM integration goes. M1 requires:

1. **Wire LLM client into loop** — Build prompt via `solver.rs`, call `llm_client.rs`, parse JSON response
2. **Send proposal to sidecar** — Call `/validate/step` via `sidecar.rs`
3. **Record to DB** — `steps.rs` already handles this
4. **Emit Tauri events** — Frontend already listens
5. **Loop until solved or budget exhausted**

SymPy and Pint validators are functional. Lean is a placeholder. M1 can be achieved with SymPy alone on algebraic proofs.

## Design Principles

See **Design Vision** section above for the full architectural philosophy. Core maxims:

- **"The database is the intelligence, the agent is just hands."** — Agents are stateless functions. SQLite is the memory. The DAG is the structure. Hands are cheap.
- **"Alignment by control flow, not alignment by prompt."** — The obligation table doesn't care about hallucination. `SELECT COUNT(*) FROM obligations WHERE status = 'open'` returns a number. If it isn't zero, the loop doesn't terminate.
- **"Hallucination is a constraint problem."** — Validators are the constraint surface. More validators = less hallucination.
- **"Every rejected step is training data."** — Failures are the product. The system gets paid to fail interestingly.

> Full implementation spec: `design philosophy/chatdb-spec-v2_2.md`

## Environment

- **Tauri 2.x** — Desktop app framework
- **Rust 2021 edition** — Backend
- **Python 3.11+** — Sidecar (FastAPI, SymPy, Pint)
- **React 19 + Vite 6** — Frontend
- **SQLite via rusqlite** — WAL mode, bundled
- **Lean 4 + Mathlib** — Formal verification (install separately when ready)

## When You're Lost

Read `db/schema.rs`. The schema IS the architecture. Every table maps to an agent. Every column maps to a training data field. If you understand the schema, you understand the system.
