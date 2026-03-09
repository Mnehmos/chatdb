# Code Review Techniques Research Catalog 
 
- Run ID: run-code-review-research-001 
- Task ID: research-code-review-001 
- Scope: docs-only research synthesis for engineering audit execution.

## TDD Enforcement Prompt Tag

```text
No implementation before failing tests (Red)
Minimal implementation to pass tests (Green)
Refactor only after green tests (Blue)
PRs without Red evidence are non-compliant
```

## 1) Exhaustive Taxonomy of Code Review Techniques

Each technique includes: purpose, when to use, signals, anti-signals, required artifacts, reviewer roles, expected findings, limitations, and KPIs.

### A. Manual and Lightweight Techniques

#### 1. Asynchronous Pull Request Review
- Purpose: baseline pre-merge quality gate.
- When to use: all non-trivial changes.
- Signals: cohesive diff, rationale, tests linked.
- Anti-signals: mixed-purpose large diff, weak evidence.
- Required artifacts: PR template, issue link, test and CI output.
- Reviewer roles: peer reviewer, optional code owner.
- Expected findings: logic defects, edge cases, readability gaps.
- Limitations: reviewer fatigue and reduced depth on large changes.
- KPIs: review lead time, comment resolution time, escaped defect rate.

#### 2. Buddy Review
- Purpose: rapid feedback and transfer.
- When to use: small fixes, onboarding, urgent patches.
- Signals: immediate clarification needed.
- Anti-signals: high-criticality changes that require strict traceability.
- Required artifacts: diff snapshot and minimal checklist note.
- Reviewer roles: author and buddy.
- Expected findings: obvious correctness or style defects.
- Limitations: weak independence and audit trail.
- KPIs: time to merge, post-merge hotfix count.

#### 3. Pair Programming as Continuous Review
- Purpose: prevent defects during implementation.
- When to use: complex or uncertain logic.
- Signals: many design choices and high coupling.
- Anti-signals: pure boilerplate changes.
- Required artifacts: commit evidence, tests, optional session notes.
- Reviewer roles: driver and navigator.
- Expected findings: design flaws and invalid assumptions earlier.
- Limitations: higher synchronous cost.
- KPIs: complex story defect density, knowledge spread index.

#### 4. Checklist-Based Review
- Purpose: reduce omission bias and normalize quality.
- When to use: recurring defect classes and audit-heavy teams.
- Signals: repeat incidents in same categories.
- Anti-signals: checklist bloat and box ticking.
- Required artifacts: versioned checklist with evidence links.
- Reviewer roles: peer plus optional specialist.
- Expected findings: policy violations and known recurrent defects.
- Limitations: weaker for novel defect patterns.
- KPIs: checklist adherence, repeat-defect recurrence.

### B. Formal and Structured Techniques

#### 5. Fagan Style Inspection
- Purpose: high-rigor defect detection with explicit roles and entry and exit criteria.
- When to use: safety, financial, legal, or mission-critical changes.
- Signals: severe consequences for escaped defects.
- Anti-signals: tiny low-risk edits where overhead dominates.
- Required artifacts: inspection packet, defect log, rework verification.
- Reviewer roles: moderator, author, reader, recorder, inspectors.
- Expected findings: high-severity logic or specification mismatches.
- Limitations: substantial coordination cost.
- KPIs: major defects per KLOC, defect removal efficiency.

#### 6. Scenario Walkthrough Review
- Purpose: validate behavior against end-to-end workflows.
- When to use: stateful or branch-heavy features.
- Signals: complex user journeys and business rules.
- Anti-signals: no behavior change refactors.
- Required artifacts: scenario matrix, acceptance criteria, test map.
- Reviewer roles: reviewer plus domain representative.
- Expected findings: missing paths and state transition defects.
- Limitations: quality depends on scenario completeness.
- KPIs: scenario coverage ratio, escaped workflow defect rate.

#### 7. Perspective-Based Reading
- Purpose: review from distinct lenses such as security, operations, QA, and maintainability.
- When to use: cross-cutting platform or shared component changes.
- Signals: multi-concern impact.
- Anti-signals: trivial isolated patch.
- Required artifacts: perspective prompts and review rubric.
- Reviewer roles: specialists by perspective.
- Expected findings: concern-specific latent defects.
- Limitations: higher reviewer coordination effort.
- KPIs: defect diversity captured per review cycle.

