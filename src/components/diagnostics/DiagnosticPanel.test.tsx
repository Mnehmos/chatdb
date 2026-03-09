import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let diagnosticStoreState: Record<string, unknown>;

const useDiagnosticStoreMock = vi.fn(() => diagnosticStoreState);

vi.mock('../../stores/diagnosticStore', () => ({
  useDiagnosticStore: useDiagnosticStoreMock,
}));

describe('DiagnosticPanel', () => {
  beforeEach(() => {
    diagnosticStoreState = {
      events: [],
      health: null,
      filterCategory: null,
      filterSeverity: null,
      expanded: new Set(),
      open: false,
      setFilterCategory: vi.fn(),
      setFilterSeverity: vi.fn(),
      setOpen: vi.fn(),
      clear: vi.fn(),
      refreshHealth: vi.fn(),
      toggleExpanded: vi.fn(),
    };
    useDiagnosticStoreMock.mockImplementation(() => diagnosticStoreState);
  });

  it('shows the compact toggle when closed and opens the panel on click', async () => {
    diagnosticStoreState = {
      ...diagnosticStoreState,
      events: [
        {
          ts: '2026-03-06T00:00:00Z',
          category: 'model',
          severity: 'warn',
          source: 'reviewer',
          message: 'Low confidence',
        },
        {
          ts: '2026-03-06T00:00:01Z',
          category: 'mechanical',
          severity: 'error',
          source: 'loop_engine',
          message: 'Loop failed',
        },
      ],
    };
    useDiagnosticStoreMock.mockImplementation(() => diagnosticStoreState);

    const { DiagnosticPanel } = await import('./DiagnosticPanel');

    const { container } = render(<DiagnosticPanel />);

    expect(screen.getByText('Diagnostics')).toBeInTheDocument();
    expect(container.querySelectorAll('.error-count-badge')).toHaveLength(2);

    fireEvent.click(screen.getByText('Diagnostics'));

    expect(diagnosticStoreState.setOpen).toHaveBeenCalledWith(true);
  });

  it('renders filtered events, health status, and forwards filter and row actions when open', async () => {
    diagnosticStoreState = {
      events: [
        {
          ts: '2026-03-06T00:00:00Z',
          category: 'mechanical',
          severity: 'fatal',
          source: 'loop_engine',
          step_number: 3,
          message: 'Loop failed',
          detail: { reason: 'timeout' },
        },
        {
          ts: '2026-03-06T00:00:01Z',
          category: 'model',
          severity: 'info',
          source: 'solver',
          message: 'Background thought',
        },
      ],
      health: {
        sidecar_reachable: true,
        lean_available: true,
        lean_ready: false,
        lean_warming_up: true,
        lean_warmup_attempts: 1,
        active_attempt: 'attempt-1',
        loop_running: true,
      },
      filterCategory: 'mechanical',
      filterSeverity: null,
      expanded: new Set([0]),
      open: true,
      setFilterCategory: vi.fn(),
      setFilterSeverity: vi.fn(),
      setOpen: vi.fn(),
      clear: vi.fn(),
      refreshHealth: vi.fn(),
      toggleExpanded: vi.fn(),
    };
    useDiagnosticStoreMock.mockImplementation(() => diagnosticStoreState);

    const { DiagnosticPanel } = await import('./DiagnosticPanel');

    render(<DiagnosticPanel />);

    expect(diagnosticStoreState.refreshHealth).toHaveBeenCalled();
    expect(screen.getByText('1/2')).toBeInTheDocument();
    expect(screen.getByText('Loop failed')).toBeInTheDocument();
    expect(screen.queryByText('Background thought')).not.toBeInTheDocument();
    expect(screen.getByText('Sidecar')).toBeInTheDocument();
    expect(screen.getByText('Lean')).toBeInTheDocument();
    expect(screen.getByText('Loop')).toBeInTheDocument();
    expect(screen.getByText(/timeout/)).toBeInTheDocument();

    fireEvent.click(screen.getByText('Mechanical'));
    fireEvent.click(screen.getByRole('button', { name: 'FATAL' }));
    fireEvent.click(screen.getByText('Loop failed'));
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));

    expect(diagnosticStoreState.setFilterCategory).toHaveBeenCalledWith(null);
    expect(diagnosticStoreState.setFilterSeverity).toHaveBeenCalledWith('fatal');
    expect(diagnosticStoreState.toggleExpanded).toHaveBeenCalledWith(0);
    expect(diagnosticStoreState.setOpen).toHaveBeenCalledWith(false);
  });
});
