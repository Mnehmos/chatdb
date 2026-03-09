import { useLoopStore } from '../../stores/loopStore';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import type { AuditResult, Step, ObligationEvent, CriticCheckEvent } from '../../types';
import { ObligationGraph } from './ObligationGraph';

function renderFormal(expr: string): string {
  // Convert SymPy-style expression to LaTeX-ish for KaTeX
  let tex = expr;

  // 1. Handle parenthesized exponents BEFORE converting ** → ^
  //    2**(k * 2**k) → 2^{k \cdot 2^{k}}  (nested parens handled iteratively)
  //    Process innermost parens first by repeating until stable
  let prev = '';
  while (prev !== tex) {
    prev = tex;
    tex = tex.replace(/\*\*\(([^()]+)\)/g, (_m, inner) => `^{${inner}}`);
  }

  // 2. Remaining ** (simple exponents like x**2)
  tex = tex.replace(/\*\*/g, '^');

  // 3. Multiplication → \cdot
  tex = tex.replace(/\*/g, ' \\cdot ');

  // 4. Common functions
  tex = tex.replace(/sqrt\(([^)]+)\)/g, '\\sqrt{$1}');
  tex = tex.replace(/\bpi\b/g, '\\pi');

  // 5. Subscript digits: x1 → x_{1}, but NOT inside braces or after _
  tex = tex.replace(/([a-zA-Z])(\d+)(?![^{]*})/g, '$1_{$2}');

  // 6. Wrap any multi-char exponent that isn't already braced
  //    ^abc → ^{abc}, ^10 → ^{10}, but ^{...} left alone, ^2 left alone
  tex = tex.replace(/\^([^{](?:[^{}\s])+)/g, '^{$1}');

  try {
    return katex.renderToString(tex, { throwOnError: false, displayMode: false });
  } catch {
    return expr;
  }
}

function AuditCard({ audit }: { audit: AuditResult }) {
  const breadthPct = Math.round(audit.breadth * 100);
  const confidencePct = Math.round(audit.confidence * 100);
  return (
    <div className="step-node audit-node">
      <div className="step-header">
        <span className="step-number audit-badge">AUDIT @ #{audit.step_number}</span>
        <span className="audit-breadth" title="Exploration breadth">
          Breadth: {breadthPct}%
        </span>
        <span className="audit-confidence" title="Confidence in current path">
          Confidence: {confidencePct}%
        </span>
      </div>
      {audit.techniques_explored.length > 0 && (
        <div className="audit-techniques">
          <span className="audit-label">Explored:</span>{' '}
          {audit.techniques_explored.join(', ')}
        </div>
      )}
      {audit.techniques_missing.length > 0 && (
        <div className="audit-techniques audit-missing">
          <span className="audit-label">Missing:</span>{' '}
          {audit.techniques_missing.join(', ')}
        </div>
      )}
      <div className="audit-direction">
        <span className="audit-label">Recommended:</span> {audit.recommended_direction}
      </div>
      {audit.should_branch && (
        <div className="audit-branch-flag">BRANCH RECOMMENDED</div>
      )}
    </div>
  );
}

/** Group steps by attempt_id, preserving insertion order. */
function groupByAttempt(steps: Step[]): { attemptId: string; steps: Step[] }[] {
  const groups: { attemptId: string; steps: Step[] }[] = [];
  let current: { attemptId: string; steps: Step[] } | null = null;
  for (const s of steps) {
    if (!current || current.attemptId !== s.attempt_id) {
      current = { attemptId: s.attempt_id, steps: [] };
      groups.push(current);
    }
    current.steps.push(s);
  }
  return groups;
}

