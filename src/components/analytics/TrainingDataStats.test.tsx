import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let loopStoreState: Record<string, unknown>;

const useLoopStoreMock = vi.fn((selector?: (state: Record<string, unknown>) => unknown) =>
  selector ? selector(loopStoreState) : loopStoreState);

vi.mock('../../stores/loopStore', () => ({
  useLoopStore: useLoopStoreMock,
}));

vi.mock('../../services/tauri', () => ({
  getTrainingDataStats: vi.fn(),
}));

describe('TrainingDataStats', () => {
  beforeEach(() => {
    loopStoreState = { status: 'idle' };
    useLoopStoreMock.mockImplementation((selector?: (state: Record<string, unknown>) => unknown) =>
      selector ? selector(loopStoreState) : loopStoreState,
    );
  });

  it('renders aggregate training data counts when stats are available', async () => {
    const api = await import('../../services/tauri');
    vi.mocked(api.getTrainingDataStats).mockResolvedValue({
      total_steps: 10,
      verified_steps: 6,
      rejected_steps: 4,
      contrastive_pairs: 2,
      orchestrator_decisions: 3,
      council_sessions: 1,
      council_findings: 4,
      critic_evaluations: 5,
      scout_queries: 6,
      librarian_actions: 7,
    } as any);

    const { TrainingDataStats } = await import('./TrainingDataStats');

    render(<TrainingDataStats />);

    await waitFor(() => {
      expect(screen.getByText('Training Data Generated')).toBeInTheDocument();
    });

    expect(screen.getByText('37')).toBeInTheDocument();
    expect(screen.getByText('Steps')).toBeInTheDocument();
    expect(screen.getByText('Contrastive')).toBeInTheDocument();
    expect(screen.getByText('Librarian')).toBeInTheDocument();
  });
});
