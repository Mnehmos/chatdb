import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let navigationStoreState: Record<string, unknown>;

const useNavigationStoreMock = vi.fn(() => navigationStoreState);

vi.mock('../../stores/navigationStore', () => ({
  useNavigationStore: useNavigationStoreMock,
}));

vi.mock('../../services/tauri', () => ({
  getObligationDetail: vi.fn(),
  getObligationProofNodes: vi.fn(),
  getObligationSignals: vi.fn(),
  getToolRunsForObligation: vi.fn(),
}));

describe('ObligationDetailView', () => {
  beforeEach(() => {
    navigationStoreState = { goBack: vi.fn() };
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);
  });

  it('renders an error state when the obligation cannot be loaded', async () => {
    const api = await import('../../services/tauri');
    vi.mocked(api.getObligationDetail).mockRejectedValue(new Error('missing obligation'));
    vi.mocked(api.getObligationProofNodes).mockResolvedValue([]);
    vi.mocked(api.getObligationSignals).mockResolvedValue([]);
    vi.mocked(api.getToolRunsForObligation).mockResolvedValue([]);

    const { ObligationDetailView } = await import('./ObligationDetailView');

    render(<ObligationDetailView obligationId="obl-missing" />);

    await waitFor(() => {
      expect(screen.getByText(/Failed to load obligation:/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(navigationStoreState.goBack).toHaveBeenCalled();
  });

  it('renders obligation metadata, signals, nodes, and tool runs', async () => {
    const api = await import('../../services/tauri');
    vi.mocked(api.getObligationDetail).mockResolvedValue({
      id: 'obl-1',
      attempt_id: 'attempt-1',
      branch_id: 1,
      parent_node_id: 'node-0',
      description: 'Close the parity contradiction',
      obligation_type: 'RESOLVE',
      priority: 0.95,
      confidence: 0.8,
      source_layer: 4,
      status: 'closed_proved',
      assigned_model: 'solver-a',
      closure_type: 'proof_node',
      escalation_level: 2,
      steps_spent: 2,
      max_steps: 5,
      satisfaction_criteria: '{"goal":"prove contradiction"}',
      search_space: '{"techniques":["parity"]}',
      assigned_models_json: '["solver-a","solver-b"]',
      active_solver_round_id: 'round-12345678',
      created_at: '2026-03-06T00:00:00Z',
      closed_at: '2026-03-06T00:05:00Z',
    } as any);
    vi.mocked(api.getObligationProofNodes).mockResolvedValue([
      {
        id: 'node-1',
        sequence_number: 4,
        node_type: 'lemma',
        status: 'verified',
        technique_class: 'induction',
        content: 'Assume the parity split.',
        formal_content: 'n = 2k ∨ n = 2k+1',
        validator_used: 'sympy',
        validator_result: 'ok',
        construction_family: 'parity',
        token_cost: 123,
        created_at: '2026-03-06T00:01:00Z',
      } as any,
    ]);
    vi.mocked(api.getObligationSignals).mockResolvedValue([
      {
        id: 'sig-1',
        source: 'solver',
        satisfies: true,
        confidence: 0.9,
        note: 'Direct contradiction found',
        created_at: '2026-03-06T00:02:00Z',
      },
      {
        id: 'sig-2',
        source: 'critic',
        satisfies: false,
        confidence: 0.2,
        created_at: '2026-03-06T00:03:00Z',
      },
    ] as any);
    vi.mocked(api.getToolRunsForObligation).mockResolvedValue([
      {
        id: 'tool-1',
        tool_name: 'oeis_lookup',
        status: 'completed',
        latency_ms: 88,
        step_number: 3,
        result_summary: 'Matched a parity sequence.',
      } as any,
    ]);

    const { ObligationDetailView } = await import('./ObligationDetailView');

    render(<ObligationDetailView obligationId="obl-1" />);

    await waitFor(() => {
      expect(screen.getByText('Close the parity contradiction')).toBeInTheDocument();
    });

    expect(screen.getByText('[S+]')).toBeInTheDocument();
    expect(screen.getByText('PROVED')).toBeInTheDocument();
    expect(screen.getByText('80%')).toBeInTheDocument();
    expect(screen.getByText('2 / 5 steps')).toBeInTheDocument();
    expect(screen.getByText('Validator')).toBeInTheDocument();
    expect(screen.getByText('solver-a')).toBeInTheDocument();
    expect(screen.getByText('Satisfaction Signals (1/2)')).toBeInTheDocument();
    expect(screen.getByText('Direct contradiction found')).toBeInTheDocument();
    expect(screen.getByText('Proof Nodes (1)')).toBeInTheDocument();
    expect(screen.getByText('Tool Calls (1)')).toBeInTheDocument();
    expect(screen.getByText(/Matched a parity sequence/)).toBeInTheDocument();

    fireEvent.click(screen.getByText('lemma'));

    expect(screen.getByText('Assume the parity split.')).toBeInTheDocument();
    expect(screen.getByText('n = 2k ∨ n = 2k+1')).toBeInTheDocument();
    expect(screen.getByText('ok')).toBeInTheDocument();
    expect(screen.getByText('123')).toBeInTheDocument();
  });
});
