import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useAgentStore } from './agentStore';

describe('agentStore', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-03-06T00:30:00Z'));
    useAgentStore.setState({
      orchestratorLog: [],
      criticLog: [],
      councilSessions: [],
      scoutResults: [],
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('wraps orchestrator and critic events with timestamps for later display', () => {
    useAgentStore.getState().addOrchestratorEvent({ type: 'route', worker: 'solver-a' });
    useAgentStore.getState().addCriticEvent({
      obligation_id: 'obl-1',
      check_description: 'Try x = 0',
      likely_wrong: true,
    });

    const state = useAgentStore.getState();
    expect(state.orchestratorLog).toEqual([
      {
        ts: '2026-03-06T00:30:00.000Z',
        data: { type: 'route', worker: 'solver-a' },
      },
    ]);
    expect(state.criticLog).toEqual([
      {
        ts: '2026-03-06T00:30:00.000Z',
        data: {
          obligation_id: 'obl-1',
          check_description: 'Try x = 0',
          likely_wrong: true,
        },
      },
    ]);
  });

  it('preserves council findings and scout results in insertion order', () => {
    useAgentStore.getState().addCouncilSession({
      obligation_id: 'obl-1',
      tally_yes: 2,
      tally_total: 3,
      outcome: 'needs_more_work',
    });
    useAgentStore.getState().addScoutResult({
      trigger: 'mid_solve',
      results_count: 3,
      sources: ['arxiv'],
      briefing: 'Try a parity split.',
    });

    const state = useAgentStore.getState();
    expect(state.councilSessions).toEqual([
      {
        obligation_id: 'obl-1',
        tally_yes: 2,
        tally_total: 3,
        outcome: 'needs_more_work',
      },
    ]);
    expect(state.scoutResults).toEqual([
      {
        trigger: 'mid_solve',
        results_count: 3,
        sources: ['arxiv'],
        briefing: 'Try a parity split.',
      },
    ]);
  });
});
