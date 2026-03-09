import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let loopStoreState: Record<string, unknown>;

const useLoopStoreMock = vi.fn((selector?: (state: Record<string, unknown>) => unknown) =>
  selector ? selector(loopStoreState) : loopStoreState);

vi.mock('../../stores/loopStore', () => ({
  useLoopStore: useLoopStoreMock,
}));

vi.mock('../../services/tauri', () => ({
  getAfterActionReport: vi.fn(),
  formalizeProof: vi.fn(),
}));

describe('AfterActionReport', () => {
  beforeEach(() => {
    loopStoreState = {
      reviewResult: null,
      auditResults: [],
    };
    useLoopStoreMock.mockImplementation((selector?: (state: Record<string, unknown>) => unknown) =>
      selector ? selector(loopStoreState) : loopStoreState,
    );
  });

  it('shows an error state when the report cannot be loaded', async () => {
    const api = await import('../../services/tauri');
    const onClose = vi.fn();
    vi.mocked(api.getAfterActionReport).mockRejectedValue(new Error('sidecar down'));

    const { AfterActionReport } = await import('./AfterActionReport');

    render(<AfterActionReport problemId="problem-1" onClose={onClose} />);

    await waitFor(() => {
      expect(screen.getByText(/Failed to load report:/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalled();
  });

  it('renders report stats, review analysis, and audit history when available', async () => {
    const api = await import('../../services/tauri');
    const onClose = vi.fn();

    loopStoreState = {
      reviewResult: {
        exploration_coverage: 0.72,
        conclusion_sound: true,
        conclusion_confidence: 0.83,
        findings_count: 2,
        training_label: 'proof_complete',
        findings: [
          { type: 'gap', summary: 'Good breadth', detail: 'Covered both parity branches', priority: 'medium' },
        ],
        missing_constructions: ['modular arithmetic'],
      },
      auditResults: [
        {
          step_number: 4,
          breadth: 0.66,
          confidence: 0.91,
          should_branch: true,
          recommended_direction: 'Try contradiction next',
        },
      ],
    };
    useLoopStoreMock.mockImplementation((selector?: (state: Record<string, unknown>) => unknown) =>
      selector ? selector(loopStoreState) : loopStoreState,
    );

    vi.mocked(api.getAfterActionReport).mockResolvedValue({
      problem_id: 'problem-1',
      problem_statement: 'Prove x^2 >= 0',
      problem_domain: 'algebra',
      attempt_id: 'attempt-1',
      total_steps: 5,
      verified_steps: 4,
      rejected_steps: 1,
      accuracy_pct: 80,
      total_tokens_in: 3200,
      total_tokens_out: 1400,
      total_wall_ms: 6200,
      models_used: ['gpt-4o', 'claude'],
      verified_chain: [
        { step_number: 1, natural: 'Assume x is real.', model: 'gpt-4o' },
        { step_number: 2, natural: 'Then x^2 is nonnegative.', formal: 'x^2 \\ge 0', model: 'claude' },
      ],
      failure_modes: [{ reason: 'validator_mismatch', count: 1 }],
      started_at: '2026-03-06T00:00:00Z',
      proof_complete: true,
      open_obligations: 0,
    });

    const { AfterActionReport } = await import('./AfterActionReport');

    render(<AfterActionReport problemId="problem-1" onClose={onClose} />);

    await waitFor(() => {
      expect(screen.getByText('PROOF VERIFIED')).toBeInTheDocument();
    });

    expect(screen.getByText('Prove x^2 >= 0')).toBeInTheDocument();
    expect(screen.getByText('5')).toBeInTheDocument();
    expect(screen.getByText('80%')).toBeInTheDocument();
    expect(screen.getByText('3.2k')).toBeInTheDocument();
    expect(screen.getByText('1.4k')).toBeInTheDocument();
    expect(screen.getByText('6.2s')).toBeInTheDocument();
    expect(screen.getByText('Verified Proof Chain')).toBeInTheDocument();
    expect(screen.getByText('validator_mismatch')).toBeInTheDocument();
    expect(screen.getByText('Training label:')).toBeInTheDocument();
    expect(screen.getByText('proof complete')).toBeInTheDocument();
    expect(screen.getByText(/Missing constructions:/)).toBeInTheDocument();
    expect(screen.getByText(/modular arithmetic/)).toBeInTheDocument();
    expect(screen.getByText('Exploration Audits (1)')).toBeInTheDocument();
    expect(screen.getByText('Try contradiction next')).toBeInTheDocument();
  });

  it('requests Lean formalization for completed proofs and renders the result', async () => {
    const api = await import('../../services/tauri');
    const onClose = vi.fn();

    vi.mocked(api.getAfterActionReport).mockResolvedValue({
      problem_id: 'problem-1',
      problem_statement: 'Prove x^2 >= 0',
      problem_domain: 'algebra',
      attempt_id: 'attempt-1',
      total_steps: 3,
      verified_steps: 3,
      rejected_steps: 0,
      accuracy_pct: 100,
      total_tokens_in: 100,
      total_tokens_out: 50,
      total_wall_ms: 200,
      models_used: ['solver-a'],
      verified_chain: [
        { step_number: 1, natural: 'Squares are nonnegative.', formal: 'x^2 >= 0', model: 'solver-a' },
      ],
      failure_modes: [],
      started_at: '2026-03-06T00:00:00Z',
      proof_complete: true,
      open_obligations: 0,
      final_answer: 'Therefore x^2 >= 0.',
    });
    vi.mocked(api.formalizeProof).mockResolvedValue({
      success: true,
      lean_source: 'theorem chatdb_problem_1 (x : Int) : x^2 >= 0 := by omega',
      errors: [],
      attempts: 1,
    });

    const { AfterActionReport } = await import('./AfterActionReport');

    render(<AfterActionReport problemId="problem-1" onClose={onClose} />);

    await waitFor(() => {
      expect(screen.getByText('PROOF VERIFIED')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Formalize in Lean 4' }));

    await waitFor(() => {
      expect(api.formalizeProof).toHaveBeenCalledWith('problem-1');
    });

    expect(screen.getByText('Lean Formalization')).toBeInTheDocument();
    expect(screen.getByText('Compiled')).toBeInTheDocument();
    expect(screen.getByText(/theorem chatdb_problem_1/)).toBeInTheDocument();
  });
});
