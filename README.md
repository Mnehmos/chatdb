# ChatDB -- Verified Reasoning Engine

Autonomous proof engine generating seven layers of verified training data.

## Quick Start

```bash
npm install
cd sidecar && pip install -e ".[dev]" && cd ..
npm run sidecar:dev   # Terminal 1
npm run tauri:dev     # Terminal 2
```

## Repository Workflow

This repository follows Gitflow plus mandatory TDD phase gates for every code change.

- **Long-lived branches**
  - `main`: production-ready, release-tagged history
  - `develop`: integration branch for completed feature work
- **Working branches**
  - `feature/*`: branch from `develop`, merge back to `develop` via PR
  - `release/*`: branch from `develop`, merge to `main` and back-merge to `develop`
  - `hotfix/*`: branch from `main`, merge to `main` and `develop`
- **Required lifecycle for all code changes**
  - Red → Green → Blue is mandatory
  - No direct implementation bypassing Red evidence

### Basic Local Flow (feature branch from develop)

```bash
git checkout develop
git pull origin develop
git checkout -b feature/<task-id>-<short-kebab-description>
git push -u origin feature/<task-id>-<short-kebab-description>
gh pr create --base develop --head feature/<task-id>-<short-kebab-description> --fill
```

### TDD Enforcement Prompt Tag

```text
No implementation before failing tests (Red)
Minimal implementation to pass tests (Green)
Refactor only after green tests (Blue)
PRs without Red evidence are non-compliant
```

## Architecture

- **Rust Backend** (Tauri 2): Loop engine, orchestrator, critic, worker pool, SQLite
- **Python Sidecar** (FastAPI): SymPy, Pint, Lean validators + Council, Scout, Librarian agents
- **React Frontend**: Zustand stores, proof tree, agent dashboard, training data stats

## Training Data Layers

| # | Source | Signal |
|---|--------|--------|
| 1 | Solvers | Step-level PRM |
| 2 | Solvers | Full trajectories |
| 3 | Orchestrator | Routing decisions |
| 4 | Council | Metacognitive deliberation |
| 5 | Scout | Research query-result-outcome |
| 6 | Critic | Prediction vs truth |
| 7 | Librarian | Curation traces |

License: Proprietary - Vario Automation
