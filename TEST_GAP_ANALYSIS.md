# Test Gap Analysis

Comprehensive testing gap analysis for ChatDB, organized by feature and production file.

## Scope

- Date of audit: 2026-03-06
- Goal: identify direct testing coverage gaps feature by feature and file by file
- Coverage here means tests that directly exercise a module or its public contract in-repo
- This is not statement or line coverage
- Generated from source inventory plus current Rust, Python, and frontend test surfaces

## Current Gate Status

### Rust

- `cargo test` passes
- Current direct test surfaces:
  - `src-tauri/tests/tool_use_tests.rs`
  - `src-tauri/tests/discerner_tests.rs`
  - internal unit tests in `src-tauri/src/loop_engine/mod.rs`
  - internal unit tests in `src-tauri/src/loop_engine/contradiction.rs`
  - internal unit tests in `src-tauri/src/loop_engine/evidence.rs`
  - internal unit tests in `src-tauri/src/loop_engine/response_guard.rs`

### Python

- Validator-focused tests pass
- Current direct test surfaces:
  - `sidecar/tests/test_validators.py`
  - `sidecar/tests/test_sympy_sandbox_boundary.py`
  - `sidecar/tests/test_typed_claims.py`
  - `sidecar/tests/test_lean_validator.py`
  - `sidecar/tests/test_discerner.py`
  - `sidecar/tests/test_reconnaissance.py`
  - `sidecar/tests/test_mcp_tools.py`
- Gap in harness reliability:
  - `test_mcp_tools.py` is sensitive to temp-dir permissions in restricted environments

### Frontend

- `npx tsc --noEmit` passes
- No `.test.ts`, `.test.tsx`, `.spec.ts`, or `.spec.tsx` files under `src/`
- `package.json` declares `vitest`, but there is no frontend test suite yet
- `npm run lint` is not currently a valid gate because ESLint 9 flat config is missing

## Coverage Summary

- Rust production files: 69
  - Files with meaningful direct tests: concentrated in 6 modules/test files
  - Largest gaps: loop execution, LLM client, sidecar client, verification pipeline, command layer, most DB files
- Python production files: 39
  - Strongest coverage: validation modules and MCP data-inspection tools
  - Largest gaps: routers, research clients, council/scout/librarian agents, loop-control MCP tools
- Frontend production files: 40 TS/TSX files plus global CSS
  - Direct tests: 0
  - Largest gaps: service boundary, Zustand stores, event wiring, interactive workspace components

## Highest-Priority Gaps

### P0

- `src-tauri/src/loop_engine/step.rs`
  - Critical orchestration path with no direct tests
- `src-tauri/src/api/llm_client.rs`
  - Provider payload, streaming, tool-use, and error classification logic has no direct tests
- `src-tauri/src/verification/mod.rs`
  - Validation plus DB-recording pipeline has no direct tests
- `src/services/tauri.ts`, `src/services/events.ts`, all Zustand stores, and all React components
  - Frontend has no direct test suite at all
- `sidecar/src/research/router.py` and most `sidecar/src/research/*.py` clients
  - Large external API surface with no contract tests
- `sidecar/src/mcp/tools_loop.py`
  - Loop control boundary has no tests

### P1

- `src-tauri/src/api/sidecar.rs`
- `src-tauri/src/api/control.rs`
- `src-tauri/src/api/commands/loop_cmd.rs`
- `src-tauri/src/db/mod.rs`
- `src-tauri/src/db/proof_nodes.rs`
- `src-tauri/src/db/obligations.rs`
- `src-tauri/src/loop_engine/satisfaction.rs`
- `src-tauri/src/loop_engine/research.rs`
- `src-tauri/src/loop_engine/solver.rs`
- `sidecar/src/main.py`
- `sidecar/src/validation/router.py`
- `sidecar/src/agents/router.py`
- `sidecar/src/agents/scout.py`
- `sidecar/src/agents/council.py`
- `sidecar/src/training_data/router.py`

