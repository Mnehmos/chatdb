import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let agentStoreState: Record<string, unknown>;

const useAgentStoreMock = vi.fn(() => agentStoreState);

vi.mock('../../stores/agentStore', () => ({
  useAgentStore: useAgentStoreMock,
}));

describe('AgentDashboard', () => {
  beforeEach(() => {
    agentStoreState = {
      orchestratorLog: [],
      criticLog: [],
      councilSessions: [],
      scoutResults: [],
    };
    useAgentStoreMock.mockImplementation(() => agentStoreState);
  });

  it('shows the empty state when there is no agent activity yet', async () => {
    const { AgentDashboard } = await import('./AgentDashboard');

    render(<AgentDashboard />);

    expect(screen.getByText('Agent activity appears here during solving.')).toBeInTheDocument();
  });

  it('renders activity counts and expands scout details on demand', async () => {
    agentStoreState = {
      orchestratorLog: [{ ts: '2026-03-06T00:00:00Z', data: { type: 'route' } }],
      criticLog: [{ ts: '2026-03-06T00:00:01Z', data: { likely_wrong: true } }],
      councilSessions: [{ trigger_type: 'review' }],
      scoutResults: [
        {
          trigger: 'mid_solve',
          sources: ['arxiv', 'oeis'],
          results_count: 3,
          obligation_desc: 'Need a contradiction lemma',
          briefing: 'Try a parity split first.',
        },
      ],
    };
    useAgentStoreMock.mockImplementation(() => agentStoreState);

    const { AgentDashboard } = await import('./AgentDashboard');

    const { container } = render(<AgentDashboard />);

    expect(screen.getByText('Agent Activity')).toBeInTheDocument();
    expect(screen.getByText('Orchestrator')).toBeInTheDocument();
    expect(screen.getByText('Critic')).toBeInTheDocument();
    expect(screen.getByText('Council')).toBeInTheDocument();
    expect(screen.getByText('Scout')).toBeInTheDocument();

    fireEvent.click(container.querySelector('.agent-panel-scout') as HTMLElement);

    expect(screen.getByText('MID-SOLVE')).toBeInTheDocument();
    expect(screen.getByText('arxiv, oeis')).toBeInTheDocument();
    expect(screen.getByText('3 results')).toBeInTheDocument();
    expect(screen.getByText('Stuck on: Need a contradiction lemma')).toBeInTheDocument();
    expect(screen.getByText('Try a parity split first.')).toBeInTheDocument();
  });
});
