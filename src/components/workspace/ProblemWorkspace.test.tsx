import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { AttemptSummary, AfterActionReportRecord, Problem } from '../../types';

let problemStoreState: Record<string, unknown>;
let workspaceStoreState: Record<string, unknown>;
let navigationStoreState: Record<string, unknown>;
let loopStoreState: Record<string, unknown>;

const useProblemStoreMock = Object.assign(vi.fn(() => problemStoreState), {
  getState: vi.fn(() => problemStoreState),
});
const useWorkspaceStoreMock = vi.fn(() => workspaceStoreState);
const useNavigationStoreMock = vi.fn(() => navigationStoreState);
const useLoopStoreMock = Object.assign(vi.fn(() => loopStoreState), {
  getState: vi.fn(() => loopStoreState),
});

vi.mock('../../stores/problemStore', () => ({
  useProblemStore: useProblemStoreMock,
}));

vi.mock('../../stores/workspaceStore', () => ({
  useWorkspaceStore: useWorkspaceStoreMock,
}));

vi.mock('../../stores/navigationStore', () => ({
  useNavigationStore: useNavigationStoreMock,
}));

vi.mock('../../stores/loopStore', () => ({
  useLoopStore: useLoopStoreMock,
}));

vi.mock('../../services/tauri', () => ({
  updateProblemTitle: vi.fn(),
}));

vi.mock('../../utils/latex', () => ({
  renderLatexText: (text: string) => text,
}));

vi.mock('../loop/LoopControls', () => ({
  LoopControls: () => <div data-testid="loop-controls" />,
}));

vi.mock('../loop/ThinkingPanel', () => ({
  ThinkingPanel: () => <div data-testid="thinking-panel" />,
}));

vi.mock('../export/ExportPanel', () => ({
  ExportPanel: ({ problemId }: { problemId: string }) => <div data-testid="export-panel">{problemId}</div>,
}));

function createProblem(overrides: Partial<Problem> = {}): Problem {
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
    ...overrides,
  };
}

function createAttempt(overrides: Partial<AttemptSummary> = {}): AttemptSummary {
  return {
    id: 'attempt-1',
    problem_id: 'problem-1',
    attempt_number: 1,
    status: 'paused',
    step_count: 4,
    steps_verified: 3,
    steps_rejected: 1,
    started_at: '2026-03-06T00:00:00Z',
    ...overrides,
  };
}

function createReport(overrides: Partial<AfterActionReportRecord> = {}): AfterActionReportRecord {
  return {
    id: 'report-1',
    attempt_id: 'attempt-1',
    problem_id: 'problem-1',
    coverage: 0.75,
    death_spirals: 1,
    contradictions: 2,
    obligations_total: 5,
    obligations_closed: 4,
    created_at: '2026-03-06T00:00:00Z',
    ...overrides,
  };
}

describe('ProblemWorkspace', () => {
  beforeEach(() => {
    problemStoreState = {
      problems: [createProblem()],
      fetchProblems: vi.fn(),
      setActiveProblem: vi.fn(),
    };
    workspaceStoreState = {
      attempts: [createAttempt()],
      attemptsLoading: false,
      loadAttempts: vi.fn(),
      reports: [createReport()],
      loadReports: vi.fn(),
      deleteAttempt: vi.fn(),
    };
    navigationStoreState = {
      goToAttempt: vi.fn(),
    };
    loopStoreState = {
      status: 'idle',
      currentStep: 0,
      continueSolve: vi.fn(),
    };

    useProblemStoreMock.mockImplementation(() => problemStoreState);
    useProblemStoreMock.getState.mockImplementation(() => problemStoreState);
    useWorkspaceStoreMock.mockImplementation(() => workspaceStoreState);
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);
    useLoopStoreMock.mockImplementation(() => loopStoreState);
    useLoopStoreMock.getState.mockImplementation(() => loopStoreState);
  });

  it('shows an empty state when the requested problem does not exist', async () => {
    problemStoreState = {
      ...problemStoreState,
      problems: [],
    };
    useProblemStoreMock.mockImplementation(() => problemStoreState);
    useProblemStoreMock.getState.mockImplementation(() => problemStoreState);

    const { ProblemWorkspace } = await import('./ProblemWorkspace');

    render(<ProblemWorkspace problemId="missing-problem" />);

    expect(screen.getByText('Problem not found')).toBeInTheDocument();
  });

  it('loads workspace data and resumes paused attempts without navigating away', async () => {
    const { ProblemWorkspace } = await import('./ProblemWorkspace');

    render(<ProblemWorkspace problemId="problem-1" />);

    expect(workspaceStoreState.loadAttempts).toHaveBeenCalledWith('problem-1');
    expect(workspaceStoreState.loadReports).toHaveBeenCalledWith('problem-1');
    expect(problemStoreState.setActiveProblem).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'problem-1' }),
    );
    expect(screen.getByText('Best Coverage: 75%')).toBeInTheDocument();
    expect(screen.getByTestId('loop-controls')).toBeInTheDocument();
    expect(screen.getByTestId('export-panel')).toHaveTextContent('problem-1');

    fireEvent.click(screen.getByRole('button', { name: 'Resume' }));

    expect(loopStoreState.continueSolve).toHaveBeenCalledWith('problem-1', 'attempt-1');
    expect(navigationStoreState.goToAttempt).not.toHaveBeenCalled();
  });
});
