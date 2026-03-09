import { useEffect, useState } from 'react';
import { listAllSteps } from '../../services/tauri';
import { useLoopStore } from '../../stores/loopStore';
import { useProblemStore } from '../../stores/problemStore';
import type { TrainingRow } from '../../types';
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

export function TrainingDataTable() {
  const [rows, setRows] = useState<TrainingRow[]>([]);
  const [filter, setFilter] = useState<'all' | 'verified' | 'rejected'>('all');
  const [scope, setScope] = useState<'problem' | 'global'>('problem');
  const [expanded, setExpanded] = useState<string | null>(null);
  const status = useLoopStore((s) => s.status);
  const { activeProblem } = useProblemStore();

  useEffect(() => {
    listAllSteps(200).then(setRows).catch(console.error);
  }, [status]);

  const scoped = scope === 'problem' && activeProblem
    ? rows.filter((r) => r.problem_id === activeProblem.id)
    : rows;

  const filtered = scoped.filter((r) => {
    if (filter === 'verified') return r.verified;
    if (filter === 'rejected') return !r.verified;
    return true;
  });

  if (!rows.length) return null;

  return (
    <div className="training-table">
      <div className="training-table-header">
        <h3>Raw Training Data</h3>
        <div className="training-filters">
          <div className="scope-toggle">
            <button className={`btn btn-sm ${scope === 'problem' ? 'btn-accent' : 'btn-secondary'}`} onClick={() => setScope('problem')}>This Problem</button>
            <button className={`btn btn-sm ${scope === 'global' ? 'btn-accent' : 'btn-secondary'}`} onClick={() => setScope('global')}>All Problems</button>
          </div>
          <button className={`btn btn-sm ${filter === 'all' ? 'btn-primary' : 'btn-secondary'}`} onClick={() => setFilter('all')}>All ({scoped.length})</button>
          <button className={`btn btn-sm ${filter === 'verified' ? 'btn-primary' : 'btn-secondary'}`} onClick={() => setFilter('verified')}>Verified ({scoped.filter(r => r.verified).length})</button>
          <button className={`btn btn-sm ${filter === 'rejected' ? 'btn-primary' : 'btn-secondary'}`} onClick={() => setFilter('rejected')}>Rejected ({scoped.filter(r => !r.verified).length})</button>
        </div>
      </div>
      <div className="training-rows">
        {filtered.map((r) => (
          <div
            key={r.id}
            className={`training-row ${r.stale_sibling ? 'rejected' : r.verified ? 'verified' : 'rejected'} ${expanded === r.id ? 'expanded' : ''}`}
            onClick={() => setExpanded(expanded === r.id ? null : r.id)}
          >
            <div className="training-row-summary">
              <span className={`td-verdict ${r.stale_sibling ? 'fail' : r.verified ? 'pass' : 'fail'}`}>
                {r.stale_sibling ? '\u23F8' : r.verified ? '\u2713' : '\u2717'}
              </span>
              <span className="td-step">#{r.step_number}</span>
              <span className="td-type">{r.proposal_type}</span>
              {r.obligation_type && <span className="td-type">{r.obligation_type}</span>}
              <span className="td-natural">{r.proposal_natural}</span>
              <span className="td-model">{r.model}</span>
              <div className="td-validators">
                {r.sympy_passed != null && <span className={r.sympy_passed ? 'v-pass' : 'v-fail'}>S</span>}
                {r.pint_passed != null && <span className={r.pint_passed ? 'v-pass' : 'v-fail'}>P</span>}
                {r.lean_passed != null && <span className={r.lean_passed ? 'v-pass' : 'v-fail'}>L</span>}
              </div>
            </div>
            {expanded === r.id && (
              <div className="training-row-detail">
                <div className="td-field"><span className="td-label">Problem</span><span>{r.problem_statement}</span></div>
                <div className="td-field"><span className="td-label">Domain</span><span>{r.problem_domain || 'n/a'}</span></div>
                {r.obligation_desc && (
                  <div className="td-field"><span className="td-label">Obligation</span><span>{r.obligation_desc}</span></div>
                )}
                {r.semantic_redundant && (
                  <div className="td-field"><span className="td-label">Redundancy</span><span>Semantically redundant with an earlier verified step on the same obligation.</span></div>
                )}
                {r.stale_sibling && (
                  <div className="td-field"><span className="td-label">Redundancy</span><span>Marked stale sibling during parallel fan-in.</span></div>
                )}
                {r.proposal_formal && (
                  <div className="td-field">
                    <span className="td-label">Formal</span>
                    <span className="td-formal" dangerouslySetInnerHTML={{ __html: renderFormal(r.proposal_formal) }} />
                  </div>
                )}
                {r.rejection_reason && <div className="td-field"><span className="td-label">Rejection</span><span className="td-rejection-text">{r.rejection_reason}</span></div>}
                <div className="td-field"><span className="td-label">Time</span><span>{new Date(r.created_at).toLocaleString()}</span></div>
                <div className="td-field"><span className="td-label">ID</span><span className="td-id">{r.id}</span></div>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
