import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Problem } from '../../types';

let problemStoreState: { problems: Problem[] };
let navigationStoreState: {
  level: string;
  problemId: string | null;
  goToLibrary: ReturnType<typeof vi.fn>;
  goToWorkspace: ReturnType<typeof vi.fn>;
};

const useProblemStoreMock = vi.fn(() => problemStoreState);
const useNavigationStoreMock = vi.fn(() => navigationStoreState);

vi.mock('../../stores/problemStore', () => ({
  useProblemStore: useProblemStoreMock,
}));

vi.mock('../../stores/navigationStore', () => ({
  useNavigationStore: useNavigationStoreMock,
}));

vi.mock('../../utils/latex', () => ({
  stripLatex: (text: string) => text,
}));

function createProblem(id: string, statement: string): Problem {
  return {
    id,
    statement,
    domain: 'algebra',
    source: 'user',
    status: 'open',
    created_at: '2026-03-06T00:00:00Z',
    total_attempts: 0,
    total_steps: 3,
  };
}

describe('Sidebar', () => {
  beforeEach(() => {
    problemStoreState = { problems: [] };
    navigationStoreState = {
      level: 'library',
      problemId: null,
      goToLibrary: vi.fn(),
      goToWorkspace: vi.fn(),
    };

    useProblemStoreMock.mockImplementation(() => problemStoreState);
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);
  });

  it('shows an empty-state message when there are no problems yet', async () => {
    const { Sidebar } = await import('./Sidebar');

    render(<Sidebar />);

    expect(screen.getByText('No problems yet')).toBeInTheDocument();
  });

  it('navigates to a problem workspace when a problem card is clicked', async () => {
    problemStoreState = {
      problems: [createProblem('problem-1', 'Prove x^2 >= 0')],
    };
    navigationStoreState = {
      ...navigationStoreState,
      level: 'workspace',
    };
    useProblemStoreMock.mockImplementation(() => problemStoreState);
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);

    const { Sidebar } = await import('./Sidebar');

    render(<Sidebar />);

    fireEvent.click(screen.getByText('Prove x^2 >= 0'));

    expect(screen.getByRole('button', { name: 'Library' })).toBeInTheDocument();
    expect(navigationStoreState.goToWorkspace).toHaveBeenCalledWith('problem-1');
  });
});
