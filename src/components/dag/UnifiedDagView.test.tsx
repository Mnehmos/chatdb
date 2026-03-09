import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

let loopStoreState: Record<string, unknown>;
let navigationStoreState: Record<string, unknown>;

const useLoopStoreMock = vi.fn((selector?: (state: typeof loopStoreState) => unknown) =>
  selector ? selector(loopStoreState) : loopStoreState,
);
const useNavigationStoreMock = vi.fn(() => navigationStoreState);

vi.mock('../../stores/loopStore', () => ({
  useLoopStore: useLoopStoreMock,
}));

vi.mock('../../stores/navigationStore', () => ({
  useNavigationStore: useNavigationStoreMock,
}));

vi.mock('../../services/tauri', () => ({
  getDagEdgesFrom: vi.fn(),
}));

describe('UnifiedDagView', () => {
  beforeEach(async () => {
    loopStoreState = {
      obligations: [],
      steps: [
        {
          id: 'step-1',
          attempt_id: 'attempt-1',
          step_number: 1,
          proposal_type: 'lemma',
          proposal_natural: 'First step',
          verified: true,
        },
        {
          id: 'step-2',
          attempt_id: 'attempt-2',
          step_number: 2,
          proposal_type: 'conclusion',
          proposal_natural: 'Other attempt',
          verified: false,
        },
      ],
    };
    navigationStoreState = {
      goToStep: vi.fn(),
    };
    useLoopStoreMock.mockImplementation((selector?: (state: typeof loopStoreState) => unknown) =>
      selector ? selector(loopStoreState) : loopStoreState,
    );
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);

    const { invoke } = await import('@tauri-apps/api/core');
    const api = await import('../../services/tauri');

    vi.mocked(invoke).mockResolvedValue([
      {
        id: 'ob-1',
        description: 'Count roots',
        obligation_type: 'COUNT',
        priority: 0.9,
        confidence: 0.8,
        status: 'open',
        depends_on: null,
        steps_spent: 1,
        max_steps: 5,
        superseded_by: null,
      },
      {
        id: 'ob-2',
        description: 'Resolve parity',
        obligation_type: 'RESOLVE',
        priority: 0.4,
        confidence: 0.7,
        status: 'closed_proved',
        depends_on: '["ob-1"]',
        steps_spent: 0,
        max_steps: 3,
        superseded_by: null,
      },
    ] as any);
    vi.mocked(api.getDagEdgesFrom).mockResolvedValue([
      { source_id: 'ob-1', target_id: 'step-1', edge_type: 'targets' },
    ] as any);
  });

  it('loads obligation and step graph data and navigates from step nodes', async () => {
    const { UnifiedDagView } = await import('./UnifiedDagView');
    const { invoke } = await import('@tauri-apps/api/core');
    const api = await import('../../services/tauri');

    render(<UnifiedDagView attemptId="attempt-1" problemId="problem-1" />);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('get_obligation_graph', { attemptId: 'attempt-1' });
      expect(api.getDagEdgesFrom).toHaveBeenCalledWith('attempt-1');
    });

    expect(screen.getByText('DAG View')).toBeInTheDocument();
    expect(screen.getAllByText('[CNT] Count roots').length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole('button', { name: 'steps' }));
    fireEvent.click(screen.getAllByText('#1 lemma')[0]);

    expect(navigationStoreState.goToStep).toHaveBeenCalledWith('problem-1', 'attempt-1', 'step-1');
  });
});