### P2

- Remaining Rust DB CRUD modules
- Remaining loop-engine helper modules
- Remaining Python router/client helpers
- Frontend presentational components after store/service coverage exists

## Feature-Level Gaps

### Rust Backend

- App bootstrap and runtime control are not directly tested
- Tauri command wrappers are mostly untested
- DB persistence is only partially covered for V14 tool-use and discerner paths
- Core proof-loop execution is largely untested outside parsing and a few heuristics
- Sidecar and LLM boundaries have no direct contract tests
- Verification pipeline has no direct tests

### Python Sidecar

- Validators are comparatively well covered
- FastAPI endpoint contracts are mostly untested
- Research routing and provider parsing are mostly untested
- MCP inspection and profile/problem tools are covered, but loop control is not
- Council, scout, and librarian agent behavior lacks dedicated tests

### Frontend

- There is no direct test coverage for services, stores, or components
- No UI integration tests exist for event-driven state updates from Tauri
- No contract tests exist for invoke command names, parameter shapes, or null handling

## File-by-File Gap Analysis

## Rust Backend

### `src-tauri/src/main.rs`

- `src-tauri/src/main.rs` — No direct tests. Add startup smoke tests for DB initialization, stale-attempt cleanup extraction, and command registration boundaries. Priority: High.

### `src-tauri/src/lib.rs`

- `src-tauri/src/lib.rs` — Module export file only. Compile coverage is probably enough; no dedicated tests needed unless exports become conditional. Priority: Low.

### `src-tauri/src/api`

- `src-tauri/src/api/mod.rs` — Structural module barrel. Compile-only coverage is acceptable. Priority: Low.
- `src-tauri/src/api/control.rs` — No direct tests. Add HTTP route tests for loop start/stop/status, invalid payloads, and error propagation when Tauri state is unavailable. Priority: High.
- `src-tauri/src/api/llm_client.rs` — No direct tests. Add provider-specific request builder tests, streaming chunk parser tests, tool-call fan-out tests, token accounting tests, and retryable vs fatal error classification tests. Priority: Critical.
- `src-tauri/src/api/sidecar.rs` — No direct tests. Add mocked HTTP tests for health, warmup polling, validation request serialization, timeout handling, and malformed response handling. Priority: High.

### `src-tauri/src/api/commands`

- `src-tauri/src/api/commands/mod.rs` — Structural command barrel. Compile-only coverage is acceptable. Priority: Low.
- `src-tauri/src/api/commands/analytics.rs` — No direct tests. Add DB-backed command tests for training-data stats shape and zero-data behavior. Priority: Medium.
- `src-tauri/src/api/commands/diagnostics.rs` — No direct tests. Add command tests for diagnostic filtering, ordering, and `sinceSequence` handling. Priority: Medium.
- `src-tauri/src/api/commands/export.rs` — No direct tests. Add filesystem-safe integration tests for export type selection, empty datasets, path errors, and row count correctness. Priority: High.
- `src-tauri/src/api/commands/loop_cmd.rs` — No direct tests. Add lifecycle tests for start/continue/pause/stop, force-stop behavior, event emission, and error reset behavior. Priority: High.
- `src-tauri/src/api/commands/management.rs` — No direct tests. Add command tests for attempts, claims, conflicts, DAG edges, reports, search, and delete-attempt flows. Priority: High.
- `src-tauri/src/api/commands/patterns.rs` — No direct tests. Add command tests for pattern search pass-through and empty result behavior. Priority: Medium.
- `src-tauri/src/api/commands/problem.rs` — No direct tests. Add command tests for create/get/list plus not-found and duplicate handling. Priority: Medium.
- `src-tauri/src/api/commands/profiles.rs` — No direct tests. Add command tests for save/load/list/delete/default-profile flows and null-option handling. Priority: Medium.
- `src-tauri/src/api/commands/proof.rs` — No direct tests. Add command tests for verified-chain retrieval and empty-chain behavior. Priority: Medium.
- `src-tauri/src/api/commands/research.rs` — No direct tests. Add command tests for API-key management and research proxy commands, especially null optional params. Priority: High.

