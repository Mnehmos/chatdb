import { useState } from 'react';
import type { Step } from '../../types';

interface Props {
  steps: Step[];
  onStepClick: (stepId: string) => void;
}

export function ProofChain({ steps, onStepClick }: Props) {
  const sorted = [...steps].sort((a, b) => a.step_number - b.step_number);
  const [expandedSteps, setExpandedSteps] = useState<Set<string>>(new Set());
  const [showAll, setShowAll] = useState(false);

  const toggle = (id: string) => {
    setExpandedSteps(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const verifiedCount = steps.filter(s => s.verified).length;
  const rejectedCount = steps.length - verifiedCount;

  // Show last 20 by default, all if toggled
  const display = showAll ? sorted : sorted.slice(-20);

  return (
    <div className="proof-chain">
      <div className="proof-chain-header">
        <h3>Proof Chain ({steps.length} steps)</h3>
        <span className="chain-stats">
          <span className="verified-count">{verifiedCount}V</span>
          {' / '}
          <span className="rejected-count">{rejectedCount}R</span>
        </span>
        {sorted.length > 20 && (
          <button className="btn btn-sm" onClick={() => setShowAll(!showAll)}>
            {showAll ? `Last 20` : `All ${sorted.length}`}
          </button>
        )}
      </div>
      {sorted.length === 0 && <div className="chain-empty">No steps yet</div>}
      {display.map(step => {
        const expanded = expandedSteps.has(step.id);
        return (
          <div
            key={step.id}
            className={`step-card ${step.stale_sibling ? 'step-stale' : step.verified ? 'step-verified' : 'step-rejected'} ${expanded ? 'step-expanded' : ''}`}
          >
            <div className="step-header" onClick={() => toggle(step.id)}>
              <span className="step-number">#{step.step_number}</span>
              <span className={`step-verdict ${step.stale_sibling ? 'verdict-stale' : step.verified ? 'verdict-pass' : 'verdict-fail'}`}>
                {step.stale_sibling ? '\u23F8' : step.verified ? '\u2713' : '\u2717'}
              </span>
              <span className="step-type">{step.proposal_type}</span>
              {step.solver_worker_id && (
                <span className="worker-badge" title={`Worker: ${step.solver_worker_id}, Mode: ${step.solver_dispatch_mode}`}>
                  {step.solver_worker_id}
                </span>
              )}
              {step.obligation_type && (
                <span className={`ob-badge ob-badge-${step.obligation_type.toLowerCase()}`} title={step.obligation_desc}>
                  {step.obligation_type}
                </span>
              )}
              <div className="step-validators">
                {step.sympy_passed != null && (
                  <span className={`validator-badge ${step.sympy_passed ? 'v-pass' : 'v-fail'}`}>
                    Sy{step.sympy_passed ? '\u2713' : '\u2717'}
                  </span>
                )}
                {step.pint_passed != null && (
                  <span className={`validator-badge ${step.pint_passed ? 'v-pass' : 'v-fail'}`}>
                    Pi{step.pint_passed ? '\u2713' : '\u2717'}
                  </span>
                )}
                {step.lean_passed != null && (
                  <span className={`validator-badge ${step.lean_passed ? 'v-pass' : 'v-fail'}`}>
                    Ln{step.lean_passed ? '\u2713' : '\u2717'}
                  </span>
                )}
              </div>
              {step.challenge_confidence != null && (
                <span className={`challenge-badge ${step.challenge_fatal ? 'ch-fatal' : 'ch-ok'}`}>
                  {(step.challenge_confidence * 100).toFixed(0)}%
                </span>
              )}
              <span className="step-expand-icon">{expanded ? '\u25BC' : '\u25B6'}</span>
            </div>

            {/* Collapsed: one-line summary */}
            {!expanded && (
              <div className="step-natural step-summary">{step.proposal_natural}</div>
            )}

            {/* Expanded: full detail */}
            {expanded && (
              <div className="step-detail">
                <div className="step-detail-section">
                  <label>Proposal</label>
                  <div className="step-natural">{step.proposal_natural}</div>
                </div>

                {step.proposal_formal && (
                  <div className="step-detail-section">
                    <label>Formal</label>
                    <code className="step-formal-block">{step.proposal_formal}</code>
                  </div>
                )}

                {step.proposal_reasoning && (
                  <div className="step-detail-section">
                    <label>Reasoning</label>
                    <div className="step-reasoning">{step.proposal_reasoning}</div>
                  </div>
                )}

                {!step.verified && step.rejection_reason && (
                  <div className="step-detail-section">
                    <label>Rejection</label>
                    <div className="step-rejection">{step.rejection_reason}</div>
                  </div>
                )}

                {step.challenge_attack && (
                  <div className="step-detail-section">
                    <label>Adversarial Challenge</label>
                    <div className="step-challenge">
                      {step.challenge_attack}
                      {step.challenge_model && (
                        <span className="challenge-model"> — {step.challenge_model}</span>
                      )}
                    </div>
                  </div>
                )}

                <div className="step-detail-meta">
                  <span>Model: {step.model}</span>
                  <span>{new Date(step.created_at).toLocaleTimeString()}</span>
                  <button className="btn btn-sm" onClick={() => onStepClick(step.id)}>
                    Full Detail
                  </button>
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
