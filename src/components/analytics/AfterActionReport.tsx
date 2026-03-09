import { useEffect, useState } from 'react';
import { formalizeProof, getAfterActionReport } from '../../services/tauri';
import { useLoopStore } from '../../stores/loopStore';
import type { AfterActionReport as AARType, LeanFormalizationResult } from '../../types';
import katex from 'katex';

function renderFormal(expr: string): string {
  let tex = expr
    .replace(/\*\*/g, '^')
    .replace(/\*/g, ' \\cdot ')
    .replace(/sqrt\(([^)]+)\)/g, '\\sqrt{$1}')
    .replace(/pi/g, '\\pi')
    .replace(/([a-z])(\d)/gi, '$1_{$2}');
  tex = tex.replace(/\^(\d{2,})/g, '^{$1}');
  try {
    return katex.renderToString(tex, { throwOnError: false, displayMode: false });
  } catch {
    return expr;
  }
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

function formatMs(ms: number): string {
  if (ms >= 60_000) return `${(ms / 60_000).toFixed(1)}m`;
  if (ms >= 1_000) return `${(ms / 1_000).toFixed(1)}s`;
  return `${ms}ms`;
}

interface Props {
  problemId: string;
  onClose: () => void;
}

export function AfterActionReport({ problemId, onClose }: Props) {
  const [report, setReport] = useState<AARType | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [formalization, setFormalization] = useState<LeanFormalizationResult | null>(null);
  const [formalizeError, setFormalizeError] = useState<string | null>(null);
  const [formalizing, setFormalizing] = useState(false);
  const reviewResult = useLoopStore((s) => s.reviewResult);
  const auditResults = useLoopStore((s) => s.auditResults);

  useEffect(() => {
    getAfterActionReport(problemId)
      .then(setReport)
      .catch((e) => setError(String(e)));
  }, [problemId]);

  if (error) return (
    <div className="aar-overlay" onClick={onClose}>
      <div className="aar-panel" onClick={(e) => e.stopPropagation()}>
        <div className="aar-header">
          <h2>After Action Report</h2>
          <button className="btn btn-sm btn-secondary" onClick={onClose}>Close</button>
        </div>
        <p className="aar-error">Failed to load report: {error}</p>
      </div>
    </div>
  );

  if (!report) return (
    <div className="aar-overlay" onClick={onClose}>
      <div className="aar-panel" onClick={(e) => e.stopPropagation()}>
        <p className="muted">Loading report...</p>
        <button className="btn btn-sm btn-secondary" onClick={onClose}>Cancel</button>
      </div>
    </div>
  );

  const proofComplete = report.proof_complete;
  const handleFormalize = async () => {
    setFormalizing(true);
    setFormalizeError(null);
    try {
      setFormalization(await formalizeProof(problemId));
    } catch (e) {
      setFormalization(null);
      setFormalizeError(String(e));
    } finally {
      setFormalizing(false);
    }
  };

  return (
    <div className="aar-overlay" onClick={onClose}>
      <div className="aar-panel" onClick={(e) => e.stopPropagation()}>
        <div className="aar-header">
          <div>
            <h2>After Action Report</h2>
            <span className="aar-subtitle">{report.problem_domain} / {new Date(report.started_at).toLocaleDateString()}</span>
          </div>
          <div style={{ display: 'flex', gap: '0.5rem' }}>
            {proofComplete && (
              <button className="btn btn-sm btn-primary" onClick={handleFormalize} disabled={formalizing}>
                {formalizing ? 'Formalizing...' : 'Formalize in Lean 4'}
              </button>
            )}
            <button className="btn btn-sm btn-secondary" onClick={onClose}>Close</button>
          </div>
        </div>

        {/* Problem statement */}
        <div className="aar-section">
          <div className="aar-label">Problem</div>
          <div className="aar-problem">{report.problem_statement}</div>
        </div>

        {/* Outcome banner */}
        <div className={`aar-outcome ${proofComplete ? 'success' : 'incomplete'}`}>
          {proofComplete ? 'PROOF VERIFIED' : 'INCOMPLETE'}
        </div>

        {/* Final answer */}
        {report.final_answer && (
          <div className="aar-section">
            <div className="aar-label">Final Answer</div>
            <div className="aar-final-answer">{report.final_answer}</div>
          </div>
        )}

        {(formalization || formalizeError) && (
          <div className="aar-section">
            <div className="aar-label">Lean Formalization</div>
            {formalizeError ? (
              <div className="aar-error">Formalization failed: {formalizeError}</div>
            ) : formalization && (
              <>
                <div className={`aar-outcome ${formalization.success ? 'success' : 'incomplete'}`}>
                  {formalization.success ? 'Compiled' : 'Needs Revision'}
                </div>
                <div className="aar-training-label">
                  Attempts: <strong>{formalization.attempts}</strong>
                </div>
                {formalization.errors.length > 0 && (
                  <div className="aar-failures">
                    {formalization.errors.map((entry, index) => (
                      <div key={index} className="aar-failure-row">
                        <span className="aar-failure-reason">{entry}</span>
                      </div>
                    ))}
                  </div>
                )}
                <pre className="aar-chain-formal" style={{ whiteSpace: 'pre-wrap' }}>{formalization.lean_source}</pre>
              </>
            )}
          </div>
        )}

        {/* Open obligations warning */}
        {report.open_obligations > 0 && (
          <div className="aar-section">
            <div className="aar-label aar-red">{report.open_obligations} Open Obligation{report.open_obligations > 1 ? 's' : ''}</div>
          </div>
        )}

        {/* Stats grid */}
        <div className="aar-stats">
          <div className="aar-stat">
            <span className="aar-stat-value">{report.total_steps}</span>
            <span className="aar-stat-label">Total Steps</span>
          </div>
          <div className="aar-stat">
            <span className="aar-stat-value aar-green">{report.verified_steps}</span>
            <span className="aar-stat-label">Verified</span>
          </div>
          <div className="aar-stat">
            <span className="aar-stat-value aar-red">{report.rejected_steps}</span>
            <span className="aar-stat-label">Rejected</span>
          </div>
          <div className="aar-stat">
            <span className="aar-stat-value">{report.accuracy_pct.toFixed(0)}%</span>
            <span className="aar-stat-label">Accuracy</span>
          </div>
          <div className="aar-stat">
            <span className="aar-stat-value">{formatTokens(report.total_tokens_in)}</span>
            <span className="aar-stat-label">Tokens In</span>
          </div>
          <div className="aar-stat">
            <span className="aar-stat-value">{formatTokens(report.total_tokens_out)}</span>
            <span className="aar-stat-label">Tokens Out</span>
          </div>
          <div className="aar-stat">
            <span className="aar-stat-value">{formatMs(report.total_wall_ms)}</span>
            <span className="aar-stat-label">Validation Time</span>
          </div>
          <div className="aar-stat">
            <span className="aar-stat-value">{report.models_used.length}</span>
            <span className="aar-stat-label">Models</span>
          </div>
        </div>

        {/* Models used */}
        <div className="aar-section">
          <div className="aar-label">Models Used</div>
          <div className="aar-models">
            {report.models_used.map((m) => <span key={m} className="aar-model-tag">{m}</span>)}
          </div>
        </div>

        {/* Verified chain — the clean proof */}
        {report.verified_chain.length > 0 && (
          <div className="aar-section">
            <div className="aar-label">Verified Proof Chain</div>
            <div className="aar-chain">
              {report.verified_chain.map((s, i) => (
                <div key={i} className="aar-chain-step">
                  <span className="aar-chain-num">{s.step_number}.</span>
                  <div className="aar-chain-body">
                    <div className="aar-chain-natural">{s.natural}</div>
                    {s.formal && (
                      <div className="aar-chain-formal" dangerouslySetInnerHTML={{ __html: renderFormal(s.formal) }} />
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Failure modes */}
        {report.failure_modes.length > 0 && (
          <div className="aar-section">
            <div className="aar-label">Failure Modes</div>
            <div className="aar-failures">
              {report.failure_modes.map((f, i) => (
                <div key={i} className="aar-failure-row">
                  <span className="aar-failure-count">{f.count}x</span>
                  <span className="aar-failure-reason">{f.reason}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Exploration Coverage from review */}
        {reviewResult && (
          <div className="aar-section">
            <div className="aar-label">Exploration Analysis</div>
            <div className="aar-stats" style={{ marginBottom: '0.75rem' }}>
              <div className="aar-stat">
                <span className="aar-stat-value">{Math.round(reviewResult.exploration_coverage * 100)}%</span>
                <span className="aar-stat-label">Coverage</span>
              </div>
              <div className="aar-stat">
                <span className={`aar-stat-value ${reviewResult.conclusion_sound ? 'aar-green' : 'aar-red'}`}>
                  {reviewResult.conclusion_sound ? 'Sound' : 'Unsound'}
                </span>
                <span className="aar-stat-label">Conclusion</span>
              </div>
              <div className="aar-stat">
                <span className="aar-stat-value">{Math.round(reviewResult.conclusion_confidence * 100)}%</span>
                <span className="aar-stat-label">Confidence</span>
              </div>
              <div className="aar-stat">
                <span className="aar-stat-value">{reviewResult.findings_count}</span>
                <span className="aar-stat-label">Findings</span>
              </div>
            </div>
            <div className="aar-training-label">
              Training label: <strong>{reviewResult.training_label.replace(/_/g, ' ')}</strong>
            </div>
            {reviewResult.findings.length > 0 && (
              <div className="aar-findings">
                {reviewResult.findings.map((f, i) => (
                  <div key={i} className={`aar-finding priority-${f.priority}`}>
                    <div className="aar-finding-header">
                      <span className="aar-finding-type">{f.type.replace(/_/g, ' ')}</span>
                      <span className={`aar-finding-priority ${f.priority}`}>{f.priority}</span>
                    </div>
                    <div className="aar-finding-summary">{f.summary}</div>
                    <div className="aar-finding-detail">{f.detail}</div>
                  </div>
                ))}
              </div>
            )}
            {reviewResult.missing_constructions.length > 0 && (
              <div className="aar-missing-constructions">
                <strong>Missing constructions:</strong> {reviewResult.missing_constructions.join(', ')}
              </div>
            )}
          </div>
        )}

        {/* Audit history */}
        {auditResults.length > 0 && (
          <div className="aar-section">
            <div className="aar-label">Exploration Audits ({auditResults.length})</div>
            <div className="aar-audits">
              {auditResults.map((a, i) => (
                <div key={i} className="aar-audit-row">
                  <span className="aar-audit-step">Step {a.step_number}</span>
                  <span className="aar-audit-breadth">Breadth: {Math.round(a.breadth * 100)}%</span>
                  <span className="aar-audit-conf">Conf: {Math.round(a.confidence * 100)}%</span>
                  {a.should_branch && <span className="aar-audit-branch">Branch</span>}
                  <div className="aar-audit-direction">{a.recommended_direction}</div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* What's next */}
        <div className="aar-section">
          <div className="aar-label">Next Milestones</div>
          <div className="aar-next">
            <div className="aar-next-item done">M1: First Verified Proof</div>
            <div className="aar-next-item current">M2: Pattern Extraction — extract reusable strategies from this proof</div>
            <div className="aar-next-item">M3: Multi-Model — run parallel workers through shared DB</div>
            <div className="aar-next-item">M4: Council Deliberation — multi-model review after attempts</div>
            <div className="aar-next-item">M5: Self-Modification</div>
            <div className="aar-next-item">M6: Novel Result</div>
            <div className="aar-next-item">M7: Training Data Export</div>
          </div>
        </div>
      </div>
    </div>
  );
}
