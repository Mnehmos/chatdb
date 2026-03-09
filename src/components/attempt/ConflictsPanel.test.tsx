import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

describe('ConflictsPanel', () => {
  it('renders nothing when there are no conflicts', async () => {
    const { ConflictsPanel } = await import('./ConflictsPanel');
    const { container } = render(<ConflictsPanel conflicts={[]} />);

    expect(container.firstChild).toBeNull();
  });

  it('shows unresolved counts and resolution state', async () => {
    const { ConflictsPanel } = await import('./ConflictsPanel');
    const { container } = render(
      <ConflictsPanel
        conflicts={[
          {
            id: 'conflict-1',
            severity: 'high',
            conflict_type: 'contradiction',
            description: 'Step 3 contradicts step 1.',
          },
          {
            id: 'conflict-2',
            severity: 'low',
            conflict_type: 'duplication',
            description: 'Equivalent lemma already exists.',
            resolution: 'merged',
          },
        ] as any}
      />,
    );

    expect(screen.getByText('Conflicts (2)')).toBeInTheDocument();
    expect(screen.getByText((content) => content.includes('1 unresolved'))).toBeInTheDocument();
    expect(screen.getByText('contradiction')).toBeInTheDocument();
    expect(screen.getByText('merged')).toBeInTheDocument();
    expect(container.querySelector('.conflict-resolved')).not.toBeNull();
    expect(container.querySelector('.conflict-active')).not.toBeNull();
  });
});
