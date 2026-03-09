import { describe, expect, it } from 'vitest';

import { renderLatexText, stripLatex } from './latex';

describe('latex utils', () => {
  it('escapes raw HTML when rendering plain text', () => {
    expect(renderLatexText('<script>alert(1)</script>')).toBe(
      '&lt;script&gt;alert(1)&lt;/script&gt;',
    );
  });

  it('renders inline LaTeX fragments through KaTeX', () => {
    const rendered = renderLatexText('Solve $x^2$ now');

    expect(rendered).toContain('katex');
    expect(rendered).toContain('Solve ');
    expect(rendered).toContain(' now');
  });

  it('strips common LaTeX commands into a plain-text preview', () => {
    expect(stripLatex('Consider $\\frac{a}{b} \\in \\mathbb{N}$', 80)).toBe(
      'Consider a/b in N',
    );
  });
});
