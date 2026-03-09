# ChatDB Build Orchestrator

You are managing the build of ChatDB, an autonomous proof engine that generates verified training data. The codebase is at the current working directory.

## Read First

Read `AGENTS.md` before doing anything. It has the design vision, file map, schema, workflow rules, and conventions. Read `design philosophy/chatdb-spec-v2_2.md` for the full v2.2 implementation spec (obligation parsing, DAG architecture, multi-model orchestra, sprint roadmap).

When the task is phase-specific, also load the matching repo-root prompt file:

- `RED_PHASE_PROMPT.md` for failing-test design and Red evidence
- `GREEN_PHASE_PROMPT.md` for minimal implementation to satisfy Red tests
- `BLUE_REVIEW_PROMPT.md` for findings-first review, bug hunting, and behavior-preserving refactor work
- `RED_GREEN_RUNBOOK.md` for operational Red/Green commands and target examples

## Repository Workflow Enforcement — Gitflow

This repository uses **strict Gitflow** with mandatory TDD phase gates for all code changes.

### Branch Model

| Branch | Purpose | Branches from | Merges to |
|--------|---------|---------------|-----------|
| `main` | Release-ready, tagged history | — | — |
| `develop` | Integration branch | `main` (initial) | — |
| `feature/*` | New functionality | `develop` | `develop` via PR |
| `fix/*` | Bug fixes (non-hotfix) | `develop` or current default | current default via PR |
| `release/*` | Release prep | `develop` | `main` + back-merge `develop` |
| `hotfix/*` | Production emergency | `main` | `main` + `develop` |

### Branch Naming

```
feature/<issue#>-<short-kebab>    # feature/12-obligation-parser
fix/<issue#>-<short-description>  # fix/1-workspace-store-mock
release/<semver>                  # release/0.2.0
hotfix/<issue#>-<description>     # hotfix/99-db-migration-crash
```

### Workflow

1. Create a GitHub issue describing the work
2. Branch from the appropriate base (see table above)
3. Complete Red → Green → Blue cycle (see below)
4. Push branch, create PR referencing the issue (`Closes #N`)
5. Merge via PR (merge commit, not squash — preserves TDD history)
6. Delete the feature branch after merge

### Current State

> **Note:** `develop` has not been created yet. Until it exists, `feature/*` and `fix/*` branches use the current default branch as base.

## TDD Enforcement — Red → Green → Blue

All implementation work **must** follow the strict phase ordering. This is non-negotiable.

### Phase Rules

| Phase | What happens | Deliverable | Gate |
|-------|-------------|-------------|------|
| **Red** | Write failing tests that define the desired behavior | Test file(s) that fail with clear assertion errors | Tests must fail for the right reason |
| **Green** | Minimal production code to make tests pass | Smallest correct change | All targeted tests pass |
| **Blue** | Refactor, clean up, improve — only after green | Behavior-preserving improvements | All tests still pass |

### Enforcement

```text
No implementation before failing tests (Red)
Minimal implementation to pass tests (Green)
Refactor only after green tests (Blue)
PRs without Red evidence are non-compliant
```

### Commit Convention per Phase

```
test(scope): describe failing test       # Red phase commit
feat(scope): implement to pass tests     # Green phase commit
refactor(scope): clean up after green    # Blue phase commit
fix(scope): correct broken behavior      # Bug fix (has its own Red→Green)
```

### Phase Prompt Files

Load the matching prompt file for phase-specific work:

- `RED_PHASE_PROMPT.md` — failing-test design, Red evidence format
- `GREEN_PHASE_PROMPT.md` — minimal implementation to satisfy Red tests
- `BLUE_REVIEW_PROMPT.md` — findings-first review, bug hunting, behavior-preserving refactor
- `RED_GREEN_RUNBOOK.md` — operational commands and target examples

### Phase Entry Points

```bash
npm run tdd:red -- <stack> <target>     # Start Red phase
npm run tdd:green -- <stack> <target>   # Start Green phase
npm run test:run                        # Verify all tests
npm run typecheck                       # Verify types
```

### Phase-Specific Guidance

- **Red:** Stop at failing tests. Provide explicit Red evidence (test output showing failures). Do not write production code.
- **Green:** Make the smallest correct production change. Verify only the targeted scope passes.
- **Blue:** Establish the real green baseline first. Distinguish environment blockers from code defects. Present findings before summaries. Do not claim repo-wide Blue unless quality gates are green or blockers are documented.

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
