# Red/Green Runbook

Operational Red/Green workflow for ChatDB. Use this with [RED_PHASE_PROMPT.md](/c:/Users/Vario/Desktop/chatdb/RED_PHASE_PROMPT.md) and [GREEN_PHASE_PROMPT.md](/c:/Users/Vario/Desktop/chatdb/GREEN_PHASE_PROMPT.md).

## Purpose

- make Red and Green runnable, not just conceptual
- keep test work phase-gated by target
- reduce noisy failures that are really setup problems

## Entry Points

- `npm run tdd:red -- <stack> <target>`
- `npm run tdd:green -- <stack> <target>`

The phase runner lives in [scripts/tdd-phase.mjs](/c:/Users/Vario/Desktop/chatdb/scripts/tdd-phase.mjs).

## Supported Stacks

- `frontend`
  - preflight: `npx tsc --noEmit`
  - target: `npm run test:run -- <target>`
- `rust`
  - target: `cargo test <target>`
- `python`
  - target: `python -m pytest <target>`

If `target` is omitted, the runner uses the full stack test command.

## Red Workflow

1. Choose the smallest missing behavior from [TEST_GAP_ANALYSIS.md](/c:/Users/Vario/Desktop/chatdb/TEST_GAP_ANALYSIS.md).
2. Add or update the failing test only.
3. Run the target in Red mode.
4. Stop once the runner confirms the target fails for the intended reason.

Examples:

```text
npm run tdd:red -- frontend src/components/loop/LoopControls.test.tsx
npm run tdd:red -- rust loop_engine::step
npm run tdd:red -- python sidecar/tests/test_main_app.py
```

Red rules:

- A preflight failure is not valid Red evidence.
- A passing target means you do not have Red yet.
- Do not edit production code before Red is established.

## Green Workflow

1. Re-read the failing test as the contract.
2. Implement the smallest production change that satisfies it.
3. Run the same target in Green mode.
4. Run any nearby safety-net checks appropriate to the module you touched.

Examples:

```text
npm run tdd:green -- frontend src/components/loop/LoopControls.test.tsx
npm run tdd:green -- rust loop_engine::step
npm run tdd:green -- python sidecar/tests/test_main_app.py
```

Green rules:

- Green is only complete when the former Red target passes.
- Do not turn Green into refactor work.
- Cleanup and deduplication belong in Blue.

## Recommended Starting Targets

Based on [TEST_GAP_ANALYSIS.md](/c:/Users/Vario/Desktop/chatdb/TEST_GAP_ANALYSIS.md), the next good Red/Green slices are:

- Frontend
  - `src/components/loop/LoopControls.test.tsx`
  - `src/stores/agentStore.test.ts`
  - `src/components/problem/ProblemInput.test.tsx`
- Rust
  - `src-tauri/src/verification/mod.rs`
  - `src-tauri/src/api/sidecar.rs`
  - `src-tauri/src/loop_engine/step.rs`
- Python
  - `sidecar/tests/test_validation_router.py`
  - `sidecar/tests/test_agents_router.py`
  - `sidecar/tests/test_research_router.py`

## Phase Handoff

- Use [RED_PHASE_PROMPT.md](/c:/Users/Vario/Desktop/chatdb/RED_PHASE_PROMPT.md) when the task is to create failing evidence.
- Use [GREEN_PHASE_PROMPT.md](/c:/Users/Vario/Desktop/chatdb/GREEN_PHASE_PROMPT.md) when the task is to make existing Red tests pass.
- Use [BLUE_REVIEW_PROMPT.md](/c:/Users/Vario/Desktop/chatdb/BLUE_REVIEW_PROMPT.md) only after Green is established.