### `src-tauri/src/db`

- `src-tauri/src/db/mod.rs` — Partial coverage through tool-use tests only. Add direct tests for schema initialization, stale cleanup, cascade deletion, problem/attempt/step lifecycle, and migration idempotence. Priority: High.
- `src-tauri/src/db/schema.rs` — Partial coverage for V14 table/column existence only. Missing backward-compat migration tests and failure-mode tests for pre-existing schemas. Priority: High.
- `src-tauri/src/db/branches.rs` — No direct tests. Add CRUD and status-transition tests plus branch-parent integrity checks. Priority: Medium.
- `src-tauri/src/db/claims.rs` — No direct tests. Add create/list query tests and attempt/step filtering tests. Priority: Medium.
- `src-tauri/src/db/conflicts.rs` — No direct tests. Add conflict insert/list/update tests and conflict-to-claim linkage tests. Priority: Medium.
- `src-tauri/src/db/contrastive.rs` — No direct tests. Add persistence/query tests for contrastive records and schema expectations. Priority: Medium.
- `src-tauri/src/db/dag_events.rs` — No direct tests. Add event ordering, sequence increment, and payload roundtrip tests. Priority: Medium.
- `src-tauri/src/db/decompositions.rs` — No direct tests. Add decomposition create/update/list tests and attempt cleanup behavior. Priority: Medium.
- `src-tauri/src/db/discerner.rs` — Partially covered by roundtrip in `discerner_tests.rs`. Missing query filtering, malformed payload, and multi-finding ordering tests. Priority: Medium.
- `src-tauri/src/db/edges.rs` — No direct tests. Add edge create/query tests and source/target filtering tests. Priority: Medium.
- `src-tauri/src/db/obligations.rs` — Partial V14 metadata coverage only. Missing assignment, priority update, closure, supersede, retraction, and open-count tests. Priority: High.
- `src-tauri/src/db/profiles.rs` — No direct tests. Add full CRUD, unique-name, default-selection, and updated-at semantics tests. Priority: Medium.
- `src-tauri/src/db/proof_nodes.rs` — No direct tests. Add node create/list/filter tests, obligation linkage tests, and sequence-number behavior. Priority: High.
- `src-tauri/src/db/reports.rs` — No direct tests. Add insert/query tests for after-action reports and problem/attempt scoping. Priority: Medium.
- `src-tauri/src/db/research.rs` — No direct tests. Add API-key storage, toggle, list, delete, and search-record persistence tests. Priority: Medium.
- `src-tauri/src/db/resolution_corpus.rs` — Partial coverage in tool-use integration tests. Missing hash-collision, duplicate registration, and ranking/usage edge-case tests. Priority: Medium.
- `src-tauri/src/db/scout_sessions.rs` — Partial coverage in tool-use integration tests. Missing status transitions and query-result association tests. Priority: Medium.
- `src-tauri/src/db/seed.rs` — No direct tests. Add seed idempotence tests and expected seeded-data assertions. Priority: Medium.
- `src-tauri/src/db/signals.rs` — No direct tests. Add satisfaction-signal record/list tests and latest-round retrieval semantics tests. Priority: High.
- `src-tauri/src/db/technique_registry.rs` — No direct tests. Add increment/list/ranking tests and merge/update behavior. Priority: Medium.
- `src-tauri/src/db/tool_policy.rs` — Partial coverage in tool-use tests. Missing duplicate violation handling and query filtering tests. Priority: Medium.
- `src-tauri/src/db/tool_runs.rs` — Partial coverage in tool-use tests. Missing partial-update, step-backfill, and error-status transition tests. Priority: Medium.

