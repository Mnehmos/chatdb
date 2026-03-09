import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

function createStep(stepNumber: number, overrides: Record<string, unknown> = {}) {
  return {
    id: `step-${stepNumber}`,
    step_number: stepNumber,
    attempt_id: 'attempt-1',
    model: 'solver-a',
    goal_state: '',
    proposal_type: 'lemma',
    proposal_natural: `Natural ${stepNumber}`,
    verified: true,
    created_at: '2026-03-06T00:00:00Z',
    ...overrides,
  } as any;
}

describe('ProofChain', () => {
  it('renders an empty state when there are no steps', async () => {
    const { ProofChain } = await import('./ProofChain');

    render(<ProofChain steps={[]} onStepClick={vi.fn()} />);

    expect(screen.getByText('Proof Chain (0 steps)')).toBeInTheDocument();
    expect(screen.getByText('No steps yet')).toBeInTheDocument();
  });

  it('shows the last 20 steps by default and can expand to the full chain', async () => {
    const { ProofChain } = await import('./ProofChain');
    const steps = Array.from({ length: 21 }, (_, index) => createStep(index + 1));

    render(<ProofChain steps={steps} onStepClick={vi.fn()} />);

    expect(screen.getByText('Proof Chain (21 steps)')).toBeInTheDocument();
    expect(screen.queryByText('#1')).not.toBeInTheDocument();
    expect(screen.getByText('#21')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'All 21' }));

    expect(screen.getByText('#1')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Last 20' })).toBeInTheDocument();
  });

  it('expands a step to show details and opens the full detail view', async () => {
    const { ProofChain } = await import('./ProofChain');
    const onStepClick = vi.fn();

    render(
      <ProofChain
        onStepClick={onStepClick}
        steps={[
          createStep(3, {
            verified: false,
            proposal_formal: 'x**2',
            proposal_reasoning: 'Square both sides.',
            rejection_reason: 'Fails for x = -1.',
            challenge_attack: 'Try x = -1.',
            challenge_model: 'critic-a',
            challenge_confidence: 0.82,
            challenge_fatal: true,
          }),
        ]}
      />,
    );

    fireEvent.click(screen.getByText('#3'));

    expect(screen.getByText('Formal')).toBeInTheDocument();
    expect(screen.getByText('Reasoning')).toBeInTheDocument();
    expect(screen.getByText('Rejection')).toBeInTheDocument();
    expect(screen.getByText('Adversarial Challenge')).toBeInTheDocument();
    expect(screen.getByText('Square both sides.')).toBeInTheDocument();
    expect(screen.getByText('Fails for x = -1.')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Full Detail' }));

    expect(onStepClick).toHaveBeenCalledWith('step-3');
  });
});
