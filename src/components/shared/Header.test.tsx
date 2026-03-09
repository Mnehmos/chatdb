import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { Header } from './Header';

describe('Header', () => {
  it('renders the product title and version', () => {
    render(<Header />);

    expect(screen.getByText('ChatDB')).toBeInTheDocument();
    expect(screen.getByText('Verified Reasoning Engine')).toBeInTheDocument();
    expect(screen.getByText('v0.1.0')).toBeInTheDocument();
  });
});
