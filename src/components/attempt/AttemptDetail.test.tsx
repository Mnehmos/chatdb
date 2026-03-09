import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Problem } from '../../types';

let loopStoreState: Record<string, unknown>;
let workspaceStoreState: Record<string, unknown>;
let navigationStoreState: Record<string, unknown>;
let problemStoreState: { problems: Problem[]; setActiveProblem: ReturnType<typeof vi.fn> };

const useLoopStoreMock = vi.fn(() => loopStoreState);
const useWorkspaceStoreMock = vi.fn(() => workspaceStoreState);
const useNavigationStoreMock = vi.fn(() => navigationStoreState);
const useProblemStoreMock = Object.assign(
  vi.fn((selector?: (state: { problems: Problem[]; setActiveProblem: ReturnType<typeof vi.fn> }) => unknown) =>
    selector ? selector(problemStoreState) : problemStoreState),
  {
    getState: vi.fn(() => problemStoreState),
  },
);

vi.mock('../../stores/loopStore', () => ({
  useLoopStore: useLoopStoreMock,
}));

vi.mock('../../stores/workspaceStore', () => ({
  useWorkspaceStore: useWorkspaceStoreMock,
}));

vi.mock('../../stores/navigationStore', () => ({
  useNavigationStore: useNavigationStoreMock,
}));

vi.mock('../../stores/problemStore', () => ({
  useProblemStore: useProblemStoreMock,
}));

vi.mock('../loop/LoopControls', () => ({
  LoopControls: () => <div data-testid="loop-controls" />,
}));

vi.mock('../loop/ThinkingPanel', () => ({
  ThinkingPanel: ({ focusObligationId }: { focusObligationId?: string | null }) => (
    <div data-testid="thinking-panel">{focusObligationId ?? 'none'}</div>
  ),
}));

vi.mock('./ObligationQueue', () => ({
  ObligationQueue: ({ obligations, onSelect }: { obligations: Array<{ id: string }>; onSelect: (id: string) => void }) => (
    <div data-testid="obligation-queue">
      <span>queue:{obligations.length}</span>
      {obligations[0] && (
        <button onClick={() => onSelect(obligations[0].id)}>select obligation</button>
      )}
    </div>
  ),
}));

vi.mock('./ObligationLanes', () => ({
  ObligationLanes: ({
    steps,
    obligations,
    status,
    onStepClick,
    onObligationFocus,
  }: {
    steps: Array<{ id: string }>;
    obligations: Array<{ id: string }>;
    status: string;
    onStepClick: (id: string) => void;
    onObligationFocus: (id: string) => void;
  }) => (
    <div data-testid="obligation-lanes">
      <span>{`${steps.length}/${obligations.length}/${status}`}</span>
      {steps[0] && <button onClick={() => onStepClick(steps[0].id)}>select step</button>}
      {obligations[0] && <button onClick={() => onObligationFocus(obligations[0].id)}>focus obligation</button>}
    </div>
  ),
}));

vi.mock('./ProofChain', () => ({
  ProofChain: ({ steps, onStepClick }: { steps: Array<{ id: string }>; onStepClick: (id: string) => void }) => (
    <div data-testid="proof-chain">
      <span>{`proof:${steps.length}`}</span>
      {steps[0] && <button onClick={() => onStepClick(steps[0].id)}>open proof step</button>}
    </div>
  ),
}));

vi.mock('./ClaimsPanel', () => ({
  ClaimsPanel: ({ claims }: { claims: unknown[] }) => <div data-testid="claims-panel">{claims.length}</div>,
}));

vi.mock('./ConflictsPanel', () => ({
  ConflictsPanel: ({ conflicts }: { conflicts: unknown[] }) => <div data-testid="conflicts-panel">{conflicts.length}</div>,
}));

function createProblem(): Problem {
  return {
    id: 'problem-1',
    title: 'Parity Puzzle',
    statement: 'Prove x^2 >= 0',
    domain: 'algebra',
    source: 'user',
    status: 'open',
    created_at: '2026-03-06T00:00:00Z',
    total_attempts: 0,
    total_steps: 0,
  };
}

