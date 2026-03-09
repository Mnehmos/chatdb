import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let loopStoreState: { status: string };
let problemStoreState: { activeProblem: { id: string } | null };

const useLoopStoreMock = vi.fn((selector?: (state: typeof loopStoreState) => unknown) =>
  selector ? selector(loopStoreState) : loopStoreState,
);
const useProblemStoreMock = vi.fn(() => problemStoreState);

vi.mock('../../stores/loopStore', () => ({
  useLoopStore: useLoopStoreMock,
}));

vi.mock('../../stores/problemStore', () => ({
  useProblemStore: useProblemStoreMock,
}));

vi.mock('../../services/tauri', () => ({
  listAllSteps: vi.fn(),
}));

describe('TrainingDataTable', () => {
  beforeEach(async () => {
    loopStoreState = { status: 'idle' };
    problemStoreState = { activeProblem: { id: 'problem-1' } };
    useLoopStoreMock.mockImplementation((selector?: (state: typeof loopStoreState) => unknown) =>
      selector ? selector(loopStoreState) : loopStoreState,
    );
    useProblemStoreMock.mockImplementation(() => problemStoreState);
    const api = await import('../../services/tauri');
    vi.mocked(api.listAllSteps).mockResolvedValue([
      {
        id: 'row-1',
        problem_id: 'problem-1',
        problem_statement: 'Problem statement 1',
        problem_domain: 'algebra',
        step_number: 1,
        proposal_type: 'lemma',
        proposal_natural: 'First natural',
        proposal_formal: 'x**2',
        model: 'solver-a',
        verified: true,
        sympy_passed: true,
        pint_passed: null,
        lean_passed: null,
        created_at: '2026-03-06T00:00:00Z',
      },
      {
        id: 'row-2',
        problem_id: 'problem-2',
        problem_statement: 'Problem statement 2',
        problem_domain: 'number_theory',
        step_number: 2,
        proposal_type: 'conclusion',
        proposal_natural: 'Second natural',
        proposal_formal: 'y**10',
        model: 'solver-b',
        verified: false,
        sympy_passed: false,
        pint_passed: null,
        lean_passed: true,
        rejection_reason: 'Counterexample exists.',
        created_at: '2026-03-06T01:00:00Z',
      },
      {
        id: 'row-3',
        problem_id: 'problem-1',
        problem_statement: 'Problem statement 1',
        problem_domain: 'algebra',
        step_number: 3,
        proposal_type: 'lemma',
        proposal_natural: 'Repeated natural',
        model: 'solver-a',
        verified: true,
        sympy_passed: true,
        pint_passed: null,
        lean_passed: null,
        semantic_redundant: true,
        created_at: '2026-03-06T02:00:00Z',
      },
    ] as any);
  });

  it('loads rows, filters by scope and verdict, and expands details', async () => {
    const { TrainingDataTable } = await import('./TrainingDataTable');
    const api = await import('../../services/tauri');

    render(<TrainingDataTable />);

    await waitFor(() => {
      expect(api.listAllSteps).toHaveBeenCalledWith(200);
    });

    expect(screen.getByText('Raw Training Data')).toBeInTheDocument();
    expect(screen.getByText('First natural')).toBeInTheDocument();
    expect(screen.queryByText('Second natural')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'All Problems' }));

    expect(screen.getByText('Second natural')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Rejected (1)' }));
    fireEvent.click(screen.getByText('Second natural'));

    expect(screen.queryByText('First natural')).not.toBeInTheDocument();
    expect(screen.getByText('Problem statement 2')).toBeInTheDocument();
    expect(screen.getByText('Counterexample exists.')).toBeInTheDocument();
    expect(screen.getByText('number_theory')).toBeInTheDocument();
  });

  it('shows semantic redundancy labels separately from stale sibling fan-in state', async () => {
    const { TrainingDataTable } = await import('./TrainingDataTable');

    render(<TrainingDataTable />);

    await screen.findByText('Repeated natural');
    fireEvent.click(screen.getByText('Repeated natural'));

    expect(screen.getByText('Semantically redundant with an earlier verified step on the same obligation.')).toBeInTheDocument();
  });
});