### C. Risk Based and Change-Aware Techniques

#### 8. Risk-Triage Review Routing
- Purpose: allocate review depth by risk score.
- When to use: all repos with variable criticality.
- Signals: auth, billing, schema, concurrency touched.
- Anti-signals: stale risk rubric.
- Required artifacts: risk score, blast-radius notes, rollback plan.
- Reviewer roles: peer, owner, risk approver.
- Expected findings: severity-weighted issues.
- Limitations: mis-scoring can under-review risk.
- KPIs: high-risk escaped defects, calibration error.

#### 9. Change Impact Analysis Review
- Purpose: assess compatibility and downstream blast radius.
- When to use: API, schema, protocol, contract changes.
- Signals: public interface edits and fan-out dependencies.
- Anti-signals: missing dependency map.
- Required artifacts: dependency graph, migration and compatibility notes.
- Reviewer roles: author, integrator, owner.
- Expected findings: breaking changes and migration omissions.
- Limitations: hidden consumers can be missed.
- KPIs: breakage incidents, rollback frequency.

#### 10. Two-Person Rule for Critical Changes
- Purpose: independent validation on critical paths.
- When to use: privilege boundaries, payments, cryptography, production controls.
- Signals: critical component touched.
- Anti-signals: cosmetic-only edits.
- Required artifacts: independent approvals and rationale records.
- Reviewer roles: two independent qualified reviewers.
- Expected findings: blind spots and unsafe assumptions.
- Limitations: throughput impact under staffing pressure.
- KPIs: critical defect escape rate, independence compliance.

### D. Security-Focused Techniques

#### 11. Threat-Model-Driven Review
- Purpose: validate implemented mitigations against explicit threats.
- When to use: new trust boundaries, exposed APIs, auth flows.
- Signals: new input vectors and secret handling paths.
- Anti-signals: high-risk change without threat model context.
- Required artifacts: threat model, trust-boundary diagram, abuse cases.
- Reviewer roles: security reviewer plus feature reviewer.
- Expected findings: authz gaps, boundary validation failures, insecure defaults.
- Limitations: coverage quality depends on threat model quality.
- KPIs: pre-production vulnerability discovery rate.

#### 12. Secure Coding Checklist Review (OWASP and ASVS aligned)
- Purpose: enforce known secure coding controls consistently.
- When to use: internet-facing and sensitive data paths.
- Signals: parsing, serialization, auth, session, crypto changes.
- Anti-signals: generic checklist not tailored to stack.
- Required artifacts: security checklist, SAST output, dependency scan.
- Reviewer roles: security champion and peer reviewer.
- Expected findings: recurring CWE classes, hardening omissions.
- Limitations: novel attack chains can bypass fixed checklists.
- KPIs: CWE recurrence trend, mean remediation time.

#### 13. Abuse-Case Review
- Purpose: review from attacker intent instead of happy path.
- When to use: fraud-sensitive, value-transfer, privilege-sensitive workflows.
- Signals: incentives for misuse and replay opportunities.
- Anti-signals: no abuse-case inventory.
- Required artifacts: misuse stories, negative tests, logging expectations.
- Reviewer roles: security, product risk, backend reviewer.
- Expected findings: logic abuse vectors, replay and idempotency defects.
- Limitations: requires domain-specific threat intelligence.
- KPIs: abuse incident trend, abuse-case coverage ratio.

### E. Architecture, Performance, and Reliability Techniques

#### 14. ADR Conformance Review
- Purpose: verify implementation conforms to approved architecture decisions.
- When to use: structural and cross-cutting changes.
- Signals: dependency direction changes and boundary edits.
- Anti-signals: significant structural change without ADR.
- Required artifacts: ADR link, module diagram, dependency diff.
- Reviewer roles: architect, code owner, peer reviewer.
- Expected findings: architecture drift and layer violations.
- Limitations: effectiveness drops if ADR governance is stale.
- KPIs: architecture drift findings per quarter.

#### 15. Interface and Contract Review
- Purpose: ensure API and schema stability with explicit versioning.
- When to use: endpoint, event, schema, SDK changes.
- Signals: contract edits and consumer impact risk.
- Anti-signals: absent compatibility evidence.
- Required artifacts: contract diff, compatibility tests, deprecation plan.
- Reviewer roles: API owner and consumer representative.
- Expected findings: breaking signatures and semantic regressions.
- Limitations: unknown consumers may be missed.
- KPIs: contract break incidents, backward-compatibility pass rate.

