import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { AppLoopEvent } from './types';

let problemStoreState: Record<string, unknown>;
let loopStoreState: Record<string, any>;
let workspaceStoreState: Record<string, unknown>;
let agentStoreState: Record<string, unknown>;
let diagnosticStoreState: Record<string, unknown>;
let navigationStoreState: Record<string, unknown>;
let loopEventHandler: ((event: AppLoopEvent) => void) | null;
let unlisten: ReturnType<typeof vi.fn>;

const useProblemStoreMock = Object.assign(
  vi.fn(() => problemStoreState),
  { getState: vi.fn(() => problemStoreState) },
);
const useLoopStoreMock = Object.assign(
  vi.fn((selector?: (state: typeof loopStoreState) => unknown) =>
    selector ? selector(loopStoreState) : loopStoreState),
  {
    getState: vi.fn(() => loopStoreState),
    setState: vi.fn(),
  },
);
const useWorkspaceStoreMock = Object.assign(
  vi.fn(() => workspaceStoreState),
  { getState: vi.fn(() => workspaceStoreState) },
);
const useAgentStoreMock = Object.assign(
  vi.fn(() => agentStoreState),
  { getState: vi.fn(() => agentStoreState) },
);
const useDiagnosticStoreMock = Object.assign(
  vi.fn(() => diagnosticStoreState),
  { getState: vi.fn(() => diagnosticStoreState) },
);
const useNavigationStoreMock = Object.assign(
  vi.fn(() => navigationStoreState),
  { getState: vi.fn(() => navigationStoreState) },
);

vi.mock('./stores/problemStore', () => ({
  useProblemStore: useProblemStoreMock,
}));

vi.mock('./stores/loopStore', () => ({
  useLoopStore: useLoopStoreMock,
}));

vi.mock('./stores/workspaceStore', () => ({
  useWorkspaceStore: useWorkspaceStoreMock,
}));

vi.mock('./stores/agentStore', () => ({
  useAgentStore: useAgentStoreMock,
}));

vi.mock('./stores/diagnosticStore', () => ({
  useDiagnosticStore: useDiagnosticStoreMock,
}));

vi.mock('./stores/navigationStore', () => ({
  useNavigationStore: useNavigationStoreMock,
}));

vi.mock('./services/events', () => ({
  onLoopEvent: vi.fn((callback: (event: AppLoopEvent) => void) => {
    loopEventHandler = callback;
    return unlisten;
  }),
}));

vi.mock('./components/library/ProblemLibrary', () => ({
  ProblemLibrary: () => <div data-testid="problem-library" />,
}));

vi.mock('./components/workspace/ProblemWorkspace', () => ({
  ProblemWorkspace: ({ problemId }: { problemId: string }) => (
    <div data-testid="problem-workspace">{problemId}</div>
  ),
}));

vi.mock('./components/attempt/AttemptDetail', () => ({
  AttemptDetail: ({ attemptId }: { attemptId: string }) => (
    <div data-testid="attempt-detail">{attemptId}</div>
  ),
}));

vi.mock('./components/step/StepDetailView', () => ({
  StepDetailView: ({ stepId }: { stepId: string }) => (
    <div data-testid="step-detail">{stepId}</div>
  ),
}));

vi.mock('./components/obligation/ObligationDetailView', () => ({
  ObligationDetailView: ({ obligationId }: { obligationId: string }) => (
    <div data-testid="obligation-detail">{obligationId}</div>
  ),
}));

vi.mock('./components/diagnostics/DiagnosticPanel', () => ({
  DiagnosticPanel: () => <div data-testid="diagnostic-panel" />,
}));

vi.mock('./components/shared/Sidebar', () => ({
  Sidebar: () => <div data-testid="sidebar" />,
}));

vi.mock('./components/shared/Header', () => ({
  Header: () => <div data-testid="header" />,
}));

vi.mock('./components/shared/Breadcrumb', () => ({
  Breadcrumb: () => <div data-testid="breadcrumb" />,
}));