function StepCard({ s }: { s: Step }) {
  return (
    <div className={`step-node ${s.verified ? 'verified' : 'rejected'} ${s.proposal_type === 'conclusion' ? 'conclusion' : ''}`}>
      <div className="step-header">
        <span className="step-number">#{s.step_number}</span>
        {s.proposal_type === 'conclusion' && <span className="step-type-badge conclusion-badge">CONCLUSION</span>}
        <span className="step-model">{s.model}</span>
        <span className={`step-verdict ${s.verified ? 'pass' : 'fail'}`}>{s.verified ? '\u2713' : '\u2717'}</span>
      </div>
      <p className="step-natural">{s.proposal_natural}</p>
      {s.proposal_reasoning && <p className="step-reasoning">{s.proposal_reasoning}</p>}
      {s.proposal_formal && (
        <div
          className="step-formal"
          dangerouslySetInnerHTML={{ __html: renderFormal(s.proposal_formal) }}
        />
      )}
      {!s.verified && s.rejection_reason && <div className="step-rejection">{s.rejection_reason}</div>}
      <div className="step-validators">
        {s.sympy_passed !== undefined && s.sympy_passed !== null && <span className={s.sympy_passed ? 'v-pass' : 'v-fail'}>S</span>}
        {s.pint_passed !== undefined && s.pint_passed !== null && <span className={s.pint_passed ? 'v-pass' : 'v-fail'}>P</span>}
        {s.lean_passed !== undefined && s.lean_passed !== null && <span className={s.lean_passed ? 'v-pass' : 'v-fail'}>L</span>}
      </div>
      {s.challenge_model && (
        <div className={`step-challenge ${s.challenge_flaw_found && s.challenge_fatal ? 'challenge-fatal' : s.challenge_flaw_found ? 'challenge-flaw' : 'challenge-survived'}`}>
          <div className="challenge-header">
            <span className="challenge-icon">{s.challenge_flaw_found && s.challenge_fatal ? '\u2694' : s.challenge_flaw_found ? '\u26A0' : '\u{1F6E1}'}</span>
            <span className="challenge-model">{s.challenge_model}</span>
            {s.challenge_confidence != null && (
              <span className="challenge-confidence">{Math.round(s.challenge_confidence * 100)}%</span>
            )}
          </div>
          {s.challenge_attack && <p className="challenge-attack">{s.challenge_attack}</p>}
        </div>
      )}
    </div>
  );
}

