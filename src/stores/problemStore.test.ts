import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useProblemStore } from './problemStore';

vi.mock('../services/tauri', () => ({
  listProblems: vi.fn(),
  createProblem: vi.fn(),
}));

describe('problemStore', () => {
  beforeEach(() => {
    useProblemStore.setState({
      problems: [],
      activeProblem: null,
      loading: false,
      error: null,
    });
  });

  it('fetchProblems stores the returned problems and clears loading', async () => {
    const api = await import('../services/tauri');
    vi.mocked(api.listProblems).mockResolvedValue([
      {
        id: 'problem-1',
        statement: 'Prove x^2 >= 0',
        domain: 'algebra',
        source: 'user',
        status: 'open',
        created_at: '2026-03-06T00:00:00Z',
        total_attempts: 0,
        total_steps: 0,
      },
    ]);

    await useProblemStore.getState().fetchProblems();

    const state = useProblemStore.getState();
    expect(state.loading).toBe(false);
    expect(state.problems).toHaveLength(1);
    expect(state.problems[0]?.id).toBe('problem-1');
    expect(state.error).toBeNull();
  });

  it('createProblem prepends the new problem and sets it active', async () => {
    const api = await import('../services/tauri');
    vi.mocked(api.createProblem).mockResolvedValue({
      id: 'problem-2',
      statement: 'Show 2 + 2 = 4',
      domain: 'arithmetic',
      source: 'user',
      status: 'open',
      created_at: '2026-03-06T00:00:00Z',
      total_attempts: 0,
      total_steps: 0,
    });

    const created = await useProblemStore
      .getState()
      .createProblem('Show 2 + 2 = 4', 'arithmetic');

    const state = useProblemStore.getState();
    expect(created.id).toBe('problem-2');
    expect(state.activeProblem?.id).toBe('problem-2');
    expect(state.problems[0]?.id).toBe('problem-2');
  });
});