### `src-tauri/src/verification`

- `src-tauri/src/verification/mod.rs` — No direct tests. Add mocked-sidecar tests for validator aggregation, rejection string building, DB write behavior, and optional Lean handling. Priority: High.

### `src-tauri/src/loop_engine`

- `src-tauri/src/loop_engine/mod.rs` — Has internal tests for proposal parsing and review parsing only. Missing outer-loop lifecycle, retry logic, attempt transitions, and event emission coverage. Priority: High.
- `src-tauri/src/loop_engine/step.rs` — No direct tests despite being the largest hot-path file. Add focused tests for one-step execution, obligation targeting, correction flow, fan-in handling, critic/adversary paths, audit insertion, and event emission. Priority: Critical.
- `src-tauri/src/loop_engine/solver.rs` — No direct tests. Add prompt-builder tests for open-obligation gating, suspected-answer sections, research context, attempt constraints, and obligation-specific prompt sections. Priority: High.
- `src-tauri/src/loop_engine/satisfaction.rs` — No direct tests. Add tests for vote aggregation, closure thresholds, and latest-round selection. Priority: High.
- `src-tauri/src/loop_engine/review.rs` — No direct tests. Add tests for review-result parsing and completion gating behavior. Priority: High.
- `src-tauri/src/loop_engine/research.rs` — No direct tests. Add tests for scout-trigger rules, result formatting, confidence handling, and no-result behavior. Priority: High.
- `src-tauri/src/loop_engine/orchestrator.rs` — No direct tests. Add worker-rotation and failure-escalation tests. Priority: High.
- `src-tauri/src/loop_engine/obligation_queue.rs` — No direct tests. Add priority selection, pivot forcing, blacklist, and stuck-step behavior tests. Priority: High.
- `src-tauri/src/loop_engine/critic.rs` — No direct tests. Add critic-prompt construction and model-name normalization tests. Priority: High.
- `src-tauri/src/loop_engine/decomposer.rs` — No direct tests. Add decomposition request/response and event emission tests. Priority: Medium.
- `src-tauri/src/loop_engine/discerner.rs` — Partial coverage through `discerner_tests.rs` only. Missing prompt routing, event emission, and DB-integration tests. Priority: Medium.
- `src-tauri/src/loop_engine/audit.rs` — No direct tests. Add `parse_audit` and breadth-score edge-case tests. Priority: Medium.
- `src-tauri/src/loop_engine/claim_extractor.rs` — No direct tests. Add extraction tests for numeric claims, malformed claims, and duplicate suppression. Priority: High.
- `src-tauri/src/loop_engine/context_enricher.rs` — No direct tests. Add prompt-context assembly tests and truncation/relevance tests. Priority: Medium.
- `src-tauri/src/loop_engine/json_parse.rs` — No direct tests. Add isolated malformed-JSON extraction tests instead of relying only on `mod.rs` helpers. Priority: Medium.
- `src-tauri/src/loop_engine/patterns.rs` — No direct tests. Add pattern-injection and filtering tests. Priority: Medium.
- `src-tauri/src/loop_engine/worker_pool.rs` — No direct tests. Add worker-selection and pool-config tests. Priority: Medium.
- `src-tauri/src/loop_engine/contradiction.rs` — Has unit tests for severity heuristics. Missing DB/event integration tests and duplicate suppression tests. Priority: Medium.
- `src-tauri/src/loop_engine/evidence.rs` — Has unit tests for evidence extraction and conflict detection. Missing integration tests against real obligation lists and step flows. Priority: Medium.
- `src-tauri/src/loop_engine/response_guard.rs` — Has unit tests for repetition detection. Missing stream-integration tests in the LLM path. Priority: Medium.

### `src-tauri/src/models`

