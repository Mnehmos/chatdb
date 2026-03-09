# Project Knowledge

This document maps the current ChatDB codebase as it exists in this repository today. It is meant to be a practical orientation file: what each part of the app does, how the layers connect, what dependencies each layer owns, and where the main integration boundaries are.

## 0. Current Project Phase

The original sprint roadmap described in the design docs has now been completed. This file should be read as a post-sprint reference for the live system, not as a roadmap snapshot.

That matters for interpretation:

- the design philosophy files still explain why the architecture looks the way it does
- this document describes the implemented application shape as it exists after the final planned sprint
- any remaining gaps called out here are code-level realities in the repository, not roadmap intentions

## 1. What the App Is

ChatDB is a desktop application for running an autonomous proof-search loop and storing the resulting proof attempts, validation outcomes, and training data in SQLite.

The app is built as four cooperating pieces:

- React renders the desktop UI.
- Tauri hosts the desktop shell and provides the IPC bridge.
- Rust is the main application backend, loop engine, API surface, and database owner.
- Python is a sidecar service that performs validation and research-oriented helper work.

The core architectural rule is still visible in the code: the database is the long-term memory. The frontend is mostly a projection of persisted data plus live event streams from the Rust loop.

## 2. High-Level Runtime Topology

```text
React UI (src/)
  -> typed Tauri invoke calls (src/services/tauri.ts)
Tauri + Rust backend (src-tauri/src/)
  -> SQLite reads/writes via Database (src-tauri/src/db/)
  -> outbound HTTP to Python sidecar (validation, research, scout)
  -> outbound HTTP to LLM providers (Anthropic/OpenAI/OpenRouter/Gemini)
  -> emits Tauri events back to the UI
Python sidecar (sidecar/src/)
  -> validators, research adapters, council/scout/librarian endpoints
SQLite (chatdb.sqlite)
  -> canonical store for problems, attempts, steps, DAG state, reports, profiles, research keys
```

The main execution flow is:

1. A user creates or selects a problem in the React UI.
2. The UI calls a Tauri command through `invoke()`.
3. Rust creates or updates DB state and may start the solve loop.
4. `LoopEngine` calls an LLM, parses the proposed step, and sends it to the sidecar for validation.
5. The verification pipeline records the step, proof node, and DAG event in SQLite.
6. Rust emits real-time events such as `loop:step_complete`, `loop:obligation_opened`, and diagnostic events.
7. The frontend listens for those events and updates Zustand stores so the UI redraws immediately.

## 3. Technology Stack and Dependencies

### Frontend

- React 19
- TypeScript 5.5
- Vite 6
- Zustand 5 for state
- Immer 10
- KaTeX for math rendering
- `@tauri-apps/api` for IPC and events
- `@tauri-apps/plugin-shell` available in the desktop runtime

Primary frontend manifest: `package.json`

Key scripts:

- `npm run dev` starts the Vite dev server
- `npm run tauri:dev` starts the desktop app in development
- `npm run tauri:build` builds the desktop bundle
- `npm run sidecar:dev` runs the Python sidecar with Uvicorn
- `npm run test` runs Vitest
- `npm run lint` runs ESLint

### Rust Backend

- Rust 2021 edition
- Tauri 2
- Tokio async runtime
- Reqwest for HTTP
- Rusqlite with bundled SQLite
- Axum for the control-plane HTTP server
- Tower HTTP CORS middleware
- Serde / Serde JSON
- UUID
- Chrono
- Tracing + tracing-subscriber + tracing-appender
- Dotenvy
- Regex

Primary backend manifest: `src-tauri/Cargo.toml`

### Python Sidecar

- Python 3.11+
- FastAPI
- Uvicorn
- SymPy
- Pint
- HTTPX
- Pydantic 2
- AnyIO
- MCP Python package

Optional extras:

- `arxiv`
- `semanticscholar`

Primary sidecar manifest: `sidecar/pyproject.toml`

### External Services

- Anthropic API
- OpenAI API
- OpenRouter API
- Gemini API
- Optional Lean 4 environment for deeper proof checks
- Optional research providers exposed through the sidecar

These are not hard-coded as secrets in the repo. The Rust loop reads model API keys from environment variables named in model profiles, such as `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENROUTER_API_KEY`, and `GEMINI_API_KEY`.

## 4. Frontend Structure

