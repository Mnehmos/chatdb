import { useEffect, useState } from 'react';
import { useLoopStore } from '../../stores/loopStore';
import { useNavigationStore } from '../../stores/navigationStore';
import { getClaimsForStep, getDagEdgesFrom, getDagEdgesTo, getToolRunsForStep } from '../../services/tauri';
import type { Claim, DagEdge, ToolRun } from '../../types';

interface Props {
  problemId: string;
  attemptId: string;
  stepId: string;
}

function toolStatusClass(status: string): string {
  if (status === 'completed') return 'tool-ok';
  if (status === 'failed') return 'tool-fail';
  return 'tool-pending';
}

function toolIcon(name: string): string {
  if (name.includes('claim')) return '\u2714';
  if (name.includes('sympy')) return '\u2261';
  if (name.includes('wolfram')) return '\u03B1';
  if (name.includes('arxiv') || name.includes('scholar')) return '\uD83D\uDCDA';
  if (name.includes('oeis')) return '#';
  return '\uD83D\uDD27';
}

export function StepDetailView({ stepId }: Props) {
  const { steps } = useLoopStore();
  const { goBack } = useNavigationStore();
  const step = steps.find(s => s.id === stepId);
  const [claims, setClaims] = useState<Claim[]>([]);
  const [edges, setEdges] = useState<DagEdge[]>([]);
  const [toolRuns, setToolRuns] = useState<ToolRun[]>([]);
  const [expandedRuns, setExpandedRuns] = useState<Set<string>>(new Set());

  useEffect(() => {
    getClaimsForStep(stepId).then(setClaims).catch(() => setClaims([]));
    Promise.all([getDagEdgesFrom(stepId), getDagEdgesTo(stepId)])
      .then(([from, to]) => setEdges([...from, ...to]))
      .catch(() => setEdges([]));
    getToolRunsForStep(stepId).then(setToolRuns).catch(() => setToolRuns([]));
  }, [stepId]);

  const toggleRun = (id: string) => {
    setExpandedRuns(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  if (!step) {
    return (
      <div className="step-detail-empty">
        <p>Step not found</p>
        <button className="btn" onClick={goBack}>Back</button>
      </div>
    );
  }

  return (
    <div className="step-detail-view">
      <div className="step-detail-header">
        <h2>Step #{step.step_number}</h2>
        <span className={`step-verdict ${step.verified ? 'verdict-pass' : 'verdict-fail'}`}>
          {step.verified ? 'Verified' : 'Rejected'}
        </span>
        <span className="step-type-badge">{step.proposal_type}</span>
      </div>

      <div className="step-detail-meta">
        <span>Model: {step.model}</span>
        <span>Created: {new Date(step.created_at).toLocaleString()}</span>
      </div>

      {/* Natural Language */}
      <div className="step-detail-section">
        <h3>Natural Language</h3>
        <div className="step-detail-content">{step.proposal_natural}</div>
      </div>

      {/* Formal Expression */}
      {step.proposal_formal && (
        <div className="step-detail-section">
          <h3>Formal Expression</h3>
          <div className="step-detail-formal">{step.proposal_formal}</div>
        </div>
      )}

      {/* Reasoning */}
      {step.proposal_reasoning && (
        <div className="step-detail-section">
          <h3>Reasoning</h3>
          <div className="step-detail-content">{step.proposal_reasoning}</div>
        </div>
      )}

      {/* Verification Results */}
      <div className="step-detail-section">
        <h3>Verification</h3>
        <div className="verification-grid">
          <div className={`verify-cell ${step.sympy_passed ? 'v-pass' : step.sympy_passed === false ? 'v-fail' : 'v-skip'}`}>
            <span className="verify-label">SymPy</span>
            <span className="verify-result">
              {step.sympy_passed == null ? 'Skipped' : step.sympy_passed ? 'Pass' : 'Fail'}
            </span>
          </div>
          <div className={`verify-cell ${step.pint_passed ? 'v-pass' : step.pint_passed === false ? 'v-fail' : 'v-skip'}`}>
            <span className="verify-label">Pint</span>
            <span className="verify-result">
              {step.pint_passed == null ? 'Skipped' : step.pint_passed ? 'Pass' : 'Fail'}
            </span>
          </div>
          <div className={`verify-cell ${step.lean_passed ? 'v-pass' : step.lean_passed === false ? 'v-fail' : 'v-skip'}`}>
            <span className="verify-label">Lean</span>
            <span className="verify-result">
              {step.lean_passed == null ? 'Skipped' : step.lean_passed ? 'Pass' : 'Fail'}
            </span>
          </div>
        </div>
      </div>

      {/* Tool Calls */}
      {toolRuns.length > 0 && (
        <div className="step-detail-section">
          <h3>Tool Calls ({toolRuns.length})</h3>
          <div className="tool-runs-list">
            {toolRuns.map(run => {
              const expanded = expandedRuns.has(run.id);
              let inputPreview = '';
              try {
                const parsed = JSON.parse(run.query_json);
                inputPreview = JSON.stringify(parsed, null, 0);
                if (inputPreview.length > 120) inputPreview = inputPreview.slice(0, 120) + '...';
              } catch { inputPreview = run.query_json.slice(0, 120); }

              return (
                <div key={run.id} className={`tool-run-card ${toolStatusClass(run.status)}`}>
                  <div className="tool-run-header" onClick={() => toggleRun(run.id)}>
                    <span className="tool-run-icon">{toolIcon(run.tool_name)}</span>
                    <span className="tool-run-name">{run.tool_name}</span>
                    <span className={`tool-run-status ${toolStatusClass(run.status)}`}>
                      {run.status}
                    </span>
                    {run.latency_ms != null && (
                      <span className="tool-run-latency">{run.latency_ms}ms</span>
                    )}
                    <span className="tool-run-toggle">{expanded ? '\u25BC' : '\u25B6'}</span>
                  </div>
                  {!expanded && run.result_summary && (
                    <div className="tool-run-preview">
                      {run.result_summary.length > 120
                        ? run.result_summary.slice(0, 120) + '...'
                        : run.result_summary}
                    </div>
                  )}
                  {expanded && (
                    <div className="tool-run-body">
                      <div className="tool-run-detail">
                        <h4>Input</h4>
                        <pre className="tool-run-json">{inputPreview}</pre>
                      </div>
                      {run.result_summary && (
                        <div className="tool-run-detail">
                          <h4>Result</h4>
                          <div className="tool-run-result">{run.result_summary}</div>
                        </div>
                      )}
                      {run.error_message && (
                        <div className="tool-run-detail">
                          <h4>Error</h4>
                          <div className="tool-run-error">{run.error_message}</div>
                        </div>
                      )}
                      <div className="tool-run-meta">
                        <span>Agent: {run.agent_role}</span>
                        {run.latency_ms != null && <span>Latency: {run.latency_ms}ms</span>}
                        <span>At: {new Date(run.created_at).toLocaleTimeString()}</span>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Adversarial Challenge */}
      {step.challenge_model && (
        <div className="step-detail-section">
          <h3>Adversarial Challenge</h3>
          <div className="challenge-detail">
            <span>Model: {step.challenge_model}</span>
            <span>Confidence: {step.challenge_confidence != null ? `${(step.challenge_confidence * 100).toFixed(0)}%` : '-'}</span>
            <span className={step.challenge_fatal ? 'ch-fatal' : 'ch-ok'}>
              {step.challenge_fatal ? 'FATAL FLAW' : step.challenge_flaw_found ? 'Flaw Found' : 'Shield'}
            </span>
          </div>
          {step.challenge_attack && (
            <div className="challenge-attack">{step.challenge_attack}</div>
          )}
        </div>
      )}

      {/* Rejection Reason */}
      {!step.verified && step.rejection_reason && (
        <div className="step-detail-section">
          <h3>Rejection Reason</h3>
          <div className="step-detail-rejection">{step.rejection_reason}</div>
        </div>
      )}

      {/* Claims Produced */}
      {claims.length > 0 && (
        <div className="step-detail-section">
          <h3>Claims Extracted ({claims.length})</h3>
          <div className="step-claims-list">
            {claims.map(c => (
              <div key={c.id} className="step-claim">
                <span className="claim-type">{c.claim_type}</span>
                <span className="claim-natural">{c.natural_text}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* DAG Neighbors */}
      {edges.length > 0 && (
        <div className="step-detail-section">
          <h3>DAG Neighbors ({edges.length})</h3>
          <div className="step-edges-list">
            {edges.map(e => (
              <div key={e.id} className="step-edge">
                <span className="edge-direction">
                  {e.source_id === stepId ? '\u2192' : '\u2190'}
                </span>
                <span className="edge-type">{e.edge_type}</span>
                <span className="edge-target">
                  {e.source_id === stepId ? e.target_id.slice(0, 8) : e.source_id.slice(0, 8)}...
                </span>
                <span className="edge-node-type">
                  ({e.source_id === stepId ? e.target_type : e.source_type})
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