describe('AttemptDetail', () => {
  beforeEach(() => {
    loopStoreState = {
      steps: [],
      obligations: [],
      status: 'idle',
      attemptId: null,
    };
    workspaceStoreState = {
      loadClaims: vi.fn(),
      loadConflicts: vi.fn(),
      claims: [],
      conflicts: [],
      clearAttemptData: vi.fn(),
      loadCurrentReport: vi.fn(),
      currentReport: null,
      dbSteps: [],
      loadDbSteps: vi.fn(),
      dbObligations: [],
      loadDbObligations: vi.fn(),
    };
    navigationStoreState = {
      goToStep: vi.fn(),
      goToObligation: vi.fn(),
    };
    problemStoreState = {
      problems: [createProblem()],
      setActiveProblem: vi.fn(),
    };

    useLoopStoreMock.mockImplementation(() => loopStoreState);
    useWorkspaceStoreMock.mockImplementation(() => workspaceStoreState);
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);
    useProblemStoreMock.mockImplementation((selector?: (state: typeof problemStoreState) => unknown) =>
      selector ? selector(problemStoreState) : problemStoreState,
    );
    useProblemStoreMock.getState.mockImplementation(() => problemStoreState);
  });

  it('shows live active-attempt data, wiring navigation and cleanup', async () => {
    loopStoreState = {
      steps: [
        { id: 'step-live', attempt_id: 'attempt-1' },
        { id: 'step-other', attempt_id: 'attempt-2' },
      ],
      obligations: [{ id: 'obl-1' }],
      status: 'running',
      attemptId: 'attempt-1',
    };
    workspaceStoreState = {
      ...workspaceStoreState,
      claims: [{ id: 'claim-1' }],
      conflicts: [{ id: 'conflict-1' }],
      currentReport: {
        coverage: 0.8,
        soundness: 0.9,
        obligations_closed: 4,
        obligations_total: 5,
        training_label: 'strong',
      },
    };
    useLoopStoreMock.mockImplementation(() => loopStoreState);
    useWorkspaceStoreMock.mockImplementation(() => workspaceStoreState);

    const { AttemptDetail } = await import('./AttemptDetail');

    const { unmount } = render(<AttemptDetail problemId="problem-1" attemptId="attempt-1" />);

    expect(workspaceStoreState.loadClaims).toHaveBeenCalledWith('attempt-1');
    expect(workspaceStoreState.loadConflicts).toHaveBeenCalledWith('attempt-1');
    expect(workspaceStoreState.loadCurrentReport).toHaveBeenCalledWith('attempt-1');
    expect(workspaceStoreState.loadDbSteps).toHaveBeenCalledWith('attempt-1');
    expect(workspaceStoreState.loadDbObligations).toHaveBeenCalledWith('attempt-1');
    expect(problemStoreState.setActiveProblem).toHaveBeenCalledWith(expect.objectContaining({ id: 'problem-1' }));

    expect(screen.getByTestId('loop-controls')).toBeInTheDocument();
    expect(screen.getByTestId('thinking-panel')).toHaveTextContent('none');
    expect(screen.getByTestId('obligation-lanes')).toHaveTextContent('1/1/running');
    expect(screen.getByTestId('claims-panel')).toHaveTextContent('1');
    expect(screen.getByTestId('conflicts-panel')).toHaveTextContent('1');
    expect(screen.getByText('Coverage: 80%')).toBeInTheDocument();
    expect(screen.getByText('Soundness: 90%')).toBeInTheDocument();
    expect(screen.getByText('Obligations: 4/5')).toBeInTheDocument();
    expect(screen.getByText('Label: strong')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'select step' }));
    fireEvent.click(screen.getByRole('button', { name: 'select obligation' }));
    fireEvent.click(screen.getByRole('button', { name: 'focus obligation' }));

    expect(navigationStoreState.goToStep).toHaveBeenCalledWith('problem-1', 'attempt-1', 'step-live');
    expect(navigationStoreState.goToObligation).toHaveBeenCalledWith('problem-1', 'attempt-1', 'obl-1');
    expect(screen.getByTestId('thinking-panel')).toHaveTextContent('obl-1');

    unmount();
    expect(workspaceStoreState.clearAttemptData).toHaveBeenCalled();
  });

  it('falls back to DB-loaded data for inactive attempts and omits live controls', async () => {
    loopStoreState = {
      steps: [{ id: 'live-other', attempt_id: 'attempt-2' }],
      obligations: [{ id: 'live-obl' }],
      status: 'paused',
      attemptId: 'attempt-2',
    };
    workspaceStoreState = {
      ...workspaceStoreState,
      dbSteps: [{ id: 'db-step' }],
      dbObligations: [],
    };
    useLoopStoreMock.mockImplementation(() => loopStoreState);
    useWorkspaceStoreMock.mockImplementation(() => workspaceStoreState);

    const { AttemptDetail } = await import('./AttemptDetail');

    render(<AttemptDetail problemId="problem-1" attemptId="attempt-1" />);

    expect(screen.queryByTestId('loop-controls')).not.toBeInTheDocument();
    expect(screen.queryByTestId('thinking-panel')).not.toBeInTheDocument();
    expect(screen.getByTestId('proof-chain')).toHaveTextContent('proof:1');

    fireEvent.click(screen.getByRole('button', { name: 'open proof step' }));
    expect(navigationStoreState.goToStep).toHaveBeenCalledWith('problem-1', 'attempt-1', 'db-step');
  });
});