The frontend is a React single-page app wrapped by Tauri. It does not talk directly to the Python sidecar. All backend interactions go through Tauri invoke calls into Rust.

### Entry and App Shell

- `src/main.tsx` bootstraps React and loads `src/styles/global.css`.
- `src/App.tsx` is the main coordinator:
  - fetches the problem list on load
  - loads agent profiles into the loop store
  - loads problem steps when navigation changes
  - subscribes to the full Tauri event stream through `onLoopEvent()`
  - routes between library, workspace, attempt, and step views

### Services Layer

- `src/services/tauri.ts` is the typed client wrapper around all Tauri commands.
  - Problem CRUD
  - Loop controls
  - Proof and obligation queries
  - Analytics and exports
  - Profiles
  - Diagnostics
  - Management system reads
  - Research API key management and search commands
- `src/services/events.ts` centralizes the list of Tauri events the UI listens to.

This is the frontend integration seam: components and stores import these wrappers instead of calling Tauri directly.

### State Stores

The UI state is split by concern using Zustand:

- `src/stores/problemStore.ts`
  - problem list
  - active problem
  - create/fetch actions
- `src/stores/loopStore.ts`
  - live run status
  - streamed steps
  - selected models and full agent config
  - profiles
  - obligations, critic checks, discerner findings
  - start/pause/stop/continue actions
- `src/stores/workspaceStore.ts`
  - persisted attempt-specific data loaded from the DB
  - attempts, reports, claims, conflicts, stored steps, stored obligations
- `src/stores/navigationStore.ts`
  - current view level and selected problem/attempt/step IDs
- `src/stores/agentStore.ts`
  - orchestrator, critic, council, and scout activity logs
- `src/stores/diagnosticStore.ts`
  - runtime event log
  - sidecar / Lean / loop health
  - filters and panel UI state

The important split is this:

- `loopStore` is the live in-memory view of the active run.
- `workspaceStore` is the persisted historical view loaded back from SQLite.

That lets the app show active runs immediately while still supporting navigation into older attempts.

### Main UI Sections

#### Problem Library

Files:

- `src/components/library/ProblemLibrary.tsx`
- `src/components/problem/ProblemInput.tsx`
- `src/components/export/ExportPanel.tsx`

Responsibilities:

- Lists all stored problems
- Filters by text, status, and domain
- Creates new problems
- Launches into a workspace
- Offers bulk export

#### Problem Workspace

File:

- `src/components/workspace/ProblemWorkspace.tsx`

Responsibilities:

- Displays the selected problem statement and metadata
- Hosts the solve controls
- Shows live attempt progress
- Lists all attempts for the problem
- Shows cross-attempt report aggregates
- Exposes per-problem export

This is the main operational screen for a problem.

#### Attempt Detail

Files:

- `src/components/attempt/AttemptDetail.tsx`
- `src/components/attempt/ObligationQueue.tsx`
- `src/components/attempt/ObligationLanes.tsx`
- `src/components/attempt/ProofChain.tsx`
- `src/components/attempt/ClaimsPanel.tsx`
- `src/components/attempt/ConflictsPanel.tsx`

Responsibilities:

- Shows one attempt in depth
- Uses live store data if the attempt is currently running
- Falls back to DB-loaded data for historical attempts
- Renders obligation-driven views when obligations exist
- Renders claim extraction and conflict tracking panels
- Shows an after-action summary if one exists

#### Step Detail

File:

- `src/components/step/StepDetailView.tsx`

Responsibilities:

- Displays one step's natural language, formal expression, and reasoning
- Shows validator outcomes (SymPy, Pint, Lean)
- Shows adversarial challenge results if present
- Loads claims attached to that step
- Loads incoming and outgoing DAG edges for that step

#### Live Loop Panels

Files:

- `src/components/loop/LoopControls.tsx`
- `src/components/loop/ThinkingPanel.tsx`
- `src/components/analytics/AfterActionReport.tsx`
- `src/components/settings/AgentProfilePanel.tsx`
- `src/components/settings/ResearchApiPanel.tsx`

Responsibilities:

- Start, continue, pause, stop, retry, and review solves
- Show selected model configuration
- Stream model tokens and sub-agent thinking in real time
- Show collaborative fan-in rounds and worker-specific outcomes
- Open modal panels for after-action review, agent profiles, and research API settings

