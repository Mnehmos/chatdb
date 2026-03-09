import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Problem } from '../../types';

let problemStoreState: { problems: Problem[] };
let navigationStoreState: { goToWorkspace: ReturnType<typeof vi.fn> };

const useProblemStoreMock = vi.fn(() => problemStoreState);
const useNavigationStoreMock = vi.fn(() => navigationStoreState);

vi.mock('../../stores/problemStore', () => ({
  useProblemStore: useProblemStoreMock,
}));

vi.mock('../../stores/navigationStore', () => ({
  useNavigationStore: useNavigationStoreMock,
}));

vi.mock('../problem/ProblemInput', () => ({
  ProblemInput: () => <div data-testid="problem-input" />,
}));

vi.mock('../export/ExportPanel', () => ({
  ExportPanel: () => <div data-testid="export-panel" />,
}));

vi.mock('../../utils/latex', () => ({
  stripLatex: (text: string) => `preview:${text}`,
}));

function createProblem(id: string, overrides: Partial<Problem> = {}): Problem {
  return {
    id,
    statement: `Statement ${id}`,
    domain: 'algebra',
    source: 'user',
    status: 'open',
    created_at: '2026-03-06T00:00:00Z',
    total_attempts: 1,
    total_steps: 3,
    ...overrides,
  };
}

describe('ProblemLibrary', () => {
  beforeEach(() => {
    problemStoreState = {
      problems: [
        createProblem('problem-1', { title: 'Parity Puzzle', domain: 'number_theory' }),
        createProblem('problem-2', { status: 'solved', source: 'book', domain: 'algebra' }),
      ],
    };
    navigationStoreState = {
      goToWorkspace: vi.fn(),
    };
    useProblemStoreMock.mockImplementation(() => problemStoreState);
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);
  });

  it('filters the library and opens a workspace when a row is selected', async () => {
    const { ProblemLibrary } = await import('./ProblemLibrary');

    render(<ProblemLibrary />);

    fireEvent.change(screen.getByPlaceholderText('Search problems...'), {
      target: { value: 'Parity' },
    });
    fireEvent.change(screen.getByDisplayValue('All Status'), {
      target: { value: 'open' },
    });
    fireEvent.change(screen.getByDisplayValue('All Domains'), {
      target: { value: 'number_theory' },
    });

    expect(screen.getByText('Parity Puzzle')).toBeInTheDocument();
    expect(screen.queryByText('preview:Statement problem-2')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('Parity Puzzle'));

    expect(navigationStoreState.goToWorkspace).toHaveBeenCalledWith('problem-1');
  });

  it('toggles the new-problem form and shows the export panel', async () => {
    const { ProblemLibrary } = await import('./ProblemLibrary');

    render(<ProblemLibrary />);

    fireEvent.click(screen.getByRole('button', { name: '+ New Problem' }));

    expect(screen.getByTestId('problem-input')).toBeInTheDocument();
    expect(screen.getByTestId('export-panel')).toBeInTheDocument();
  });
});