- `src-tauri/src/models/mod.rs` — Structural module barrel. Compile-only coverage is acceptable. Priority: Low.
- `src-tauri/src/models/agents.rs` — No direct tests. Add serde roundtrip tests for multi-agent config if command/API drift becomes common. Priority: Low.
- `src-tauri/src/models/council.rs` — No direct tests. Low risk; serde roundtrip tests would be sufficient. Priority: Low.
- `src-tauri/src/models/dag.rs` — No direct tests. Add serde roundtrip and default-field tests for obligation/proof-node models. Priority: Low.
- `src-tauri/src/models/management.rs` — No direct tests. Low risk; type-shape tests only if IPC drift appears. Priority: Low.
- `src-tauri/src/models/patterns.rs` — No direct tests. Compile-only coverage is acceptable for now. Priority: Low.
- `src-tauri/src/models/proof.rs` — No direct tests. Add serde roundtrip tests only if IPC mismatch bugs recur. Priority: Low.
- `src-tauri/src/models/research.rs` — No direct tests. Compile-only coverage is acceptable for now. Priority: Low.
- `src-tauri/src/models/tool_use.rs` — No direct tests. Add serde roundtrip tests for tool-run payloads if export/API drift appears. Priority: Low.

## Python Sidecar

### `sidecar/src/main.py`

- `sidecar/src/main.py` — No direct FastAPI app tests. Add `TestClient` tests for `/health`, router mounting, and lifespan Lean warmup behavior under available/unavailable Lean. Priority: High.

### `sidecar/src/validation`

- `sidecar/src/validation/__init__.py` — Structural package file. Import smoke is sufficient. Priority: Low.
- `sidecar/src/validation/router.py` — No direct endpoint tests. Add API-contract tests for equality, typed claims, Lean skip behavior, and malformed request bodies. Priority: High.
- `sidecar/src/validation/sympy_validator.py` — Good direct coverage for algebraic validation and sandbox boundaries, but missing parser-error, timeout/performance, Piecewise edge, and large-expression regression tests. Priority: Medium.
- `sidecar/src/validation/pint_validator.py` — Only smoke-level coverage. Add unit tests for invalid units, dimension mismatch, parse errors, and empty input behavior. Priority: Medium.
- `sidecar/src/validation/lean_validator.py` — Good unit coverage plus gated integration tests. Missing explicit tests for runtime-unavailable envs, stderr-only failure cases, and warmup-state behavior. Priority: Medium.
- `sidecar/src/validation/typed_claims.py` — Strong direct coverage. Missing malformed domain syntax, nested predicate edge cases, and unsupported predicate operator tests. Priority: Medium.

### `sidecar/src/patterns`

- `sidecar/src/patterns/__init__.py` — Structural package file. Import smoke is sufficient. Priority: Low.
- `sidecar/src/patterns/router.py` — No direct endpoint tests. Add tests for request validation, extraction output shape, and empty-input behavior. Priority: Medium.

### `sidecar/src/training_data`

- `sidecar/src/training_data/__init__.py` — Structural package file. Import smoke is sufficient. Priority: Low.
- `sidecar/src/training_data/router.py` — No direct endpoint tests. Add stats/export endpoint contract tests, empty-state behavior, and file-output error handling. Priority: High.

### `sidecar/src/agents`

- `sidecar/src/agents/__init__.py` — Structural package file. Import smoke is sufficient. Priority: Low.
- `sidecar/src/agents/router.py` — No direct endpoint tests. Add request-model validation and route-to-agent tests for council, scout, reconnaissance, and librarian endpoints. Priority: High.
- `sidecar/src/agents/reconnaissance.py` — Partial direct coverage for query building, regex extraction, and empty flow only. Missing briefing content, source-priority behavior, error aggregation, and end-to-end mocked source tests. Priority: High.
- `sidecar/src/agents/scout.py` — No direct tests. Add async tests for multi-source search, result normalization, and failure handling. Priority: High.
- `sidecar/src/agents/council.py` — No direct tests. Add transcript parsing, finding extraction, and no-model/error behavior tests. Priority: High.
- `sidecar/src/agents/librarian.py` — No direct tests. Add pattern-action decision tests and malformed finding handling tests. Priority: Medium.