function ObligationCard({ ob, criticChecks }: { ob: ObligationEvent; criticChecks: CriticCheckEvent[] }) {
  const check = criticChecks.find(c => c.obligation_id === ob.id);
  const statusClass = ob.status === 'open' ? 'obligation-open' : ob.status === 'closed_proved' ? 'obligation-proved' : 'obligation-refuted';
  const statusIcon = ob.status === 'open' ? '\u{1F512}' : ob.status === 'closed_proved' ? '\u2705' : '\u274C';
  const priorityLabel = ob.priority >= 0.8 ? 'HIGH' : ob.priority >= 0.5 ? 'MED' : 'LOW';

  return (
    <div className={`step-node obligation-node ${statusClass}`}>
      <div className="step-header">
        <span className="obligation-icon">{statusIcon}</span>
        <span className="obligation-badge">OBLIGATION [{priorityLabel}]</span>
        <span className={`obligation-status ${statusClass}`}>{ob.status.replace(/_/g, ' ')}</span>
      </div>
      <p className="obligation-description">{ob.description}</p>
      {ob.assigned_model && (
        <div className="obligation-assignment">
          <span className="obligation-model">{ob.assigned_model}</span>
          {ob.steps_spent != null && ob.max_steps != null && (
            <span className="obligation-progress"> ({ob.steps_spent}/{ob.max_steps} steps)</span>
          )}
        </div>
      )}
      {check && (
        <div className="critic-check">
          <span className="critic-label">Critic Check:</span> {check.check_description}
          {check.counterexample_hint && (
            <span className="critic-hint"> ({check.counterexample_hint})</span>
          )}
          {check.likely_wrong && <span className="critic-warning"> — likely wrong</span>}
        </div>
      )}
      {ob.closed_by_step && (
        <div className="obligation-closed-by">
          {ob.closure_note
            ? <>{ob.closure_note} <span className="obligation-step-ref">(step #{ob.closed_by_step})</span></>
            : <>Resolved by step #{ob.closed_by_step}</>
          }
        </div>
      )}
    </div>
  );
}

export function ProofTree() {
  const { steps, auditResults, reviewResult, reviewing, extractedPatterns, obligations, criticChecks, attemptId } = useLoopStore();
  if (!steps.length && !auditResults.length) return <div className="proof-tree empty"><p className="muted">No steps yet. Start solving.</p></div>;

  const attemptGroups = groupByAttempt(steps);
  const totalAttempts = attemptGroups.length;

  return (
    <div className="proof-tree">
      <h3>Proof Chain</h3>
      <div className="steps-list">
        {attemptGroups.map((group, gi) => {
          const verified = group.steps.filter(s => s.verified).length;
          const rejected = group.steps.filter(s => !s.verified).length;
          const hasConcl = group.steps.some(s => s.proposal_type === 'conclusion' && s.verified);

          // Merge audits that belong to this attempt's step range
          const minStep = group.steps[0]?.step_number ?? 0;
          const maxStep = group.steps[group.steps.length - 1]?.step_number ?? Infinity;
          const attemptAudits = auditResults.filter(a => a.step_number >= minStep && a.step_number <= maxStep);

          // Build merged timeline for this attempt
          type TimelineItem = { kind: 'step'; data: Step } | { kind: 'audit'; data: AuditResult };
          const timeline: TimelineItem[] = [];
          for (const s of group.steps) timeline.push({ kind: 'step', data: s });
          for (const a of attemptAudits) timeline.push({ kind: 'audit', data: a });
          timeline.sort((a, b) => {
            const numA = a.kind === 'step' ? a.data.step_number : a.data.step_number;
            const numB = b.kind === 'step' ? b.data.step_number : b.data.step_number;
            if (numA !== numB) return numA - numB;
            return a.kind === 'audit' ? 1 : -1;
          });

          return (
            <div key={group.attemptId} className="attempt-group">
              {/* Show attempt header when there are multiple attempts */}
              {totalAttempts > 1 && (
                <div className={`attempt-header ${hasConcl ? 'attempt-solved' : ''}`}>
                  <span className="attempt-label">Attempt {gi + 1}{totalAttempts > 1 ? ` / ${totalAttempts}` : ''}</span>
                  <span className="attempt-stats">
                    {verified} verified, {rejected} rejected
                  </span>
                  {hasConcl && <span className="attempt-badge solved">Proved</span>}
                </div>
              )}
              {timeline.map((item, i) => {
                if (item.kind === 'audit') {
                  return <AuditCard key={`audit-${item.data.step_number}`} audit={item.data} />;
                }
                return <StepCard key={item.data.id || `step-${gi}-${i}`} s={item.data} />;
              })}
            </div>
          );
        })}
      </div>

      {/* Obligation Graph + Card List */}
      {attemptId && <ObligationGraph attemptId={attemptId} />}
      {obligations.length > 0 && (
        <div className="obligations-section">
          <h4>Obligations ({obligations.filter(o => o.status === 'open').length} open)</h4>
          {obligations.map(ob => (
            <ObligationCard key={ob.id} ob={ob} criticChecks={criticChecks} />
          ))}
        </div>
      )}

      {/* Review indicator */}
      {reviewing && (
        <div className="review-indicator">
          <div className="review-spinner" />
          Running post-attempt review...
        </div>
      )}

      {/* Review findings summary */}
      {reviewResult && (
        <div className="review-summary">
          <h4>Post-Attempt Review</h4>
          <div className="review-stats">
            <span>Coverage: {Math.round(reviewResult.exploration_coverage * 100)}%</span>
            <span>Conclusion: {reviewResult.conclusion_sound ? 'Sound' : 'Unsound'} ({Math.round(reviewResult.conclusion_confidence * 100)}%)</span>
            <span className="review-label">{reviewResult.training_label.replace(/_/g, ' ')}</span>
          </div>
          {reviewResult.findings.length > 0 && (
            <div className="review-findings">
              {reviewResult.findings.map((f, i) => (
                <div key={i} className={`review-finding priority-${f.priority}`}>
                  <span className="finding-type">{f.type.replace(/_/g, ' ')}</span>
                  <span className="finding-summary">{f.summary}</span>
                  <p className="finding-detail">{f.detail}</p>
                </div>
              ))}
            </div>
          )}
          {reviewResult.missing_constructions.length > 0 && (
            <div className="review-missing">
              <span className="audit-label">Missing constructions:</span>{' '}
              {reviewResult.missing_constructions.join(', ')}
            </div>
          )}
        </div>
      )}

      {/* Extracted patterns */}
      {extractedPatterns.length > 0 && (
        <div className="patterns-extracted">
          <h4>Patterns Extracted</h4>
          {extractedPatterns.map((p, i) => (
            <div key={i} className="pattern-card">
              <div className="pattern-header">
                <span className="pattern-name">{p.name}</span>
                {p.technique_class && <span className="pattern-class">{p.technique_class}</span>}
              </div>
              <p className="pattern-description">{p.description}</p>
              <div className="pattern-details">
                <div className="pattern-trigger"><span className="audit-label">Trigger:</span> {p.trigger_text}</div>
                <div className="pattern-strategy"><span className="audit-label">Strategy:</span> {p.strategy}</div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
