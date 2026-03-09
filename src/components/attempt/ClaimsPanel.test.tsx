import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

describe('ClaimsPanel', () => {
  it('renders nothing when there are no claims', async () => {
    const { ClaimsPanel } = await import('./ClaimsPanel');
    const { container } = render(<ClaimsPanel claims={[]} />);

    expect(container.firstChild).toBeNull();
  });

  it('groups claims by type and marks superseded entries', async () => {
    const { ClaimsPanel } = await import('./ClaimsPanel');
    const { container } = render(
      <ClaimsPanel
        claims={[
          {
            id: 'claim-1',
            claim_type: 'parity',
            object: 'n',
            direction: 'even',
            confidence: 0.9,
            natural_text: 'n is even',
            scope_type: 'domain',
            scope_param: 'integers',
            scope_constraint: 'n > 0',
          },
          {
            id: 'claim-2',
            claim_type: 'parity',
            object: 'm',
            confidence: 0.6,
            natural_text: 'm is odd',
            superseded_by: 'claim-3',
          },
        ] as any}
      />,
    );

    expect(screen.getByText('Claims (2)')).toBeInTheDocument();
    expect(screen.getByText('parity: 2')).toBeInTheDocument();
    expect(screen.getByText('n is even')).toBeInTheDocument();
    expect(screen.getByText('domain: integers (n > 0)')).toBeInTheDocument();
    expect(container.querySelector('.claim-superseded')).not.toBeNull();
  });
});
