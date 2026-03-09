import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let loopStoreState: Record<string, unknown>;
let navigationStoreState: Record<string, unknown>;

const useLoopStoreMock = vi.fn(() => loopStoreState);
const useNavigationStoreMock = vi.fn(() => navigationStoreState);

vi.mock('../../stores/loopStore', () => ({
  useLoopStore: useLoopStoreMock,
}));

vi.mock('../../stores/navigationStore', () => ({
  useNavigationStore: useNavigationStoreMock,
}));

vi.mock('../../services/tauri', () => ({
  getClaimsForStep: vi.fn(),
  getDagEdgesFrom: vi.fn(),
  getDagEdgesTo: vi.fn(),
  getToolRunsForStep: vi.fn(),
}));

describe('StepDetailView', () => {
  beforeEach(async () => {
    loopStoreState = { steps: [] };
    navigationStoreState = { goBack: vi.fn() };
    useLoopStoreMock.mockImplementation(() => loopStoreState);
    useNavigationStoreMock.mockImplementation(() => navigationStoreState);
    const api = await import('../../services/tauri');
    vi.mocked(api.getClaimsForStep).mockResolvedValue([]);
    vi.mocked(api.getDagEdgesFrom).mockResolvedValue([]);
    vi.mocked(api.getDagEdgesTo).mockResolvedValue([]);
    vi.mocked(api.getToolRunsForStep).mockResolvedValue([]);
  });

  it('shows a not-found state and goes back when the step is missing', async () => {
    const { StepDetailView } = await import('./StepDetailView');

    render(<StepDetailView problemId="problem-1" attemptId="attempt-1" stepId="missing-step" />);

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));

    expect(screen.getByText('Step not found')).toBeInTheDocument();
    expect(navigationStoreState.goBack).toHaveBeenCalled();
  });

  it('renders verification detail, claims, DAG neighbors, and expandable tool runs', async () => {
    const api = await import('../../services/tauri');

    loopStoreState = {
      steps: [
        {
          id: 'step-1',
          attempt_id: 'attempt-1',
          step_number: 3,
          model: 'gpt-4o',
          goal_state: '',
          proposal_type: 'lemma',
          proposal_natural: 'Assume n is even.',
          proposal_formal: 'n = 2k',
          proposal_reasoning: 'Introduce the standard parity witness.',
          verified: false,
          rejection_reason: 'Counterexample at n = 1',
          sympy_passed: false,
          pint_passed: null,
          lean_passed: true,
          challenge_model: 'critic-1',
          challenge_flaw_found: true,
          challenge_attack: 'Try n = 1.',
          challenge_confidence: 0.87,
          challenge_fatal: true,
          created_at: '2026-03-06T00:00:00Z',
        },
      ],
    };
    useLoopStoreMock.mockImplementation(() => loopStoreState);

    vi.mocked(api.getClaimsForStep).mockResolvedValue([
      { id: 'claim-1', claim_type: 'parity', natural_text: 'n is even' } as any,
    ]);
    vi.mocked(api.getDagEdgesFrom).mockResolvedValue([
      { id: 1, source_id: 'step-1', target_id: 'node-2', edge_type: 'supports', target_type: 'proof_node' } as any,
    ]);
    vi.mocked(api.getDagEdgesTo).mockResolvedValue([
      { id: 2, source_id: 'node-0', target_id: 'step-1', edge_type: 'depends_on', source_type: 'proof_node' } as any,
    ]);
    vi.mocked(api.getToolRunsForStep).mockResolvedValue([
      {
        id: 'tool-1',
        tool_name: 'sympy_check',
        status: 'failed',
        latency_ms: 120,
        query_json: '{"expr":"x^2"}',
        result_summary: 'SymPy found a mismatch',
        error_message: 'Division by zero',
        agent_role: 'solver',
        created_at: '2026-03-06T00:00:00Z',
      } as any,
    ]);

    const { StepDetailView } = await import('./StepDetailView');

    render(<StepDetailView problemId="problem-1" attemptId="attempt-1" stepId="step-1" />);

    await waitFor(() => {
      expect(screen.getByText('Claims Extracted (1)')).toBeInTheDocument();
    });

    expect(screen.getByText('Rejected')).toBeInTheDocument();
    expect(screen.getByText('lemma')).toBeInTheDocument();
    expect(screen.getByText('Assume n is even.')).toBeInTheDocument();
    expect(screen.getByText('n = 2k')).toBeInTheDocument();
    expect(screen.getByText('Introduce the standard parity witness.')).toBeInTheDocument();
    expect(screen.getByText('Counterexample at n = 1')).toBeInTheDocument();
    expect(screen.getByText('Model: critic-1')).toBeInTheDocument();
    expect(screen.getByText('Try n = 1.')).toBeInTheDocument();
    expect(screen.getByText('parity')).toBeInTheDocument();
    expect(screen.getByText('n is even')).toBeInTheDocument();
    expect(screen.getByText('DAG Neighbors (2)')).toBeInTheDocument();

    fireEvent.click(screen.getByText('sympy_check'));

    expect(screen.getByText('Division by zero')).toBeInTheDocument();
    expect(screen.getByText(/SymPy found a mismatch/)).toBeInTheDocument();
  });
});