`ThinkingPanel` listens to low-level streaming events like `loop:thinking_start`, `loop:token`, and `loop:thinking_end`, so it behaves more like a terminal log window than a static component.

The attempt-facing views also now expose collaborative solver state directly:

- `ProofChain` shows solver worker badges
- stale sibling candidates are rendered as downgraded results instead of looking like ordinary rejections
- `ObligationDetailView` can show the active fan-in worker list and collaborative round ID for the selected obligation

#### Navigation and Diagnostics

Files:

- `src/components/shared/Header.tsx`
- `src/components/shared/Sidebar.tsx`
- `src/components/shared/Breadcrumb.tsx`
- `src/components/diagnostics/DiagnosticPanel.tsx`

Responsibilities:

- Global app branding and navigation context
- Fast switching between problems
- Breadcrumb navigation across library -> workspace -> attempt -> step
- Runtime diagnostics with category/severity filters and system health polling

## 5. Rust Backend Structure

Rust is the real application core. Tauri commands are thin entry points; the database layer, loop engine, and verification path do the heavy lifting.

### Entry Point and Process Setup

Files:

- `src-tauri/src/main.rs`
- `src-tauri/src/lib.rs`

Responsibilities:

- Load `.env` files
- Initialize rolling file logging under `logs/`
- Open and migrate `chatdb.sqlite`
- Build shared `AppState`
- Register all Tauri commands
- Start the auxiliary Axum control server on `127.0.0.1:9744`

`AppState` stores:

- `db: Database`
- `loop_running`
- `current_attempt_id`
- `app_handle`

Only the DB is durable. The rest is runtime process state needed to coordinate the active loop and emit events.

### API Surface

Files:

- `src-tauri/src/api/commands/*.rs`
- `src-tauri/src/api/control.rs`
- `src-tauri/src/api/sidecar.rs`
- `src-tauri/src/api/llm_client.rs`

Command groups exposed to the frontend:

- `problem`
- `loop_cmd`
- `proof`
- `patterns`
- `analytics`
- `profiles`
- `diagnostics`
- `management`
- `export`
- `research`

The control-plane server in `control.rs` exposes a second interface over HTTP for external tools:

- `POST /loop/start`
- `POST /loop/stop`
- `POST /loop/pause`
- `GET /loop/status`

That control API exists so the MCP tooling can drive the loop without going through the desktop UI.

### Database Layer

Files:

- `src-tauri/src/db/mod.rs`
- `src-tauri/src/db/schema.rs`
- supporting query modules under `src-tauri/src/db/`

Responsibilities:

- Own all SQLite schema creation and migrations
- Expose typed CRUD and reporting methods
- Record every step, attempt, node, obligation, signal, report, profile, and research key
- Keep problem and attempt counters in sync

Startup behavior:

- Enables WAL mode
- Enables foreign keys
- Applies base schema plus migrations V3 through V15
- Seeds the technique registry

This module is the persistence contract for the entire application.

### Loop Engine

Files:

- `src-tauri/src/loop_engine/mod.rs`
- plus submodules for solver, orchestrator, critic, review, decomposer, obligation queue, contradiction detection, context enrichment, research, and step parsing

Responsibilities:

- Start and run proof attempts
- Build solver worker pools plus reviewer, adversary, critic, discerner, and decomposer clients from the selected configuration
- Manage retries across multiple attempts
- Track obligations, decomposition, contradiction handling, and branching
- Emit diagnostics and UI events during execution
- Run post-attempt review and persist after-action reports

Operationally, this is the "brainstem" of the app. It coordinates the call sequence but still writes all durable state through the `db` module.

#### Current Step Execution Architecture (`step.rs`)

The step executor was recently tightened so the main step path is split into three clearer layers inside `src-tauri/src/loop_engine/step.rs`:

- `call_solver()`
  - owns the raw LLM call
  - runs the tool-use loop
  - streams token events
  - handles repetition detection
  - is structured to stay `Send + 'static` safe so it can run inside spawned async tasks
- `process_solver_result()`
  - performs the serial post-processing path after the LLM returns
  - parses the model output
  - runs validation
  - records results
  - applies adversary checks, satisfaction tallies, audit logic, and orchestrator follow-up
- `run_step()`
  - coordinates the round
  - runs the mid-step discerner trigger
  - selects obligations
  - chooses between freeform mode, distinct-obligation parallel mode, and same-obligation fan-in mode

The current execution modes are:

