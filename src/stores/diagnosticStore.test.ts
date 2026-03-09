import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useDiagnosticStore } from './diagnosticStore';

vi.mock('../services/tauri', () => ({
  getSystemHealth: vi.fn(),
}));

describe('diagnosticStore', () => {
  beforeEach(() => {
    localStorage.clear();
    useDiagnosticStore.setState({
      events: [],
      health: null,
      filterCategory: null,
      filterSeverity: null,
      expanded: new Set(),
      open: false,
    });
  });

  it('clear resets events, expanded rows, and active filters', () => {
    useDiagnosticStore.setState({
      events: [
        {
          ts: '2026-03-06T00:00:00Z',
          category: 'mechanical',
          severity: 'fatal',
          source: 'loop_engine',
          message: 'Loop crashed',
        },
      ],
      filterCategory: 'mechanical',
      filterSeverity: 'fatal',
      expanded: new Set([0]),
    });

    useDiagnosticStore.getState().clear();

    const state = useDiagnosticStore.getState();
    expect(state.events).toEqual([]);
    expect(Array.from(state.expanded)).toEqual([]);
    expect(state.filterCategory).toBeNull();
    expect(state.filterSeverity).toBeNull();
  });

  it('setOpen persists the open state to localStorage', () => {
    useDiagnosticStore.getState().setOpen(true);
    expect(localStorage.getItem('diag_open')).toBe('1');

    useDiagnosticStore.getState().setOpen(false);
    expect(localStorage.getItem('diag_open')).toBe('0');
  });
});
