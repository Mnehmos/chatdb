# ChatDB TDD + Gitflow Execution Plan

## Plan Metadata

- Run ID: run-tdd-gitflow-redesign-001
- Task ID: plan-redesign-001
- Scope Boundary: planning and specification artifacts in docs only
- In-Scope Path: docs/
- Out-of-Scope: source code, build config, runtime configuration, infra changes

## TDD Enforcement Prompt Tag

```text
No implementation before failing tests (Red)
Minimal implementation to pass tests (Green)
Refactor only after green tests (Blue)
PRs without Red evidence are non-compliant
```

## 1) Enforced TDD Lifecycle Contract (Red → Green → Blue)

### 1.1 Global Contract

All implementation work must run through explicit phase-gated TDD cycles. No direct implementation without an approved failing-test baseline.

### 1.2 Red Phase Contract

#### Entry Criteria

- Requirement is traceable to an approved task or acceptance criterion.
- Test target is identified (unit, integration, contract, or end-to-end).
- No implementation edits have started for the target behavior.

#### Required Actions

- Write or update tests that express the intended behavior.
- Run tests and confirm failure is due to missing behavior, not broken setup.
- Record failing evidence (command + failing assertion summary) in PR notes.

#### Exit Criteria

- At least one deterministic test fails for the intended reason.
- Failure message clearly describes the behavioral gap.
- Red evidence is captured and attached to the change record.

### 1.3 Green Phase Contract

#### Entry Criteria

- Red phase evidence exists and is valid.
- Scope of implementation is limited to passing failing tests.

#### Required Actions

- Implement the minimal change needed to satisfy failing tests.
- Re-run targeted test suite until all Red tests pass.
- Avoid broad refactors and non-essential design changes in this phase.

#### Exit Criteria

- All tests created in Red now pass.
- No new failing tests are introduced in the affected test scope.
- Change remains minimal and directly tied to test expectations.

### 1.4 Blue Phase Contract

#### Entry Criteria

- Green phase is complete and tests are passing.
- Refactor scope is justified (readability, maintainability, duplication reduction).

#### Required Actions

- Refactor production and/or test code without behavior changes.
- Run full required test gates after each significant refactor step.
- Keep commits traceable to non-functional improvements.

#### Exit Criteria

- Required full checks pass.
- No behavior drift relative to Green outcomes.
- Complexity and maintainability are improved and documented in PR summary.

### 1.5 TDD Anti-Bypass Rules

- No merge if Red evidence is missing.
- No merge if Green evidence does not show transition from failing to passing tests.
- No merge if Blue introduces behavior changes without new Red tests.
- Emergency work (hotfix) may compress cycle but must still provide post-fix Red/Green evidence.

## 2) Gitflow Branch Model for This Repository

### 2.1 Protected Long-Lived Branches

- main: production-ready, release-tagged history only.
- develop: integration branch for completed features.

### 2.2 Short-Lived Working Branches