- Freeform mode
  - used when no obligations are selectable
  - builds one general solver prompt
  - calls `call_solver()`
  - immediately passes the result into `process_solver_result()`
- Parallel obligation mode
  - used when one or more obligations are available
  - calls `select_multiple()` from `obligation_queue.rs`
  - selects up to 3 distinct obligations for the round
  - reserves step numbers up front from the current base step
  - spawns the solver calls in parallel with `tokio::task::JoinSet`
  - collects all results, sorts them by reserved step number, then processes them serially through the full post-processing pipeline
- Same-obligation fan-in mode
  - used when exactly 1 obligation is eligible, fan-in is enabled, and at least 2 solver workers are configured
  - dispatches multiple solver workers against that same obligation in parallel
  - caps worker count through `max_fanin_workers`
  - assigns a shared `solver_round_id` plus per-worker dispatch metadata
  - re-checks the obligation state before each serial post-processing pass
  - records later siblings as stale if an earlier sibling already closed or changed the target obligation

The fan-in path means the loop can keep multiple solvers busy even when the obligation graph narrows to a single eligible node.

This is an important boundary in the live system:

- LLM generation can now happen in parallel across distinct obligations and, when needed, across multiple workers on the same obligation
- validation, DB mutation, and downstream state transitions still stay serial and deterministic

That preserves throughput without changing the core persistence or UI contracts.

The fan-in implementation also added explicit collaborative-round events:

- `loop:fanin_round_start`
- `loop:fanin_round_update`
- `loop:fanin_round_complete`

### LLM Client

File:

- `src-tauri/src/api/llm_client.rs`

Responsibilities:

- Unified wrapper for Anthropic, OpenAI, OpenRouter, and Gemini
- Supports standard completion and streaming completion
- Handles provider-specific token limit fields
- Supports model tool-use / function-calling flows
- Captures token counts for persistence in training data

This abstraction is what lets the frontend choose providers without changing the rest of the loop.

### Verification Pipeline

File:

- `src-tauri/src/verification/mod.rs`

Responsibilities:

- Send proposed steps to the sidecar for validation
- Build the rejection reason from validator outputs
- Record the step in `steps`
- Dual-write a matching proof node in `proof_nodes`
- Append a `dag_event`

This module is the key bridge between "LLM output" and "persisted structured artifact."

## 6. Database Model and What Lives There

The database schema is broad because it stores both product data and operational traces.

### Core Proof Tables

- `problems`
- `attempts`
- `steps`
- `patterns`
- `modifications`

These hold the user-facing proof work and reusable patterns.

The `steps` table now also carries collaborative solver metadata:

- `solver_round_id`
- `solver_worker_id`
- `solver_dispatch_mode`
- `stale_sibling`

That lets the DB distinguish:

- ordinary single-worker steps
- distinct-obligation parallel steps
- same-obligation fan-in candidates
- candidates that became obsolete because an earlier sibling already resolved the target

### Agent and Training Data Tables

- `orchestrator_decisions`
- `critic_evaluations`
- `council_sessions`
- `council_findings`
- `scout_queries`
- `librarian_actions`
- `research_cache`
- `contrastive_pairs`

These capture the supervision and meta-reasoning traces that the project treats as part of the product.

### DAG and Obligation Tables

- `proof_nodes`
- `obligations`
- `dag_events`
- `technique_registry`
- `branches`
- `decompositions`
- `obligation_signals`

These support the obligation-driven proof search model rather than a plain linear transcript.

The `obligations` table now also stores collaborative assignment metadata:

- `assigned_model` remains the legacy single-string field
- `assigned_models_json` stores all active collaborating workers during a fan-in round
- `active_solver_round_id` links the obligation to the current collaborative round

This preserves backward compatibility for older views while making same-obligation collaboration visible to the UI and DB queries.

### Management and Diagnostics Tables

- `claims`
- `dag_edges`
- `conflicts`
- `after_action_reports`
- `agent_profiles`
- `discerner_decisions`
- `discerner_findings`
- `research_api_keys`

These support explainability, run review, profile persistence, and external integrations.

## 7. Python Sidecar Structure

The sidecar is a stateless FastAPI service. It performs validation and helper work, but it is not the source of truth for application state.

### Entry Point

File:

- `sidecar/src/main.py`

Responsibilities:

- Create the FastAPI app
- Mount routers
- Expose `/health`
- Warm Lean in the background if Lean is installed

