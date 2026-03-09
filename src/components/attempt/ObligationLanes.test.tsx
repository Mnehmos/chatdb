import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../loop/MiniTokenStream', () => ({
  MiniTokenStream: ({
    obligationId,
    focused,
    onFocus,
  }: {
    obligationId: string;
    focused?: boolean;
    onFocus?: () => void;
  }) => (
    <button onClick={onFocus}>
      {`mini-${obligationId}-${focused ? 'focused' : 'plain'}`}
    </button>
  ),
}));

function createStep(id: string, overrides: Record<string, unknown> = {}) {
  return {
    id,
    step_number: 1,
    attempt_id: 'attempt-1',
    model: 'solver-a',
    goal_state: '',
    proposal_type: 'lemma',
    proposal_natural: `step ${id}`,
    verified: true,
    created_at: '2026-03-06T00:00:00Z',
    ...overrides,
  } as any;
}

function createObligation(id: string, overrides: Record<string, unknown> = {}) {
  return {
    id,
    description: `obligation ${id}`,
    obligation_type: 'COUNT',
    priority: 0.5,
    status: 'open',
    ...overrides,
  } as any;
}

describe('ObligationLanes', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders general and obligation lanes, limits visible lanes, and wires focus', async () => {
    const { ObligationLanes } = await import('./ObligationLanes');
    const onStepClick = vi.fn();
    const onObligationFocus = vi.fn();

    render(
      <ObligationLanes
        status="running"
        steps={[
          createStep('general', { proposal_natural: 'General step', obligation_id: null }),
          createStep('step-1', {
            step_number: 2,
            proposal_natural: 'Open step',
            obligation_id: 'ob-1',
          }),
          createStep('step-2', {
            step_number: 3,
            proposal_natural: 'Rejected step',
            obligation_id: 'ob-2',
            verified: false,
            rejection_reason: 'counterexample found',
          }),
        ]}
        obligations={[
          createObligation('ob-1', { description: 'Primary lane', priority: 0.9 }),
          createObligation('ob-2', { description: 'Secondary lane', priority: 0.7 }),
          createObligation('ob-3', { description: 'Solved lane', priority: 0.4, status: 'closed_proved' }),
          createObligation('ob-4', { description: 'Overflow lane', priority: 0.1, status: 'closed_refuted' }),
        ]}
        focusedObligationId="ob-1"
        onStepClick={onStepClick}
        onObligationFocus={onObligationFocus}
      />,
    );

    expect(screen.getByText('GENERAL')).toBeInTheDocument();
    expect(screen.getByText('mini-ob-1-focused')).toBeInTheDocument();
    expect(screen.getByText('mini-ob-2-plain')).toBeInTheDocument();
    expect(screen.queryByText('mini-ob-3-plain')).not.toBeInTheDocument();
    expect(screen.getByText('+1 more obligation')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Open step'));
    fireEvent.click(screen.getByText('mini-ob-1-focused'));

    expect(onStepClick).toHaveBeenCalledWith('step-1');
    expect(onObligationFocus).toHaveBeenCalledWith(null);
  });

  it('shows empty non-running lanes when no steps have been produced', async () => {
    const { ObligationLanes } = await import('./ObligationLanes');

    render(
      <ObligationLanes
        status="paused"
        steps={[]}
        obligations={[createObligation('ob-1', { status: 'closed_refuted' })]}
      />,
    );

    expect(screen.getByText('FAILED')).toBeInTheDocument();
    expect(screen.getByText('no steps')).toBeInTheDocument();
  });
});