#### 16. Performance Review
- Purpose: detect algorithmic and resource regressions pre-merge.
- When to use: hot paths and latency-sensitive changes.
- Signals: complexity increases, query/loop growth, allocation spikes.
- Anti-signals: no baseline benchmark.
- Required artifacts: benchmark and profile snapshots, performance budget.
- Reviewer roles: performance-aware reviewer and service owner.
- Expected findings: N+1 queries, complexity inflation, memory pressure.
- Limitations: synthetic benchmarks can diverge from production patterns.
- KPIs: P95 and P99 deltas, regression gate pass rate.

#### 17. Concurrency Correctness Review
- Purpose: identify race conditions, deadlocks, and retry hazards.
- When to use: shared state, async orchestration, lock logic changes.
- Signals: synchronization primitive changes and cancellation paths.
- Anti-signals: no stress testing evidence.
- Required artifacts: concurrency model note, stress test outputs.
- Reviewer roles: senior concurrency reviewer.
- Expected findings: lock-order inversions, race conditions, non-idempotent retries.
- Limitations: schedule-dependent bugs are inherently hard to prove absent.
- KPIs: concurrency incident trend, flaky test rate.

#### 18. Operability and SRE Review
- Purpose: validate observability, alertability, and rollback safety.
- When to use: production-impacting services and deployment changes.
- Signals: changed SLO paths, new failure modes.
- Anti-signals: missing runbooks and alert specs.
- Required artifacts: metrics/logs/traces plan, alerts, runbook, rollback plan.
- Reviewer roles: SRE reviewer and service owner.
- Expected findings: telemetry blind spots and weak failure handling.
- Limitations: requires mature observability platform.
- KPIs: MTTR trend, alert precision and recall, rollback success rate.

### F. Testability, Maintainability, and Governance Techniques

#### 19. Testability-First Review
- Purpose: ensure changed behavior is verifiable with strong evidence.
- When to use: any behavior change, mandatory for critical logic.
- Signals: branch additions, side effects, integration boundaries.
- Anti-signals: assertions absent for changed behavior.
- Required artifacts: test plan, coverage diff, failure-mode tests.
- Reviewer roles: peer reviewer with QA perspective.
- Expected findings: untested branches, brittle tests, weak assertions.
- Limitations: raw coverage can be gamed.
- KPIs: mutation score trend, flaky rate, escaped defect correlation.

#### 20. Maintainability and Readability Review
- Purpose: reduce long-term ownership cost and cognitive load.
- When to use: all changes, especially shared modules.
- Signals: deep nesting, long methods, ambiguous naming.
- Anti-signals: repeated waivers without debt tracking.
- Required artifacts: complexity metrics and naming rationale where needed.
- Reviewer roles: peer reviewer and code owner.
- Expected findings: duplication, excessive complexity, weak modularity.
- Limitations: partially subjective judgments.
- KPIs: complexity trend, churn-hotspot stabilization.

#### 21. Refactoring Safety Review
- Purpose: verify behavior-preserving intent and safety-net adequacy.
- When to use: internal reorganization and framework upgrades.
- Signals: structural changes with no intended behavior delta.
- Anti-signals: mixed refactor and feature work in one diff.
- Required artifacts: before-and-after behavioral assertions and regression results.
- Reviewer roles: module owner and refactor reviewer.
- Expected findings: hidden semantic drift and side effects.
- Limitations: very large diffs reduce reviewer accuracy.
- KPIs: post-refactor defect rate, rollback incidence.

#### 22. Data Integrity and Migration Review
- Purpose: protect persistent data correctness and recoverability.
- When to use: schema changes, backfills, lifecycle policy changes.
- Signals: irreversible operations and large table rewrites.
- Anti-signals: no rollback or restore strategy.
- Required artifacts: migration plan, rollback script, dry-run evidence.
- Reviewer roles: DB owner, backend reviewer, operations reviewer.
- Expected findings: data loss risks, lock contention, non-idempotent steps.
- Limitations: test data may not reflect production scale.
- KPIs: migration failure rate, restore drill success.