- feature/*: new capabilities, enhancements, non-urgent fixes.
- release/*: stabilization branch for release hardening and version prep.
- hotfix/*: urgent production fixes branched from main, merged back to both main and develop.

### 2.3 Branch Origin and Merge Rules

- feature/* branches from develop and merges into develop via PR.
- release/* branches from develop and merges into main (release) and develop (back-merge).
- hotfix/* branches from main and merges into main and develop.

### 2.4 Naming Standard

- feature/<task-id>-<short-kebab-description>
- release/<version>
- hotfix/<incident-or-task-id>-<short-kebab-description>

## 3) Pull Request Gates and Mandatory Checks

### 3.1 Required PR Template Sections

- Objective and linked task(s)
- TDD evidence block (Red failure, Green pass, Blue refactor scope)
- Risk impact and rollback notes
- Validation summary by layer (frontend, backend, sidecar, integration as applicable)

### 3.2 Mandatory Automated Checks

- Lint/format checks for touched stacks
- Unit/integration test suites for affected components
- Build verification for impacted applications
- Security/dependency scan baseline (if configured)

### 3.3 Mandatory Human Checks

- At least one reviewer approval for feature/* PRs
- Two approvals for release/* and hotfix/* PRs
- Reviewer must confirm Red→Green→Blue evidence is coherent
- Reviewer must reject PRs with scope drift relative to task objective

### 3.4 Merge Policies

- Squash merge for feature/* unless preserving granular history is required
- Merge commit for release/* and hotfix/* to preserve branch lineage
- Block force-push on protected branches

## 4) Commit Conventions and Branch Protection Recommendations

### 4.1 Commit Message Convention

Use conventional commit prefixes with optional scope and mandatory task traceability:

- feat(scope): summary [task-id]
- fix(scope): summary [task-id]
- test(scope): summary [task-id]
- refactor(scope): summary [task-id]
- docs(scope): summary [task-id]
- chore(scope): summary [task-id]

### 4.2 Commit Hygiene Rules

- One logical intent per commit.
- Red, Green, and Blue transitions should be observable in commit sequence or PR timeline.
- Avoid mixed functional and refactor changes in one commit unless unavoidable.

### 4.3 Branch Protection Recommendations

Apply to main and develop:

- Require pull request before merge
- Require passing required status checks
- Require up-to-date branch before merge
- Require signed commits where organizational policy permits
- Restrict direct pushes to admins/release managers only
- Require conversation resolution before merge

## 5) Prompt Templates for Enforcement Blocks

### 5.1 User-Facing Enforcement Block Template

Purpose: inserted into task intake prompts to enforce process contracts before work begins.

Template:

"Execution Contract:
This task must follow enforced TDD and Gitflow.
1) Perform Red phase first: create failing tests with explicit failure evidence.
2) Perform Green phase second: implement minimum code to pass Red tests.
3) Perform Blue phase third: refactor without behavior change and rerun required checks.
4) Use Gitflow branches only: feature/*, release/*, hotfix/* according to task type.
5) Open PR with required gates: TDD evidence, checks passing, approvals complete.
Any bypass requires explicit exception approval recorded in the task log."

### 5.2 Agent-Facing Enforcement Block Template

Purpose: inserted into autonomous or delegated agent instructions to make process non-optional.

Template:

"Agent Execution Guardrails:
- Operate in strict Red → Green → Blue order.
- Do not implement behavior in Red.
- Do not refactor in Green beyond minimal pass conditions.
- Do not merge or mark complete without proof of required checks.
- Constrain branch strategy to Gitflow model:
  - feature/* from develop
  - release/* from develop
  - hotfix/* from main
- Reject tasks lacking branch type, acceptance criteria, or required test scope.
- Return structured completion payload with: status, files changed, checks run, evidence summary."

## 6) Migration Sequence: Non-Git State to Gitflow-Ready State

### Phase A — Baseline Capture and Safety

1. Snapshot current project directory state (including ignored/untracked audit note).
2. Create initial repository history anchor commit from current baseline.
3. Add contribution governance docs (this plan + PR template + branch policy notes).

### Phase B — Initialize Core Branch Topology

1. Establish main as protected release branch.
2. Create develop from main.
3. Configure default integration target for routine work as develop.

### Phase C — Policy and Gate Activation

1. Enable branch protections on main and develop.
2. Require status checks and review counts by branch type.
3. Publish commit convention and PR evidence requirements to contributors.

### Phase D — TDD Workflow Activation

1. Update team execution prompts with enforcement blocks.
2. Pilot first feature/* branch using strict Red→Green→Blue evidence.
3. Validate reviewer behavior against gate checklist.

### Phase E — Release/Hotfix Path Validation

1. Simulate release/* flow from develop to main and back-merge to develop.
2. Simulate hotfix/* flow from main and dual merge back to main/develop.
3. Confirm no direct commits were required on protected branches.

### Phase F — Operationalization

1. Make this process the default for all new tasks.
2. Add periodic audit of PR compliance and branch hygiene.
3. Record exceptions and corrective actions in project governance notes.

## 7) Rollout Acceptance Checklist

- [ ] Repository is initialized and baseline anchor commit exists.
- [ ] main and develop branches exist and are protected.
- [ ] feature/*, release/*, and hotfix/* naming and routing are documented.
- [ ] PR template requires Red/Green/Blue evidence.
- [ ] Required status checks are enforced on protected branches.
- [ ] Minimum reviewer approvals are enforced by branch type.
- [ ] Commit convention is documented and communicated.
- [ ] First pilot feature branch completed with full TDD evidence.
- [ ] Release simulation completed and back-merge validated.
- [ ] Hotfix simulation completed with dual-merge validation.
- [ ] No policy-critical bypass remains unresolved.

## 8) Task Map for Execution Ownership

| task_id | objective | mode | dependencies | acceptance_criteria | parallelizable |
|---|---|---|---|---|---|
| tdd-gf-01 | Define enforceable TDD phase contract and anti-bypass rules | planner | [] | Red/Green/Blue entry-exit gates documented and auditable | false |
| tdd-gf-02 | Define Gitflow branch model and naming standards | planner | [tdd-gf-01] | main/develop/feature/release/hotfix rules documented | false |
| tdd-gf-03 | Define PR gates, commit conventions, and branch protection policy | planner | [tdd-gf-02] | Merge checks and review requirements explicitly listed | false |
| tdd-gf-04 | Provide user-facing and agent-facing enforcement prompt templates | planner | [tdd-gf-03] | Templates ready for direct insertion into task prompts | true |
| tdd-gf-05 | Define migration sequence from non-git to Gitflow-ready state | planner | [tdd-gf-03] | Ordered migration phases with validation points documented | false |
| tdd-gf-06 | Define rollout acceptance checklist | planner | [tdd-gf-04, tdd-gf-05] | Checklist covers topology, policy, TDD evidence, and simulations | false |
