import katex from 'katex';
import 'katex/dist/katex.min.css';

/**
 * Render a string containing LaTeX delimiters ($...$  and $$...$$) into HTML.
 * Non-math text is escaped and passed through as-is.
 * Returns an HTML string safe for dangerouslySetInnerHTML.
 */
export function renderLatexText(text: string): string {
  if (!text) return '';
  // Fast-path: no $ at all → return escaped text
  if (!text.includes('$')) return escapeHtml(text);

  const parts: string[] = [];
  let i = 0;

  while (i < text.length) {
    // Check for display math $$...$$
    if (text[i] === '$' && text[i + 1] === '$') {
      const end = text.indexOf('$$', i + 2);
      if (end !== -1) {
        const tex = text.slice(i + 2, end);
        parts.push(renderKatex(tex, true));
        i = end + 2;
        continue;
      }
    }
    // Check for inline math $...$
    if (text[i] === '$') {
      const end = text.indexOf('$', i + 1);
      if (end !== -1) {
        const tex = text.slice(i + 1, end);
        parts.push(renderKatex(tex, false));
        i = end + 1;
        continue;
      }
    }
    // Regular text — collect until next $
    const next = text.indexOf('$', i);
    if (next === -1) {
      parts.push(escapeHtml(text.slice(i)));
      break;
    }
    parts.push(escapeHtml(text.slice(i, next)));
    i = next;
  }

  return parts.join('');
}

function renderKatex(tex: string, displayMode: boolean): string {
  try {
    return katex.renderToString(tex, { throwOnError: false, displayMode });
  } catch {
    return `<code>${escapeHtml(tex)}</code>`;
  }
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

/**
 * Truncate text with LaTeX — strips $ delimiters AND LaTeX commands for a plain-text preview.
 */
export function stripLatex(text: string, maxLen: number): string {
  let plain = text;
  // Remove display and inline math delimiters
  plain = plain.replace(/\$\$/g, '').replace(/\$/g, '');
  // Remove LaTeX commands with braced arguments: \mathbb{N} → N, \frac{a}{b} → a/b
  plain = plain.replace(/\\frac\{([^}]*)\}\{([^}]*)\}/g, '$1/$2');
  plain = plain.replace(/\\(?:mathbb|mathcal|mathfrak|mathrm|mathbf|mathit|text|textbf|operatorname)\{([^}]*)\}/g, '$1');
  // Remove remaining LaTeX commands: \alpha → alpha, \leq → leq
  plain = plain.replace(/\\([a-zA-Z]+)/g, '$1');
  // Remove braces
  plain = plain.replace(/[{}]/g, '');
  // Collapse whitespace
  plain = plain.replace(/\s+/g, ' ').trim();
  return plain.length > maxLen ? plain.slice(0, maxLen) + '...' : plain;
}
