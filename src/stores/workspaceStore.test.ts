import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useWorkspaceStore } from './workspaceStore';

vi.mock('../services/tauri', () => ({
  listAttempts: vi.fn(),
  getClaimsForAttempt: vi.fn(),
  getConflictsForAttempt: vi.fn(),
  getReportForAttempt: vi.fn(),
  getReportsForProblem: vi.fn(),
  deleteAttempt: vi.fn(),
  getAttemptSteps: vi.fn(),
  getObligationGraph: vi.fn(),
}));

describe('workspaceStore', () => {
  beforeEach(() => {
    useWorkspaceStore.setState({
      attempts: [],
      attemptsLoading: false,
      dbSteps: [],
      dbObligations: [],
      claims: [],
      conflicts: [],
      reports: [],
      currentReport: null,
    });
  });

  it('maps obligation graph rows into obligation events for attempt views', async () => {
    const api = await import('../services/tauri');
    vi.mocked(api.getObligationGraph).mockResolvedValue([
      {
        id: 'obl-1',
        description: 'Prove the induction hypothesis',
        obligation_type: 'LEMMA',
        priority: 0.9,
        status: 'open',
        assigned_model: 'solver-a',
        steps_spent: 2,
        max_steps: 5,
      },
    ] as any);

    await useWorkspaceStore.getState().loadDbObligations('attempt-1');

    expect(useWorkspaceStore.getState().dbObligations).toEqual([
      {
        id: 'obl-1',
        description: 'Prove the induction hypothesis',
        obligation_type: 'LEMMA',
        priority: 0.9,
        status: 'open',
        assigned_model: 'solver-a',
        steps_spent: 2,
        max_steps: 5,
      },
    ]);
  });

  it('refreshes attempts after a successful delete', async () => {
    const api = await import('../services/tauri');
    vi.mocked(api.deleteAttempt).mockResolvedValue(undefined);
    vi.mocked(api.listAttempts).mockResolvedValue([
      {
        id: 'attempt-2',
        problem_id: 'problem-1',
        attempt_number: 2,
        status: 'reviewed',
        step_count: 4,
        steps_verified: 3,
        steps_rejected: 1,
        started_at: '2026-03-06T00:00:00Z',
      },
    ]);

    await useWorkspaceStore.getState().deleteAttempt('attempt-1', 'problem-1');

    expect(api.deleteAttempt).toHaveBeenCalledWith('attempt-1');
    expect(api.listAttempts).toHaveBeenCalledWith('problem-1');
    expect(useWorkspaceStore.getState().attempts[0]?.id).toBe('attempt-2');
  });

  it('clears attempt-scoped detail data without touching the attempt list', () => {
    useWorkspaceStore.setState({
      attempts: [
        {
          id: 'attempt-1',
          problem_id: 'problem-1',
          attempt_number: 1,
          status: 'running',
          step_count: 2,
          steps_verified: 1,
          steps_rejected: 1,
          started_at: '2026-03-06T00:00:00Z',
        },
      ],
      claims: [{ id: 'claim-1' } as any],
      conflicts: [{ id: 'conflict-1' } as any],
      currentReport: { id: 'report-1' } as any,
      dbSteps: [{ id: 'step-1' } as any],
      dbObligations: [{ id: 'obl-1' } as any],
    });

    useWorkspaceStore.getState().clearAttemptData();

    const state = useWorkspaceStore.getState();
    expect(state.attempts).toHaveLength(1);
    expect(state.claims).toEqual([]);
    expect(state.conflicts).toEqual([]);
    expect(state.currentReport).toBeNull();
    expect(state.dbSteps).toEqual([]);
    expect(state.dbObligations).toEqual([]);
  });
});