#### 23. Privacy and Compliance Review
- Purpose: enforce policy and legal requirements in code paths.
- When to use: PII handling, retention/deletion, consent and logging flows.
- Signals: new personal-data fields and transfer paths.
- Anti-signals: missing data inventory linkage.
- Required artifacts: data classification map, retention mapping, audit-log requirements.
- Reviewer roles: compliance or privacy reviewer plus engineer.
- Expected findings: over-collection, retention violations, consent bypass.
- Limitations: legal interpretation may require counsel input.
- KPIs: compliance finding count, remediation SLA attainment.

#### 24. CODEOWNERS-Gated Review
- Purpose: ensure domain expertise in approval chain.
- When to use: shared repos and critical subsystems.
- Signals: owned files changed.
- Anti-signals: stale ownership map.
- Required artifacts: ownership rules and escalation path.
- Reviewer roles: required owner reviewer plus peer.
- Expected findings: domain-policy and nuanced correctness defects.
- Limitations: bottlenecks if owner capacity is constrained.
- KPIs: owner-review compliance and queue time by owner group.

#### 25. Reviewer Calibration and Post-Merge Audit Sampling
- Purpose: maintain review consistency and detect process drift.
- When to use: monthly or quarterly governance cadence.
- Signals: high reviewer variance or rising incidents.
- Anti-signals: non-representative sampling frames.
- Required artifacts: sample set, rubric, audit checklist, findings log.
- Reviewer roles: quality lead, senior reviewers, independent auditor.
- Expected findings: rubric ambiguity, process nonconformance, drift.
- Limitations: post-merge detection is reactive.
- KPIs: inter-reviewer agreement, audit pass rate, recurrence of nonconformance.

## 2) Comparative Matrix (Technique vs Cost, Coverage, Defect Classes, Suitability)

| Technique | Cost | Coverage | Defect Classes Caught Best | Change Size Suitability | Risk Suitability |
|---|---|---|---|---|---|
| Async PR Review | Low-Med | Broad baseline | Logic defects, edge-case misses, readability gaps | S/M/L | Low-Med |
| Buddy Review | Low | Narrow-Moderate | Obvious correctness/style defects | S | Low-Med |
| Pair Programming | Med-High | Moderate | Early design flaws and assumption errors | S/M | Med-High |
| Checklist-Based Review | Med | Targeted broad | Recurrent policy and known defect classes | S/M/L | Med-High |
| Fagan Inspection | High | Deep broad | High-severity logic/spec mismatches | M/L | High-Critical |
| Scenario Walkthrough | Med | Behavioral deep | Workflow/state-transition defects | M/L | Med-High |
| Perspective-Based Reading | Med-High | Cross-domain broad | Security/ops/testability cross-cutting issues | M/L | High |
| Risk-Triage Routing | Low-Med | Allocation method | Severity-weighted defects by depth assignment | S/M/L | All |
| Impact Analysis Review | Med | Interface/dependency focused | Breakage and migration-omission defects | S/M/L | Med-High |
| Two-Person Rule | Med-High | Critical-path focused | Blind spots in high-impact code | S/M/L | High-Critical |
| Threat-Model Review | Med-High | Security deep | Authz and trust-boundary flaws | M/L | High-Critical |
| Secure Checklist Review | Med | Security focused | Recurring CWE patterns | S/M/L | Med-High |
| Abuse-Case Review | Med-High | Security logic deep | Replay, fraud, business-logic abuse | M/L | High-Critical |
| ADR Conformance Review | Med | Architecture focused | Layer violations and drift | M/L | Med-High |
| Interface/Contract Review | Med | API/schema focused | Breaking contract changes | S/M/L | Med-High |
| Performance Review | Med-High | Performance focused | N+1, complexity, resource regressions | M/L | Med-High |
| Concurrency Review | High | Correctness deep | Races, deadlocks, idempotency failures | M/L | High-Critical |
| Operability/SRE Review | Med | Reliability focused | Telemetry gaps, rollback weakness, alert quality defects | M/L | High |
| Testability-First Review | Med | Quality broad | Untested branches and brittle assertions | S/M/L | All |
| Maintainability Review | Low-Med | Long-term quality | Complexity and modularity debt | S/M/L | All |
| Refactoring Safety Review | Med | Behavior-preservation focused | Hidden semantic drift | M/L | Med-High |
| Data Migration Review | High | Data integrity deep | Data loss and lock hazards | M/L | High-Critical |
| Privacy/Compliance Review | Med-High | Legal/policy focused | Consent, retention, classification violations | M/L | High-Critical |
| CODEOWNERS-Gated Review | Low-Med | Domain focused | Domain-specific correctness/policy defects | S/M/L | Med-High |
| Calibration plus Audit Sampling | Med | Governance broad | Process drift and control failures | Portfolio | All |

