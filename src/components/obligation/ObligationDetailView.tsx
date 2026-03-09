import { useEffect, useState } from 'react';
import { useNavigationStore } from '../../stores/navigationStore';
import { getObligationDetail, getObligationProofNodes, getObligationSignals, getToolRunsForObligation } from '../../services/tauri';
import type { ObligationFull, ProofNode, SatisfactionSignal, ToolRun } from '../../types';

interface Props {
  obligationId: string;
}

const priorityLabel = (p: number): string => {
  if (p >= 0.9) return 'S+';
  if (p >= 0.7) return 'S';
  if (p >= 0.5) return 'A';
  if (p >= 0.3) return 'B';
  return 'C';
};

const statusLabel = (s: string): string => {
  if (s.includes('proved')) return 'PROVED';
  if (s.includes('refuted')) return 'REFUTED';
  return s.toUpperCase();
};

const statusClass = (s: string): string => {
  if (s.includes('proved')) return 'ob-proved';
  if (s.includes('refuted')) return 'ob-refuted';
  if (s === 'open') return 'ob-open';
  return 'ob-other';
};

export function ObligationDetailView({ obligationId }: Props) {
  const { goBack } = useNavigationStore();
  const [ob, setOb] = useState<ObligationFull | null>(null);
  const [nodes, setNodes] = useState<ProofNode[]>([]);
  const [signals, setSignals] = useState<SatisfactionSignal[]>([]);
  const [expandedNodes, setExpandedNodes] = useState<Set<string>>(new Set());
  const [toolRuns, setToolRuns] = useState<ToolRun[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      getObligationDetail(obligationId),
      getObligationProofNodes(obligationId),
      getObligationSignals(obligationId),
    ]).then(([detail, proofNodes, sigs]) => {
      setOb(detail);
      setNodes(proofNodes);
      setSignals(sigs);
    }).catch(e => setError(String(e)));
    getToolRunsForObligation(obligationId).then(setToolRuns).catch(() => setToolRuns([]));
  }, [obligationId]);

  const toggleNode = (id: string) => {
    setExpandedNodes(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  if (error) {
    return (
      <div className="ob-detail-view">
        <p className="ob-detail-error">Failed to load obligation: {error}</p>
        <button className="btn" onClick={goBack}>Back</button>
      </div>
    );
  }

  if (!ob) {
    return (
      <div className="ob-detail-view">
        <p className="ob-detail-loading">Loading...</p>
      </div>
    );
  }

  const yesCount = signals.filter(s => s.satisfies).length;

  return (
    <div className="ob-detail-view">
      {/* Header */}
      <div className="ob-detail-header">
        <button className="btn btn-sm" onClick={goBack}>Back</button>
        <span className="ob-detail-priority">[{priorityLabel(ob.priority)}]</span>
        <span className="ob-detail-type">{ob.obligation_type}</span>
        <span className={`ob-detail-status ${statusClass(ob.status)}`}>
          {statusLabel(ob.status)}
        </span>
      </div>

      {/* Description */}
      <div className="ob-detail-section">
        <h3>Description</h3>
        <div className="ob-detail-desc">{ob.description}</div>
      </div>

      {/* Metadata */}
      <div className="ob-detail-section">
        <h3>Details</h3>
        <div className="ob-detail-meta-grid">
          <div className="ob-meta-item">
            <span className="ob-meta-label">Confidence</span>
            <span className="ob-meta-value">{(ob.confidence * 100).toFixed(0)}%</span>
          </div>
          <div className="ob-meta-item">
            <span className="ob-meta-label">Budget</span>
            <span className="ob-meta-value">{ob.steps_spent} / {ob.max_steps} steps</span>
          </div>
          <div className="ob-meta-item">
            <span className="ob-meta-label">Escalation</span>
            <span className="ob-meta-value">{ob.escalation_level}</span>
          </div>
          {ob.assigned_model && (
            <div className="ob-meta-item">
              <span className="ob-meta-label">Model</span>
              <span className="ob-meta-value ob-meta-model">{ob.assigned_model}</span>
            </div>
          )}
          {ob.assigned_models_json && (
            <div className="ob-meta-item">
              <span className="ob-meta-label">Fan-in Workers</span>
              <span className="ob-meta-value ob-meta-mono">{ob.assigned_models_json}</span>
            </div>
          )}
          {ob.active_solver_round_id && (
            <div className="ob-meta-item">
              <span className="ob-meta-label">Active Round</span>
              <span className="ob-meta-value ob-meta-mono">{ob.active_solver_round_id.slice(0, 8)}</span>
            </div>
          )}
          {ob.source_layer != null && (
            <div className="ob-meta-item">
              <span className="ob-meta-label">Source Layer</span>
              <span className="ob-meta-value">
                {ob.source_layer === 0 ? 'Decomposer' : ob.source_layer === 1 ? 'Self-tag' : ob.source_layer === 3 ? 'Classifier' : ob.source_layer === 4 ? 'Validator' : `Layer ${ob.source_layer}`}
              </span>
            </div>
          )}
          {ob.depends_on && (
            <div className="ob-meta-item">
              <span className="ob-meta-label">Depends On</span>
              <span className="ob-meta-value ob-meta-mono">{ob.depends_on}</span>
            </div>
          )}
          <div className="ob-meta-item">
            <span className="ob-meta-label">Created</span>
            <span className="ob-meta-value">{new Date(ob.created_at).toLocaleString()}</span>
          </div>
          {ob.closed_at && (
            <div className="ob-meta-item">
              <span className="ob-meta-label">Closed</span>
              <span className="ob-meta-value">{new Date(ob.closed_at).toLocaleString()}</span>
            </div>
          )}
          {ob.closure_type && (
            <div className="ob-meta-item">
              <span className="ob-meta-label">Closure Type</span>
              <span className="ob-meta-value">{ob.closure_type}</span>
            </div>
          )}
        </div>
      </div>

      {/* Satisfaction Signals */}
      {signals.length > 0 && (
        <div className="ob-detail-section">
          <h3>Satisfaction Signals ({yesCount}/{signals.length})</h3>
          <div className="ob-signals-list">
            {signals.map(sig => (
              <div key={sig.id} className={`ob-signal-row ${sig.satisfies ? 'sig-yes' : 'sig-no'}`}>
                <span className="sig-icon">{sig.satisfies ? '\u2713' : '\u2717'}</span>
                <span className="sig-source">{sig.source}</span>
                <span className="sig-confidence">{(sig.confidence * 100).toFixed(0)}%</span>
                {sig.note && <span className="sig-note">{sig.note}</span>}
                <span className="sig-time">{new Date(sig.created_at).toLocaleTimeString()}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Satisfaction Criteria */}
      {ob.satisfaction_criteria && (
        <div className="ob-detail-section">
          <h3>Satisfaction Criteria</h3>
          <pre className="ob-detail-json">{ob.satisfaction_criteria}</pre>
        </div>
      )}

      {/* Proof Nodes */}
      <div className="ob-detail-section">
        <h3>Proof Nodes ({nodes.length})</h3>
        {nodes.length === 0 ? (
          <div className="ob-detail-empty">No proof nodes yet</div>
        ) : (
          <div className="ob-nodes-list">
            {nodes.map(node => {
              const expanded = expandedNodes.has(node.id);
              return (
                <div key={node.id} className={`ob-node-card ${node.status}`}>
                  <div className="ob-node-header" onClick={() => toggleNode(node.id)}>
                    <span className="ob-node-seq">#{node.sequence_number}</span>
                    <span className="ob-node-type">{node.node_type}</span>
                    <span className={`ob-node-status ${node.status === 'verified' ? 'v-pass' : 'v-fail'}`}>
                      {node.status}
                    </span>
                    {node.technique_class && (
                      <span className="ob-node-technique">{node.technique_class}</span>
                    )}
                    <span className="ob-node-toggle">{expanded ? '\u25BC' : '\u25B6'}</span>
                  </div>
                  {expanded && (
                    <div className="ob-node-body">
                      <div className="ob-node-content">
                        <h4>Reasoning</h4>
                        <div className="ob-node-text">{node.content}</div>
                      </div>
                      {node.formal_content && (
                        <div className="ob-node-content">
                          <h4>Formal</h4>
                          <pre className="ob-node-formal">{node.formal_content}</pre>
                        </div>
                      )}
                      {node.validator_used && (
                        <div className="ob-node-content">
                          <h4>Validation ({node.validator_used})</h4>
                          <pre className="ob-node-validation">{node.validator_result || 'No result'}</pre>
                        </div>
                      )}
                      {node.construction_family && (
                        <div className="ob-node-content">
                          <h4>Construction Family</h4>
                          <span>{node.construction_family}</span>
                        </div>
                      )}
                      {node.token_cost != null && (
                        <div className="ob-node-content">
                          <h4>Token Cost</h4>
                          <span>{node.token_cost.toLocaleString()}</span>
                        </div>
                      )}
                      <div className="ob-node-content">
                        <h4>Created</h4>
                        <span>{new Date(node.created_at).toLocaleString()}</span>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Tool Calls */}
      {toolRuns.length > 0 && (
        <div className="ob-detail-section">
          <h3>Tool Calls ({toolRuns.length})</h3>
          <div className="tool-runs-list">
            {toolRuns.map(run => (
              <div key={run.id} className={`tool-run-card ${run.status === 'completed' ? 'tool-ok' : run.status === 'failed' ? 'tool-fail' : 'tool-pending'}`}>
                <div className="tool-run-header">
                  <span className="tool-run-name">{run.tool_name}</span>
                  <span className={`tool-run-status ${run.status === 'completed' ? 'tool-ok' : 'tool-fail'}`}>{run.status}</span>
                  {run.latency_ms != null && <span className="tool-run-latency">{run.latency_ms}ms</span>}
                  {run.step_number != null && <span className="tool-run-step">step {run.step_number}</span>}
                </div>
                {run.result_summary && (
                  <div className="tool-run-preview">
                    {run.result_summary.length > 150 ? run.result_summary.slice(0, 150) + '...' : run.result_summary}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Search Space */}
      {ob.search_space && (
        <div className="ob-detail-section">
          <h3>Search Space</h3>
          <pre className="ob-detail-json">{ob.search_space}</pre>
        </div>
      )}
    </div>
  );
}