### `sidecar/src/research`

- `sidecar/src/research/__init__.py` — Structural package file. Import smoke is sufficient. Priority: Low.
- `sidecar/src/research/router.py` — No direct tests. Add endpoint tests for each supported source family, invalid-source errors, and `get`/citation behaviors. Priority: High.
- `sidecar/src/research/config.py` — No direct tests. Add env/config resolution tests and API-key fallback tests. Priority: Medium.
- `sidecar/src/research/arxiv_client.py` — No direct tests. Add HTTP response parsing and malformed-feed tests. Priority: High.
- `sidecar/src/research/crossref_client.py` — No direct tests. Add DOI/search response parsing and empty-result tests. Priority: Medium.
- `sidecar/src/research/google_search.py` — No direct tests. Add HTML parsing and no-result tests with mocked HTTP. Priority: Medium.
- `sidecar/src/research/huggingface_client.py` — No direct tests. Add model/dataset response parsing tests and sort/task parameter tests. Priority: Medium.
- `sidecar/src/research/lean_ecosystem.py` — No direct tests. Add loogle/reservoir parsing and failure-path tests. Priority: Medium.
- `sidecar/src/research/oeis_client.py` — No direct tests. Add search and get-sequence parsing tests. Priority: Medium.
- `sidecar/src/research/openalex_client.py` — No direct tests. Add search result normalization and pagination field tests. Priority: Medium.
- `sidecar/src/research/pubmed_client.py` — No direct tests. Add search/get parsing tests and `db` parameter coverage. Priority: Medium.
- `sidecar/src/research/semantic_scholar.py` — No direct tests. Add search/get response parsing and citation-direction tests. Priority: High.
- `sidecar/src/research/stackexchange_client.py` — No direct tests. Add site-switching and combined-search normalization tests. Priority: Medium.
- `sidecar/src/research/tavily_client.py` — No direct tests. Add API-key missing and result-format tests. Priority: Medium.
- `sidecar/src/research/unpaywall_client.py` — No direct tests. Add DOI lookup and no-PDF cases. Priority: Medium.
- `sidecar/src/research/wolfram_client.py` — No direct tests. Add response-format and error normalization tests. Priority: Medium.

### `sidecar/src/mcp`

- `sidecar/src/mcp/__init__.py` — Structural package file. Import smoke is sufficient. Priority: Low.
- `sidecar/src/mcp/db.py` — Indirectly exercised by tool tests but not directly tested. Add DB-path resolution and connection error tests. Priority: Medium.
- `sidecar/src/mcp/discerner.py` — Directly covered by `test_discerner.py`. Missing malformed-event and recommendation-stability edge cases only. Priority: Low.
- `sidecar/src/mcp/tools_problem.py` — Directly covered by `test_mcp_tools.py`. Missing malformed input and DB connection failure tests. Priority: Medium.
- `sidecar/src/mcp/tools_profile.py` — Directly covered by `test_mcp_tools.py`. Missing invalid JSON config and delete-missing-profile tests. Priority: Medium.
- `sidecar/src/mcp/tools_inspect.py` — Directly covered by `test_mcp_tools.py`. Missing empty-attempt and malformed payload tests. Priority: Medium.
- `sidecar/src/mcp/tools_diagnose.py` — Directly covered by `test_mcp_tools.py`. Missing no-data and malformed diagnostic payload tests. Priority: Medium.
- `sidecar/src/mcp/tools_intervene.py` — Directly covered by `test_mcp_tools.py`. Missing invalid-obligation and DB-failure tests. Priority: Medium.
- `sidecar/src/mcp/tools_knowledge.py` — Directly covered by `test_mcp_tools.py`. Missing pagination/query-normalization tests. Priority: Low.
- `sidecar/src/mcp/tools_loop.py` — No direct tests. Add HTTP success, connect-error, status-error, and malformed-response tests for start/stop/pause/status. Priority: High.

