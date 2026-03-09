import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Problem } from '../../types';

let problemStoreState: { problems: Problem[] };
let navigationStoreState: Record<string, unknown>;

const useProblemStoreMock = vi.fn(() => problemStoreState);
const useNavigationStoreMock = vi.fn(() => navigationStoreState);

vi.mock('../../stores/problemStore', () => ({
  useProblemStore: useProblemStoreMock,
}));

vi.mock('../../stores/navigationStore', () => ({
  useNavigationStore: useNavigationStoreMock,
}));

vi.mock('../../utils/latex', () => ({
  stripLatex: (text: string) => `plain:${text}`,
}));

function createProblem(): Problem {
  return {
    id: 'problem-1',
    title: 'Parity Puzzle',
    statement: '$x^2 \\ge 0$',
    domain: 'algebra',
    source: 'user',
    status: 'open',
    created_at: '2026-03-06T00:00:00Z',
    total_attempts: 0,
    total_steps: 0,
  };
}

describe('Breadcrumb', () => {
  beforeEach(() => {
    problemStoreState = { problems: [createProblem()] };
    navigationStoreState = {
      level: 'library',
      problemId: null,
      attemptId: null,
      stepId: null,
      obligationId: null,
      goToLibrary: vi.fn(),
      goToWorkspace: vi.fn(),
      goToAttempt: vi.fn(),
    };

    useProblemStoreMock.mockImplementation(() => problemStoreState);
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);
  });

  it('renders nothing at the library level', async () => {
    const { Breadcrumb } = await import('./Breadcrumb');

    const { container } = render(<Breadcrumb />);

    expect(container).toBeEmptyDOMElement();
  });

  it('renders problem and attempt crumbs and wires the navigation callbacks', async () => {
    navigationStoreState = {
      ...navigationStoreState,
      level: 'attempt',
      problemId: 'problem-1',
      attemptId: 'attempt-7',
    };
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);

    const { Breadcrumb } = await import('./Breadcrumb');

    render(<Breadcrumb />);

    fireEvent.click(screen.getByRole('button', { name: 'Library' }));
    fireEvent.click(screen.getByRole('button', { name: 'Parity Puzzle' }));
    fireEvent.click(screen.getByRole('button', { name: 'Attempt' }));

    expect(screen.getByRole('button', { name: 'Parity Puzzle' })).toBeInTheDocument();
    expect(navigationStoreState.goToLibrary).toHaveBeenCalled();
    expect(navigationStoreState.goToWorkspace).toHaveBeenCalledWith('problem-1');
    expect(navigationStoreState.goToAttempt).toHaveBeenCalledWith('problem-1', 'attempt-7');
  });
});
