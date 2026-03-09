import { useEffect } from 'react';
import { ProblemLibrary } from './components/library/ProblemLibrary';
import { AttemptDetail } from './components/attempt/AttemptDetail';
import { DiagnosticPanel } from './components/diagnostics/DiagnosticPanel';
import { ObligationDetailView } from './components/obligation/ObligationDetailView';
import { ProblemWorkspace } from './components/workspace/ProblemWorkspace';
import { Breadcrumb } from './components/shared/Breadcrumb';
import { Header } from './components/shared/Header';
import { Sidebar } from './components/shared/Sidebar';
import { StepDetailView } from './components/step/StepDetailView';
import { handleLoopEvent } from './loopEvents';
import { onLoopEvent } from './services/events';
import { useLoopStore } from './stores/loopStore';
import { useNavigationStore } from './stores/navigationStore';
import { useProblemStore } from './stores/problemStore';

export default function App() {
  const { fetchProblems } = useProblemStore();
  const { level, problemId, attemptId, stepId, obligationId } = useNavigationStore();

  useEffect(() => {
    fetchProblems();
    useLoopStore.getState().loadProfiles();
  }, []);

  useEffect(() => {
    if (problemId) {
      useLoopStore.getState().loadSteps(problemId);
    } else {
      useLoopStore.setState({ steps: [], currentStep: 0, status: 'idle' });
    }
  }, [problemId]);

  useEffect(() => {
    const unlisten = onLoopEvent(handleLoopEvent);
    return unlisten;
  }, []);

  return (
    <div className="app">
      <Header />
      <Breadcrumb />
      <div className="app-layout">
        <Sidebar />
        <main className="main-content">
          {level === 'library' && <ProblemLibrary />}
          {level === 'workspace' && problemId && <ProblemWorkspace problemId={problemId} />}
          {level === 'attempt' && problemId && attemptId && (
            <AttemptDetail problemId={problemId} attemptId={attemptId} />
          )}
          {level === 'obligation' && problemId && attemptId && obligationId && (
            <ObligationDetailView obligationId={obligationId} />
          )}
          {level === 'step' && problemId && attemptId && stepId && (
            <StepDetailView problemId={problemId} attemptId={attemptId} stepId={stepId} />
          )}
          <DiagnosticPanel />
        </main>
      </div>
    </div>
  );
}