### `sidecar/src/other`

- `sidecar/src/__init__.py` — Structural package file. Import smoke is sufficient. Priority: Low.
- `sidecar/src/modification/__init__.py` — Empty/structural only. No dedicated tests needed today. Priority: Low.
- `sidecar/src/sdk/__init__.py` — Empty/structural only. No dedicated tests needed today. Priority: Low.
- `sidecar/src/seeding/__init__.py` — Empty/structural only. No dedicated tests needed today. Priority: Low.

## Frontend

### App bootstrap and services

- `src/main.tsx` — No direct tests. Add app bootstrap smoke test once jsdom/RTL harness exists. Priority: Medium.
- `src/App.tsx` — No direct tests. Add top-level integration tests for event hookup, workspace selection, and panel rendering. Priority: High.
- `src/services/tauri.ts` — No direct tests. Add mocked `invoke` contract tests for command names, parameter shapes, and `null` vs `undefined` behavior. Priority: Critical.
- `src/services/events.ts` — No direct tests. Add mocked Tauri event tests for subscription fan-out and proper unlisten cleanup. Priority: Critical.
- `src/utils/latex.ts` — No direct tests. Add formatting and error-tolerant rendering tests. Priority: Medium.
- `src/types/index.ts` — Type-only file. Compile coverage exists; optional runtime schema/contract tests if IPC drift continues. Priority: Low.
- `src/styles/global.css` — No automated visual or snapshot coverage. Add visual regression or manual UI checklist if styling bugs become common. Priority: Low.

### Zustand stores

- `src/stores/problemStore.ts` — No direct tests. Add create/fetch/active-problem state transition tests. Priority: High.
- `src/stores/loopStore.ts` — No direct tests. Add lifecycle tests for solve start/continue/pause/stop, profile persistence, obligations, and event-driven updates. Priority: Critical.
- `src/stores/agentStore.ts` — No direct tests. Add event accumulation and reset/filter tests. Priority: High.
- `src/stores/diagnosticStore.ts` — No direct tests. Add diagnostic append/filter/since-sequence tests. Priority: High.
- `src/stores/navigationStore.ts` — No direct tests. Add route/selection state tests. Priority: Medium.
- `src/stores/workspaceStore.ts` — No direct tests. Add workspace selection and panel state tests. Priority: Medium.

### Shared components

- `src/components/shared/Header.tsx` — No direct tests. Add basic render and status indicator tests. Priority: Medium.
- `src/components/shared/Sidebar.tsx` — No direct tests. Add problem list render and selection tests. Priority: Medium.
- `src/components/shared/Breadcrumb.tsx` — No direct tests. Add simple render/path-state tests. Priority: Low.

### Problem and workspace flow

- `src/components/problem/ProblemInput.tsx` — No direct tests. Add input validation, submit behavior, and disabled/loading state tests. Priority: High.
- `src/components/workspace/ProblemWorkspace.tsx` — No direct tests. Add integration tests for workspace composition and active-panel switching. Priority: High.
- `src/components/library/ProblemLibrary.tsx` — No direct tests. Add search/list/select tests. Priority: Medium.

### Loop and diagnostics flow

- `src/components/loop/LoopControls.tsx` — No direct tests. Add start/pause/stop button state and callback tests. Priority: High.
- `src/components/loop/ThinkingPanel.tsx` — No direct tests. Add token stream/status render tests. Priority: Medium.
- `src/components/loop/MiniTokenStream.tsx` — No direct tests. Add truncation/render tests. Priority: Medium.
- `src/components/diagnostics/DiagnosticPanel.tsx` — No direct tests. Add event list, filter, and empty-state tests. Priority: High.
- `src/components/agent/AgentDashboard.tsx` — No direct tests. Add multi-panel render tests for orchestrator/critic/scout data. Priority: High.

