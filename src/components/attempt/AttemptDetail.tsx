import { useEffect, useState } from 'react';
import { useLoopStore } from '../../stores/loopStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useNavigationStore } from '../../stores/navigationStore';
import { useProblemStore } from '../../stores/problemStore';
import { LoopControls } from '../loop/LoopControls';
import { ThinkingPanel } from '../loop/ThinkingPanel';
import { ObligationQueue } from './ObligationQueue';
import { ObligationLanes } from './ObligationLanes';
import { ProofChain } from './ProofChain';
import { ClaimsPanel } from './ClaimsPanel';
import { ConflictsPanel } from './ConflictsPanel';

interface Props {
  problemId: string;
  attemptId: string;
}

export function AttemptDetail({ problemId, attemptId }: Props) {
  const { steps, obligations, status, attemptId: activeAttemptId } = useLoopStore();
  const {
    loadClaims, loadConflicts, claims, conflicts,
    clearAttemptData, loadCurrentReport, currentReport,
    dbSteps, loadDbSteps, dbObligations, loadDbObligations,
    attempts,
  } = useWorkspaceStore();
  const { goToStep, goToObligation } = useNavigationStore();
  const problem = useProblemStore(s => s.problems.find(p => p.id === problemId));
  const [focusedObligationId, setFocusedObligationId] = useState<string | null>(null);

  const isActiveAttempt = activeAttemptId === attemptId;

  // For active attempts, prefer live data but fall back to DB; for past attempts, use DB
  const liveSteps = steps.filter(s => s.attempt_id === attemptId);
  const attemptSteps = isActiveAttempt && liveSteps.length > 0 ? liveSteps : dbSteps;
  // Merge live obligations with DB obligations — live may have tallies, DB has full list
  const attemptObligations = (() => {
    if (!isActiveAttempt) return dbObligations;
    if (obligations.length === 0) return dbObligations;
    // Merge: use live data where available, fill gaps from DB
    const liveMap = new Map(obligations.map(o => [o.id, o]));
    const dbMap = new Map(dbObligations.map(o => [o.id, o]));
    const merged = new Map<string, typeof obligations[0]>();
    for (const [id, o] of liveMap) merged.set(id, o);
    for (const [id, o] of dbMap) {
      if (!merged.has(id)) merged.set(id, o);
    }
    return Array.from(merged.values());
  })();

  // Always load DB data on mount — separate from the interval effect so navigation
  // back to the same attempt still triggers a fresh load.
  useEffect(() => {
    loadClaims(attemptId);
    loadConflicts(attemptId);
    loadCurrentReport(attemptId);
    loadDbSteps(attemptId);
    loadDbObligations(attemptId);
  }, [attemptId]);

  // For active attempts, periodically refresh DB data
  useEffect(() => {
    if (!isActiveAttempt) return;
    const interval = setInterval(() => {
      loadDbSteps(attemptId);
      loadDbObligations(attemptId);
    }, 5000);
    return () => clearInterval(interval);
  }, [attemptId, isActiveAttempt]);

  // Clear stale data only when switching to a different attempt
  useEffect(() => {
    return () => { clearAttemptData(); };
  }, [attemptId]);

  // Set activeProblem for backward compat with LoopControls
  useEffect(() => {
    if (problem) {
      useProblemStore.getState().setActiveProblem(problem);
    }
  }, [problem]);

  return (
    <div className="attempt-detail">
      <div className="attempt-header">
        <h2>Attempt — {problem?.title || problem?.statement?.slice(0, 40) || problemId}</h2>
        {isActiveAttempt
          ? <span className={`status-badge status-${status}`}>{status}</span>
          : (() => { const a = attempts.find(a => a.id === attemptId); return a ? <span className={`status-badge status-${a.status}`}>{a.status}</span> : null; })()
        }
      </div>

      {/* Loop controls if this is the active attempt */}
      {isActiveAttempt && (
        <>
          <LoopControls />
          {status === 'running' && <ThinkingPanel focusObligationId={focusedObligationId} />}
        </>
      )}

      {/* Main two-column layout */}
      <div className="attempt-columns">
        <div className="attempt-left">
          <ObligationQueue obligations={attemptObligations} onSelect={(id) => goToObligation(problemId, attemptId, id)} />
        </div>
        <div className="attempt-right">
          {/* When obligations exist, show side-by-side lanes; otherwise plain proof chain */}
          {attemptObligations.length > 0 ? (
            <ObligationLanes
              steps={attemptSteps}
              obligations={attemptObligations}
              status={status}
              onStepClick={(stepId) => goToStep(problemId, attemptId, stepId)}
              focusedObligationId={focusedObligationId}
              onObligationFocus={setFocusedObligationId}
            />
          ) : (
            <ProofChain
              steps={attemptSteps}
              onStepClick={(stepId) => goToStep(problemId, attemptId, stepId)}
            />
          )}
        </div>
      </div>

      {/* Bottom panels */}
      <div className="attempt-bottom">
        <ClaimsPanel claims={claims} />
        <ConflictsPanel conflicts={conflicts} />
      </div>

      {/* Report summary if available */}
      {currentReport && (
        <div className="attempt-report-summary">
          <h3>After Action Report</h3>
          <div className="report-stats">
            {currentReport.coverage != null && (
              <span>Coverage: {(currentReport.coverage * 100).toFixed(0)}%</span>
            )}
            {currentReport.soundness != null && (
              <span>Soundness: {(currentReport.soundness * 100).toFixed(0)}%</span>
            )}
            <span>Obligations: {currentReport.obligations_closed}/{currentReport.obligations_total}</span>
            {currentReport.training_label && (
              <span>Label: {currentReport.training_label}</span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
