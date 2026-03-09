import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let loopStoreState: Record<string, unknown>;

const useLoopStoreMock = vi.fn(() => loopStoreState);

vi.mock('../../stores/loopStore', () => ({
  useLoopStore: useLoopStoreMock,
}));

vi.mock('./ObligationGraph', () => ({
  ObligationGraph: ({ attemptId }: { attemptId: string }) => (
    <div data-testid="obligation-graph">{attemptId}</div>
  ),
}));

function createStep(
  id: string,
  attemptId: string,
  stepNumber: number,
  overrides: Record<string, unknown> = {},
) {
  return {
    id,
    attempt_id: attemptId,
    step_number: stepNumber,
    model: 'solver-a',
    goal_state: '',
    proposal_type: 'lemma',
    proposal_natural: `Step ${stepNumber}`,
    verified: true,
    sympy_passed: true,
    created_at: '2026-03-06T00:00:00Z',
    ...overrides,
  } as any;
}

describe('ProofTree', () => {
  beforeEach(() => {
    loopStoreState = {
      steps: [],
      auditResults: [],
      reviewResult: null,
      reviewing: false,
      extractedPatterns: [],
      obligations: [],
      criticChecks: [],
      attemptId: null,
    };
    useLoopStoreMock.mockImplementation(() => loopStoreState);
  });

  it('shows an empty state before any steps are produced', async () => {
    const { ProofTree } = await import('./ProofTree');

    render(<ProofTree />);

    expect(screen.getByText('No steps yet. Start solving.')).toBeInTheDocument();
  });

  it('renders attempts, audits, obligations, review findings, and extracted patterns', async () => {
    loopStoreState = {
      steps: [
        createStep('step-1', 'attempt-1', 1, {
          proposal_natural: 'Open with a lemma.',
          proposal_formal: 'x**2',
        }),
        createStep('step-2', 'attempt-1', 2, {
          proposal_type: 'conclusion',
          proposal_natural: 'Therefore proved.',
        }),
        createStep('step-3', 'attempt-2', 3, {
          verified: false,
          proposal_natural: 'Fallback branch.',
          rejection_reason: 'Branch stalled.',
          challenge_model: 'critic-b',
          challenge_attack: 'Missing construction family.',
          challenge_flaw_found: true,
          challenge_fatal: false,
          challenge_confidence: 0.64,
        }),
      ],
      auditResults: [
        {
          step_number: 1,
          breadth: 0.35,
          confidence: 0.8,
          techniques_explored: ['modular arithmetic'],
          techniques_missing: ['contradiction'],
          recommended_direction: 'branch on parity',
          should_branch: true,
        },
      ],
      reviewResult: {
        exploration_coverage: 0.8,
        conclusion_sound: true,
        conclusion_confidence: 0.75,
        training_label: 'strong_attempt',
        findings: [
          {
            type: 'coverage_gap',
            summary: 'Need a contradiction branch.',
            detail: 'The proof never explored the adversarial case.',
            priority: 'high',
          },
        ],
        missing_constructions: ['contradiction'],
      },
      reviewing: true,
      extractedPatterns: [
        {
          name: 'Parity split',
          technique_class: 'number_theory',
          description: 'Split the argument into even and odd cases.',
          trigger_text: 'parity mismatch',
          strategy: 'branch on parity and close both subcases',
        },
      ],
      obligations: [
        {
          id: 'ob-1',
          description: 'Check parity branch',
          obligation_type: 'RESOLVE',
          priority: 0.95,
          status: 'open',
          assigned_model: 'solver-a',
          steps_spent: 2,
          max_steps: 4,
        },
        {
          id: 'ob-2',
          description: 'Close base case',
          obligation_type: 'CASE_CHECK',
          priority: 0.4,
          status: 'closed_proved',
          closed_by_step: 2,
          closure_note: 'Resolved cleanly.',
        },
      ],
      criticChecks: [
        {
          obligation_id: 'ob-1',
          check_description: 'Need a counterexample search.',
          counterexample_hint: 'n = 1',
          likely_wrong: true,
        },
      ],
      attemptId: 'attempt-2',
    };
    useLoopStoreMock.mockImplementation(() => loopStoreState);

    const { ProofTree } = await import('./ProofTree');

    render(<ProofTree />);

    expect(screen.getByText('Attempt 1 / 2')).toBeInTheDocument();
    expect(screen.getByText('Attempt 2 / 2')).toBeInTheDocument();
    expect(screen.getByText('AUDIT @ #1')).toBeInTheDocument();
    expect(screen.getByText('BRANCH RECOMMENDED')).toBeInTheDocument();
    expect(screen.getByText('Obligations (1 open)')).toBeInTheDocument();
    expect(screen.getByText('Need a counterexample search.')).toBeInTheDocument();
    expect(screen.getByText(/likely wrong/)).toBeInTheDocument();
    expect(screen.getByText('Running post-attempt review...')).toBeInTheDocument();
    expect(screen.getByText('Post-Attempt Review')).toBeInTheDocument();
    expect(screen.getByText('Coverage: 80%')).toBeInTheDocument();
    expect(screen.getByText('Missing constructions:')).toBeInTheDocument();
    expect(screen.getByText('Patterns Extracted')).toBeInTheDocument();
    expect(screen.getByText('Parity split')).toBeInTheDocument();
    expect(screen.getByTestId('obligation-graph')).toHaveTextContent('attempt-2');
  });
});