describe('App', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(console, 'log').mockImplementation(() => {});
    vi.spyOn(console, 'warn').mockImplementation(() => {});

    problemStoreState = {
      fetchProblems: vi.fn(),
    };
    loopStoreState = {
      loadProfiles: vi.fn(),
      loadSteps: vi.fn(),
      addStep: vi.fn(),
      setStatus: vi.fn(),
      setAttemptInfo: vi.fn(),
      resetForNewAttempt: vi.fn(),
      addAudit: vi.fn(),
      setReviewing: vi.fn(),
      reviewResult: null,
      setReview: vi.fn(),
      setWarmupStatus: vi.fn(),
      addObligation: vi.fn(),
      updateObligation: vi.fn(),
      obligations: [],
      addCriticCheck: vi.fn(),
      setExtractedPatterns: vi.fn(),
      addDiscernerFinding: vi.fn(),
    };
    workspaceStoreState = {
      loadAttempts: vi.fn(),
      loadReports: vi.fn(),
    };
    agentStoreState = {
      addOrchestratorEvent: vi.fn(),
      addCriticEvent: vi.fn(),
      addCouncilSession: vi.fn(),
      addScoutResult: vi.fn(),
    };
    diagnosticStoreState = {
      addEvent: vi.fn(),
      setOpen: vi.fn(),
    };
    navigationStoreState = {
      level: 'workspace',
      problemId: 'problem-1',
      attemptId: null,
      stepId: null,
      obligationId: null,
    };
    loopEventHandler = null;
    unlisten = vi.fn();

    useProblemStoreMock.mockImplementation(() => problemStoreState);
    useProblemStoreMock.getState.mockImplementation(() => problemStoreState);
    useLoopStoreMock.mockImplementation((selector?: (state: typeof loopStoreState) => unknown) =>
      selector ? selector(loopStoreState) : loopStoreState,
    );
    useLoopStoreMock.getState.mockImplementation(() => loopStoreState);
    useWorkspaceStoreMock.mockImplementation(() => workspaceStoreState);
    useWorkspaceStoreMock.getState.mockImplementation(() => workspaceStoreState);
    useAgentStoreMock.mockImplementation(() => agentStoreState);
    useAgentStoreMock.getState.mockImplementation(() => agentStoreState);
    useDiagnosticStoreMock.mockImplementation(() => diagnosticStoreState);
    useDiagnosticStoreMock.getState.mockImplementation(() => diagnosticStoreState);
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);
    useNavigationStoreMock.getState.mockImplementation(() => navigationStoreState);
  });

  it('loads initial data, renders the current route, and cleans up listeners', async () => {
    const { default: App } = await import('./App');
    const { rerender, unmount } = render(<App />);

    expect(problemStoreState.fetchProblems).toHaveBeenCalled();
    expect(loopStoreState.loadProfiles).toHaveBeenCalled();
    expect(loopStoreState.loadSteps).toHaveBeenCalledWith('problem-1');
    expect(screen.getByTestId('problem-workspace')).toHaveTextContent('problem-1');
    expect(screen.getByTestId('diagnostic-panel')).toBeInTheDocument();

    navigationStoreState = {
      level: 'library',
      problemId: null,
      attemptId: null,
      stepId: null,
      obligationId: null,
    };
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);
    useNavigationStoreMock.getState.mockImplementation(() => navigationStoreState);

    rerender(<App />);

    expect(useLoopStoreMock.setState).toHaveBeenCalledWith({
      steps: [],
      currentStep: 0,
      status: 'idle',
    });
    expect(screen.getByTestId('problem-library')).toBeInTheDocument();

    unmount();

    expect(unlisten).toHaveBeenCalled();
  });

  it('routes loop and agent events into the stores, including fan-in events', async () => {
    const { default: App } = await import('./App');
    render(<App />);

    loopStoreState.obligations = [
      {
        id: 'ob-1',
        signals: [{ source: 'solver', satisfies: false, note: 'stale' }],
      },
    ];

    loopEventHandler?.({
      type: 'loop:step_complete',
      payload: {
        step_number: 7,
        attempt_id: 'attempt-9',
        model: 'solver-x',
        proposal_type: 'lemma',
        proposal_natural: 'Fresh step',
        verified: true,
      },
    });
    loopEventHandler?.({
      type: 'loop:attempt_start',
      payload: {
        attempt_number: 2,
        max_attempts: 5,
        attempt_id: 'attempt-2',
      },
    });
    loopEventHandler?.({
      type: 'loop:satisfaction_signal',
      payload: {
        obligation_id: 'ob-1',
        tally_yes: 1,
        tally_total: 1,
        source: 'reviewer',
        satisfies: true,
        note: 'fresh round',
      },
    });
    loopEventHandler?.({
      type: 'loop:fanin_round_start',
      payload: {
        attempt_id: 'attempt-2',
        obligation_id: 'ob-1',
        obligation_desc: 'Need contradiction',
        worker_count: 2,
        worker_models: ['solver-a', 'solver-b'],
        solver_round_id: 'round-1',
        reserved_step_numbers: [8, 9],
      },
    });
    loopEventHandler?.({
      type: 'loop:fanin_round_complete',
      payload: {
        solver_round_id: 'round-1',
        results_processed: 2,
        results_skipped_stale: 0,
      },
    });
    loopEventHandler?.({
      type: 'agent:critic_evaluation',
      payload: {
        obligation_id: 'ob-1',
        check_description: 'Try x = 0',
        likely_wrong: true,
      },
    });
    loopEventHandler?.({
      type: 'loop:error',
      payload: { message: 'Loop broke' },
    });

    expect(loopStoreState.addStep).toHaveBeenCalledWith(
      expect.objectContaining({
        attempt_id: 'attempt-9',
        step_number: 7,
        proposal_natural: 'Fresh step',
      }),
    );
    expect(loopStoreState.setAttemptInfo).toHaveBeenCalledWith(2, 5, 'attempt-2');
    expect(workspaceStoreState.loadAttempts).toHaveBeenCalledWith('problem-1');
    expect(loopStoreState.updateObligation).toHaveBeenCalledWith(
      'ob-1',
      expect.objectContaining({
        tally_yes: 1,
        tally_total: 1,
        signals: [{ source: 'reviewer', satisfies: true, note: 'fresh round' }],
      }),
    );
    expect(loopStoreState.updateObligation).toHaveBeenCalledWith(
      'ob-1',
      expect.objectContaining({
        dispatch_mode: 'parallel_fanin',
        assigned_models: ['solver-a', 'solver-b'],
        active_solver_round_id: 'round-1',
      }),
    );
    expect(agentStoreState.addOrchestratorEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'fanin_start',
        obligation_id: 'ob-1',
        worker_count: 2,
      }),
    );
    expect(agentStoreState.addOrchestratorEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'fanin_complete',
        results_processed: 2,
      }),
    );
    expect(agentStoreState.addCriticEvent).toHaveBeenCalledWith({
      obligation_id: 'ob-1',
      check_description: 'Try x = 0',
      likely_wrong: true,
    });
    expect(diagnosticStoreState.addEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        category: 'mechanical',
        severity: 'fatal',
        message: 'Loop broke',
      }),
    );
    expect(diagnosticStoreState.setOpen).toHaveBeenCalledWith(true);
    expect(loopStoreState.setStatus).toHaveBeenCalledWith('idle');
  });
});