### Validation Router

File:

- `sidecar/src/validation/router.py`

Endpoints:

- `POST /validate/step`
- `POST /validate/sympy_check`

Behavior:

- Uses SymPy for equality-style algebraic checks
- Uses Pint only when the expression likely contains physical units
- Uses Lean only when requested and available
- Treats SymPy as the main gatekeeper
- Treats Lean as advisory, not as a veto on `all_passed`

### Agents Router

File:

- `sidecar/src/agents/router.py`

Endpoints:

- `POST /agents/council/deliberate`
- `POST /agents/scout/query`
- `POST /agents/librarian/process`

This is where the Python helper agents are exposed to Rust.

### Research Router

File:

- `sidecar/src/research/router.py`

Endpoints:

- `POST /research/search`
- `POST /research/get`
- `POST /research/citations`
- `POST /research/multi`
- `GET /research/sources`

Supported source families in current code include:

- arXiv
- Semantic Scholar
- OEIS
- Crossref
- OpenAlex
- Wolfram Alpha
- HuggingFace models and datasets
- Loogle / Lean ecosystem search
- Reservoir
- PubMed
- Unpaywall

### Training Data and Pattern Routers

Files:

- `sidecar/src/training_data/router.py`
- `sidecar/src/patterns/router.py`

Current state:

- present and mounted
- still mostly placeholders or stubs

### MCP Support

Files:

- `sidecar/mcp_server.py`
- `sidecar/src/mcp/*.py`

This provides a stdio MCP server for external assistants/tools. It uses:

- the control API in the running Tauri app
- the sidecar HTTP API
- direct DB inspection helpers

That makes ChatDB usable as a tool-backed service, not only as a desktop UI.

## 8. How the Layers Wire Together

### Frontend to Rust

- The frontend uses `invoke()` through `src/services/tauri.ts`.
- Rust exposes commands with `#[tauri::command]`.
- The commands call into `db`, `loop_engine`, and other backend modules.

### Rust to Frontend

- Rust emits Tauri events using the app handle.
- `src/services/events.ts` subscribes to those events.
- `src/App.tsx` dispatches them into the correct Zustand stores.

This is how live progress appears without the frontend polling every query.

That event stream now carries explicit collaborative-round signaling in addition to normal step and token events, so the UI can distinguish:

- one worker on one obligation
- several workers on distinct obligations
- several workers collaborating on the same obligation

### Rust to Python Sidecar

- Rust uses `SidecarClient` in `src-tauri/src/api/sidecar.rs`.
- The main calls are validation, sidecar health, research requests, and scout briefing.

### Rust to LLM Providers

- Rust uses `LlmClient`.
- The provider is chosen from the active profile / loop config.
- The loop can build separate clients for solver, reviewer, adversary, critic, discerner, and decomposer roles.

### Sidecar to External Knowledge Sources

- The sidecar's research router fans out to provider-specific adapters.
- This keeps the Rust hot path simpler while still allowing broader research integrations.

## 9. Ports, Startup, and Current Integration Notes

### Current Ports in Code

- Vite dev server: `1420`
- Tauri control server: `9744`
- Python sidecar from `package.json`: `9743`
- MCP sidecar defaults: `9743`
- Rust `SidecarClient` constant: `9745`

### Important Current Mismatch

There is an active port mismatch in the repository:

- the dev script starts the sidecar on `9743`
- the MCP defaults point at `9743`
- the architecture docs in the repo mention `9743`
- the Rust HTTP client is currently hard-coded to `http://127.0.0.1:9745`

Unless something outside this repository remaps that port, the Rust backend will not reach the default sidecar started by `npm run sidecar:dev`. This should be reconciled before relying on local end-to-end runs.

### Development Boot Path

The intended local dev startup is:

1. Install frontend deps with `npm install`
2. Install sidecar deps with `pip install -e ".[dev]"` inside `sidecar/`
3. Start the sidecar
4. Start Tauri dev mode

Tauri itself starts Vite through `beforeDevCommand` in `src-tauri/tauri.conf.json`.

## 10. Practical Mental Model

If you need to reason about the project quickly, this is the most useful simplification:

- React is the operator console.
- Rust is the orchestrator, persistence owner, and runtime control plane.
- Python is the validator and research utility plane.
- SQLite is the memory and audit trail.

If you understand those four statements, the rest of the codebase mostly falls into place.