### Attempt, proof, obligation, and DAG views

- `src/components/attempt/AttemptDetail.tsx` — No direct tests. Add composed-attempt render tests. Priority: High.
- `src/components/attempt/ProofChain.tsx` — No direct tests. Add verified-step ordering and empty-state tests. Priority: Medium.
- `src/components/attempt/ObligationQueue.tsx` — No direct tests. Add queue render and status indicator tests. Priority: High.
- `src/components/attempt/ObligationLanes.tsx` — No direct tests. Add lane grouping and priority/state render tests. Priority: High.
- `src/components/attempt/ClaimsPanel.tsx` — No direct tests. Add claim-list render tests. Priority: Medium.
- `src/components/attempt/ConflictsPanel.tsx` — No direct tests. Add conflict severity/render tests. Priority: Medium.
- `src/components/proof/ProofTree.tsx` — No direct tests. Add proof-tree render and selection tests. Priority: Medium.
- `src/components/proof/ObligationGraph.tsx` — No direct tests. Add graph-node render and empty-state tests. Priority: Medium.
- `src/components/dag/UnifiedDagView.tsx` — No direct tests. Add DAG panel render and edge/node interaction tests. Priority: High.
- `src/components/obligation/ObligationDetailView.tsx` — No direct tests. Add obligation detail and signals render tests. Priority: Medium.
- `src/components/step/StepDetailView.tsx` — No direct tests. Add verified/rejected step detail render tests. Priority: Medium.

### Analytics, export, and settings

- `src/components/analytics/TrainingDataStats.tsx` — No direct tests. Add stats rendering and loading/error state tests. Priority: Medium.
- `src/components/analytics/TrainingDataTable.tsx` — No direct tests. Add row rendering and empty-state tests. Priority: Medium.
- `src/components/analytics/AfterActionReport.tsx` — No direct tests. Add report render and missing-data tests. Priority: Medium.
- `src/components/export/ExportPanel.tsx` — No direct tests. Add export action, validation, and disabled-state tests. Priority: High.
- `src/components/settings/AgentProfilePanel.tsx` — No direct tests. Add save/load/delete/default profile interaction tests. Priority: High.
- `src/components/settings/ResearchApiPanel.tsx` — No direct tests. Add key add/delete/toggle interaction tests. Priority: High.

## Recommended Test Build-Out Order

1. Frontend harness
   - Add Vitest + jsdom + React Testing Library
   - Add first tests for `src/services/tauri.ts`, `src/services/events.ts`, and `src/stores/loopStore.ts`
2. Rust hot path
   - `src-tauri/src/api/llm_client.rs`
   - `src-tauri/src/verification/mod.rs`
   - `src-tauri/src/loop_engine/step.rs`
   - `src-tauri/src/api/sidecar.rs`
3. Rust DB lifecycle
   - `src-tauri/src/db/mod.rs`
   - `src-tauri/src/db/obligations.rs`
   - `src-tauri/src/db/proof_nodes.rs`
   - `src-tauri/src/db/signals.rs`
4. Python HTTP contracts
   - `sidecar/src/main.py`
   - `sidecar/src/validation/router.py`
   - `sidecar/src/agents/router.py`
   - `sidecar/src/research/router.py`
5. Python external clients
   - `sidecar/src/research/*.py`
   - `sidecar/src/mcp/tools_loop.py`

## Notes

- The frontend is the largest absolute testing hole because it has zero direct tests.
- The Rust backend is the highest risk testing hole because the hot path exists but is mostly untested.
- The Python sidecar has the healthiest current test surface, but it is concentrated in validators rather than routers and external-provider boundaries.
- Before treating frontend or Python gates as reliable, fix the missing ESLint config and harden pytest temp-dir behavior in restricted environments.