## 3) Concrete Operational Heuristics for Audits

### Review Depth Routing Heuristic
- Low risk plus small change: Async PR plus concise checklist.
- Medium risk or medium change: add scenario walkthrough and testability pass.
- High risk: add specialist lane (security, architecture, performance, data, reliability).
- Critical risk: enforce two-person rule plus formal inspection elements and rollback rehearsal.

### Change Shape Heuristic
- Keep reviewable units under roughly 200 to 400 changed lines when feasible.
- Split mixed-purpose changes (feature plus refactor plus rename).
- Require design summary when boundaries or dependencies change.

### Trigger-Based Specialist Invocation
- Security trigger: auth/authz, external inputs, secret handling, trust-boundary changes.
- Architecture trigger: new dependencies, boundary shifts, shared contracts.
- Performance trigger: hot-path logic, query fan-out, allocation spikes.
- Data trigger: schema migration, retention/deletion logic, backfills.
- Reliability trigger: retries, failover, deployment behavior, SLO-sensitive paths.

### Reviewer Assignment Heuristic
- Include at least one domain-aware reviewer for each change.
- Add a rotating outsider reviewer to expose hidden assumptions.
- Monitor reviewer load to avoid queue concentration and fatigue.

## 4) Review Prompts (Checklist-Ready)

### Universal Prompts
1. What behavior changed and what evidence proves it?
2. Which assumptions are implicit and where are they validated?
3. Which failure modes were tested, including rollback behavior?
4. What downstream contracts or consumers could break?
5. What observability confirms safe operation after deployment?

### Security Prompts
1. Where does trust boundary crossing occur and how is validation enforced?
2. Are authorization checks explicit, centralized, and test-covered?
3. Could this enable privilege escalation, data exposure, or replay abuse?
4. Are secrets and sensitive payloads prevented from logs and traces?

### Architecture Prompts
1. Does implementation align with approved ADRs and dependency direction?
2. Did coupling increase, and if yes, is justification documented?
3. Are module boundaries clearer or weaker after this change?

### Performance Prompts
1. What complexity changes occur on critical paths?
2. What benchmark or profile evidence supports no regression?
3. Does this introduce N+1 behavior or unbounded resource growth?

### Testability and Maintainability Prompts
1. Are changed branches covered with meaningful assertions?
2. Can a new maintainer explain intent quickly from code and docs?
3. Did complexity increase, and if so, is the tradeoff justified?

## 5) KPI Framework for Engineering Audits

### Outcome KPIs
- Escaped defects per 1000 changed lines.
- Severity-weighted escaped defect index.
- Critical incident attribution to review misses.

### Process KPIs
- Median review lead time by risk tier.
- Review depth adherence by risk tier.
- Reviewer load distribution and queue aging.
- Reopen rate after approval.

### Quality-of-Review KPIs
- Findings per review hour.
- Finding-class coverage (security, performance, architecture, testability, maintainability).
- Inter-reviewer agreement from calibration sessions.

### Governance KPIs
- CODEOWNERS compliance.
- Audit sampling pass rate.
- Repeat nonconformance recurrence rate.

## 6) Limitations, Gaps, and Confidence
- No universal KPI thresholds fit every repository; calibrate by criticality and defect cost.
- Escaped-defect attribution is noisy; evaluate trends over windows, not single points.
- Checklist value decays if not refreshed from incidents and postmortems.
- Confidence: High for taxonomy and practice patterns; Medium for exact threshold recommendations requiring local data.

## 7) Suggested Reference Baseline for Audit Programs
- Fagan inspection literature (formal software inspection methods).
- IEEE and ISO software quality and review standards.
- OWASP Code Review Guide and ASVS control mappings.
- DORA and Accelerate metrics for software delivery quality.
- SRE reliability and operability review practices.

## 8) Downstream Checklist Design Guidance
- Maintain one core checklist plus specialist overlays.
- Require objective evidence links for every checklist item.
- Tie mandatory reviewer roles to risk rubric and ownership mapping.
- Define explicit exit criteria for each risk tier.
- Recalibrate quarterly using audit sampling and reviewer calibration.
