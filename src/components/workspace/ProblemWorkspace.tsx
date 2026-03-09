import { useEffect, useState } from 'react';
import { useProblemStore } from '../../stores/problemStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useNavigationStore } from '../../stores/navigationStore';
import { useLoopStore } from '../../stores/loopStore';
import { LoopControls } from '../loop/LoopControls';
import { ThinkingPanel } from '../loop/ThinkingPanel';
import { updateProblemTitle } from '../../services/tauri';
import { ExportPanel } from '../export/ExportPanel';
import { renderLatexText } from '../../utils/latex';

interface Props {
  problemId: string;
}

export function ProblemWorkspace({ problemId }: Props) {
  const { problems, fetchProblems } = useProblemStore();
  const { attempts, attemptsLoading, loadAttempts, reports, loadReports, deleteAttempt } = useWorkspaceStore();
  const { goToAttempt } = useNavigationStore();
  const { status, currentStep } = useLoopStore();
  const problem = problems.find(p => p.id === problemId);

  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState('');

  useEffect(() => {
    loadAttempts(problemId);
    loadReports(problemId);
  }, [problemId]);

  // Keep attempts table in sync while a run is active — refresh after each step
  // so step count, V/R columns, and status update without requiring navigation.
  useEffect(() => {
    if (status === 'running') {
      loadAttempts(problemId);
    }
  }, [currentStep]);

  // Also set activeProblem for backward compat with LoopControls
  useEffect(() => {
    if (problem) {
      useProblemStore.getState().setActiveProblem(problem);
    }
  }, [problem]);

  if (!problem) return <div className="workspace-empty">Problem not found</div>;

  const handleTitleSave = async () => {
    if (titleDraft.trim()) {
      await updateProblemTitle(problemId, titleDraft.trim());
      fetchProblems();
    }
    setEditingTitle(false);
  };

  const bestCoverage = reports.length > 0
    ? Math.max(...reports.map(r => r.coverage ?? 0))
    : null;

  return (
    <div className="problem-workspace">
      {/* Problem Header */}
      <div className="workspace-header">
        <div className="workspace-title-row">
          {editingTitle ? (
            <input
              className="title-edit"
              value={titleDraft}
              onChange={e => setTitleDraft(e.target.value)}
              onBlur={handleTitleSave}
              onKeyDown={e => e.key === 'Enter' && handleTitleSave()}
              autoFocus
            />
          ) : (
            <h2
              className="workspace-title"
              onClick={() => { setTitleDraft(problem.title || ''); setEditingTitle(true); }}
              title="Click to edit title"
            >
              {problem.title || 'Untitled Problem'}
            </h2>
          )}
          <span className={`status-badge status-${problem.status}`}>{problem.status}</span>
          {problem.difficulty && <span className="difficulty-badge">{problem.difficulty}</span>}
        </div>
        <div className="workspace-statement" dangerouslySetInnerHTML={{ __html: renderLatexText(problem.statement) }} />
        <div className="workspace-meta">
          {problem.domain && <span>Domain: {problem.domain}</span>}
          {problem.source && <span>Source: {problem.source}</span>}
          {problem.known_answer && <span>Known Answer: {problem.known_answer}</span>}
          {bestCoverage !== null && <span>Best Coverage: {(bestCoverage * 100).toFixed(0)}%</span>}
        </div>
      </div>

      {/* Loop Controls for active solving */}
      <LoopControls />
      {status === 'running' && <ThinkingPanel />}

      {/* Attempt Table */}
      <div className="workspace-section">
        <h3>Attempts ({attempts.length})</h3>
        {attemptsLoading ? (
          <div className="loading">Loading attempts...</div>
        ) : (
          <table className="attempt-table">
            <thead>
              <tr>
                <th>#</th>
                <th>Status</th>
                <th>Steps</th>
                <th>V / R</th>
                <th>Coverage</th>
                <th>Efficiency</th>
                <th>Models</th>
                <th>Started</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {attempts.length === 0 && (
                <tr><td colSpan={9} className="library-empty">No attempts yet</td></tr>
              )}
              {attempts.map((a, i) => (
                <tr
                  key={a.id}
                  className="attempt-row"
                  onClick={() => goToAttempt(problemId, a.id)}
                >
                  <td>{a.attempt_number ?? attempts.length - i}</td>
                  <td><span className={`status-badge status-${a.status}`}>{a.status}</span></td>
                  <td>{a.step_count}</td>
                  <td>
                    <span className="verified-count">{a.steps_verified}</span>
                    {' / '}
                    <span className="rejected-count">{a.steps_rejected}</span>
                  </td>
                  <td>{a.coverage != null ? `${(a.coverage * 100).toFixed(0)}%` : '-'}</td>
                  <td>{a.efficiency != null ? `${(a.efficiency * 100).toFixed(0)}%` : '-'}</td>
                  <td className="models-cell">{a.models_used || '-'}</td>
                  <td>{new Date(a.started_at).toLocaleDateString()}</td>
                  <td style={{ display: 'flex', gap: '4px' }}>
                    {(a.status === 'stopped' || a.status === 'paused' || a.status === 'reviewed') && status !== 'running' && (
                      <button
                        className="btn btn-sm btn-primary"
                        title={`Resume attempt #${a.attempt_number ?? attempts.length - i} from step ${a.step_count}`}
                        onClick={async (e) => {
                          e.stopPropagation();
                          await useLoopStore.getState().continueSolve(problemId, a.id);
                        }}
                      >
                        Resume
                      </button>
                    )}
                    <button
                      className="btn btn-sm btn-danger"
                      title="Delete this attempt and all its data"
                      onClick={async (e) => {
                        e.stopPropagation();
                        if (window.confirm(`Delete attempt #${a.attempt_number ?? attempts.length - i} and all its steps? This cannot be undone.`)) {
                          console.log('[Delete] Deleting attempt', a.id, 'from problem', problemId);
                          await deleteAttempt(a.id, problemId);
                          console.log('[Delete] Done');
                        }
                      }}
                    >
                      Del
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Cross-Attempt Summary */}
      {reports.length > 0 && (
        <div className="workspace-section">
          <h3>Cross-Attempt Summary</h3>
          <div className="cross-summary">
            <div className="summary-stat">
              <span className="stat-value">{reports.length}</span>
              <span className="stat-label">Reports</span>
            </div>
            <div className="summary-stat">
              <span className="stat-value">
                {reports.reduce((s, r) => s + r.obligations_closed, 0)}
              </span>
              <span className="stat-label">Obligations Closed</span>
            </div>
            <div className="summary-stat">
              <span className="stat-value">
                {reports.reduce((s, r) => s + r.contradictions, 0)}
              </span>
              <span className="stat-label">Contradictions</span>
            </div>
            <div className="summary-stat">
              <span className="stat-value">
                {reports.reduce((s, r) => s + r.death_spirals, 0)}
              </span>
              <span className="stat-label">Death Spirals</span>
            </div>
          </div>
        </div>
      )}

      {/* Export Panel */}
      <ExportPanel problemId={problemId} />
    </div>
  );
}
