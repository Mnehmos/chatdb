import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

function createObligation(id: string, overrides: Record<string, unknown> = {}) {
  return {
    id,
    description: `obligation ${id}`,
    obligation_type: 'COUNT',
    priority: 0.4,
    status: 'open',
    steps_spent: 0,
    max_steps: 5,
    signals: [],
    tally_yes: 0,
    tally_total: 0,
    ...overrides,
  } as any;
}

describe('ObligationQueue', () => {
  it('renders an empty state when there are no obligations', async () => {
    const { ObligationQueue } = await import('./ObligationQueue');

    render(<ObligationQueue obligations={[]} />);

    expect(screen.getByText('Obligations')).toBeInTheDocument();
    expect(screen.getByText('No obligations')).toBeInTheDocument();
  });

  it('sorts open and closed obligations and emits selections', async () => {
    const { ObligationQueue } = await import('./ObligationQueue');
    const onSelect = vi.fn();

    const { container } = render(
      <ObligationQueue
        obligations={[
          createObligation('open-low', {
            description: 'low priority open',
            priority: 0.2,
            steps_spent: 1,
            max_steps: 4,
          }),
          createObligation('closed-refuted', {
            description: 'refuted branch',
            status: 'closed_refuted',
            closed_by_step: 8,
            closure_note: 'contradiction found',
          }),
          createObligation('open-high', {
            description: 'high priority open',
            priority: 0.95,
            signals: [{ source: 'critic', satisfies: true, note: 'good candidate' }],
            tally_yes: 1,
            tally_total: 1,
            assigned_model: 'solver-a',
          }),
          createObligation('closed-proved', {
            description: 'proved branch',
            status: 'closed_proved',
            closed_by_step: 3,
            closure_note: 'resolved by lemma',
          }),
        ]}
        onSelect={onSelect}
      />,
    );

    expect(screen.getByText('Obligations (4)')).toBeInTheDocument();
    expect(screen.getAllByText('OPEN').length).toBeGreaterThan(0);
    expect(screen.getAllByText('CLOSED').length).toBeGreaterThan(0);
    expect(screen.getByText('resolved by lemma')).toBeInTheDocument();
    expect(screen.getByText('contradiction found')).toBeInTheDocument();
    expect(screen.getByText('closed at step #3')).toBeInTheDocument();
    expect(screen.getByText('closed at step #8')).toBeInTheDocument();
    expect(screen.getByText('Votes:')).toBeInTheDocument();
    expect(screen.getByText('solver-a')).toBeInTheDocument();

    const descriptions = Array.from(container.querySelectorAll('.obligation-card .ob-desc')).map(
      (node) => node.textContent,
    );

    expect(descriptions).toEqual([
      'high priority open',
      'low priority open',
      'proved branch',
      'refuted branch',
    ]);

    fireEvent.click(screen.getByText('high priority open'));

    expect(onSelect).toHaveBeenCalledWith('open-high');
  });

  it('does not mark a tally as majority until at least three votes have been cast', async () => {
    const { ObligationQueue } = await import('./ObligationQueue');

    const { container, rerender } = render(
      <ObligationQueue
        obligations={[
          createObligation('open-thin-majority', {
            description: 'mechanical only',
            tally_yes: 1,
            tally_total: 1,
            signals: [{ source: 'mechanical', satisfies: true, note: 'validator pass' }],
          }),
        ]}
      />,
    );

    expect(container.querySelector('.ob-tally-count')).not.toHaveClass('tally-majority');

    rerender(
      <ObligationQueue
        obligations={[
          createObligation('open-real-majority', {
            description: 'council confirmed',
            tally_yes: 2,
            tally_total: 3,
            signals: [
              { source: 'mechanical', satisfies: true, note: 'validator pass' },
              { source: 'reviewer', satisfies: true, note: 'looks complete' },
              { source: 'adversary', satisfies: false, note: 'one caveat remains' },
            ],
          }),
        ]}
      />,
    );

    expect(container.querySelector('.ob-tally-count')).toHaveClass('tally-majority');
  });
});
