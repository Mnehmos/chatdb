import { useNavigationStore } from '../../stores/navigationStore';
import { useProblemStore } from '../../stores/problemStore';
import { stripLatex } from '../../utils/latex';

export function Breadcrumb() {
  const { level, problemId, attemptId, stepId, obligationId, goToLibrary, goToWorkspace, goToAttempt } = useNavigationStore();
  const { problems } = useProblemStore();

  if (level === 'library') return null;

  const problem = problems.find(p => p.id === problemId);
  const problemLabel = problem?.title || (problem?.statement ? stripLatex(problem.statement, 50) : '') || problemId || '';

  return (
    <nav className="breadcrumb">
      <button className="breadcrumb-link" onClick={goToLibrary}>Library</button>
      <span className="breadcrumb-sep">&gt;</span>
      <button
        className={`breadcrumb-link ${level === 'workspace' ? 'breadcrumb-active' : ''}`}
        onClick={() => problemId && goToWorkspace(problemId)}
      >
        {problemLabel}
      </button>
      {(level === 'attempt' || level === 'step' || level === 'obligation') && attemptId && (
        <>
          <span className="breadcrumb-sep">&gt;</span>
          <button
            className={`breadcrumb-link ${level === 'attempt' ? 'breadcrumb-active' : ''}`}
            onClick={() => problemId && attemptId && goToAttempt(problemId, attemptId)}
          >
            Attempt
          </button>
        </>
      )}
      {level === 'obligation' && obligationId && (
        <>
          <span className="breadcrumb-sep">&gt;</span>
          <span className="breadcrumb-active">Obligation</span>
        </>
      )}
      {level === 'step' && stepId && (
        <>
          <span className="breadcrumb-sep">&gt;</span>
          <span className="breadcrumb-active">Step</span>
        </>
      )}
    </nav>
  );
}
