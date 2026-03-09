# ChatDB Build Orchestrator

You are managing the build of ChatDB, an autonomous proof engine that generates verified training data. The codebase is at the current working directory.

## Read First

Read `AGENTS.md` before doing anything. It has the design vision, file map, schema, workflow rules, and conventions. Read `design philosophy/chatdb-spec-v2_2.md` for the full v2.2 implementation spec (obligation parsing, DAG architecture, multi-model orchestra, sprint roadmap).

When the task is phase-specific, also load the matching repo-root prompt file:

- `RED_PHASE_PROMPT.md` for failing-test design and Red evidence
- `GREEN_PHASE_PROMPT.md` for minimal implementation to satisfy Red tests
- `BLUE_REVIEW_PROMPT.md` for findings-first review, bug hunting, and behavior-preserving refactor work
- `RED_GREEN_RUNBOOK.md` for operational Red/Green commands and target examples

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

## Phase Prompt Usage

- For Red work, stop at failing tests and provide explicit Red evidence.
- For Green work, make the smallest correct production change and verify the targeted scope.
- For Blue work, establish the real green baseline first, distinguish environment blockers from code defects, and present findings before summaries.
- Do not claim a repo-wide Blue phase unless the meaningful quality gates are green or the blockers are explicitly documented.
- Preferred Red/Green entry points:
  - `npm run tdd:red -- <stack> <target>`
  - `npm run tdd:green -- <stack> <target>`

## Current Target: Sprint 1 — Obligation Parsing Pipeline

M1 (first verified proof) is complete. The system runs a linear step chain with obligations gating conclusion. The next target is three-layer obligation detection per the v2.2 spec.

### What's Done (M1)
- Full SQLite schema (13 core + 4 DAG tables) — `src-tauri/src/db/schema.rs`
- All DB operations including proof_nodes, obligations, dag_events CRUD
- LLM client (Anthropic/OpenAI/OpenRouter) with streaming + thinking — `src-tauri/src/api/llm_client.rs`
- Sidecar with working SymPy + Pint validators
- React frontend with stores, components, event listeners (obligation events included)
- Loop engine with obligation gating, exploration audit, pattern injection, council review
- Dual-write to proof_nodes on every validated step

### Sprint 1 Critical Path
1. Fix DAG foundation bugs (parent_ids always None, conclusions bypass pipeline, obligation node ID mismatch)
2. Layer 1: Solver self-tagging JSON sidecar in prompt
3. Layer 4: Validator-generated obligations (obligation_extractor.py with pattern library)
4. Layer 3: Classifier ensemble (2-3 models in parallel)
5. Merge pipeline with dedup, supersession, confidence scoring, precision governor
6. Pre-solve intelligence briefing (4 parallel agents)
7. Wire all 3 layers into main loop via `tokio::join!`

### Full Roadmap
See `AGENTS.md` Roadmap section and `design philosophy/chatdb-spec-v2_2.md` Part 17.

## Rules

- **Database is the memory.** No in-memory state that isn't backed by SQLite. Every step, every decision, every verdict gets recorded.
- **Rust orchestrates, Python validates.** Hot path in Rust. Validators and agents in Python sidecar on :9743.
- **Frontend observes.** React listens to Tauri events. Never mutates state directly.
- **Every DB operation returns Result.** No unwraps in production paths.
- **New table → schema.rs. New query → its own db/ file. New agent → model + db + command + sidecar endpoint + component.**
- **IDs are uuid v4. Timestamps are chrono UTC rfc3339.**
- **Test validators work before wiring the loop.** Hit `POST /validate/step` manually first.
- **Commit after each milestone.** Not after each file.

## Schema Is The Architecture

When confused, read `src-tauri/src/db/schema.rs`. Every table = an agent. Every column = a training data field. The schema is the spec.
