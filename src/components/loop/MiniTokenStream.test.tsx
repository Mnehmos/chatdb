import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

type EventCallback = (event: { payload: unknown }) => void;

const listenMock = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

describe('MiniTokenStream', () => {
  beforeEach(() => {
    listenMock.mockReset();
    vi.resetModules();
  });

  it('filters events to a single obligation and toggles activity on matching thinking events', async () => {
    const handlers = new Map<string, EventCallback>();
    const unlistens = [vi.fn(), vi.fn(), vi.fn()];

    listenMock.mockImplementation((name: string, callback: EventCallback) => {
      handlers.set(name, callback);
      return Promise.resolve(unlistens[handlers.size - 1]);
    });

    const onFocus = vi.fn();
    const { MiniTokenStream } = await import('./MiniTokenStream');

    const { unmount } = render(
      <MiniTokenStream obligationId="obl-1" focused onFocus={onFocus} />,
    );

    expect(screen.getByText('waiting...')).toBeInTheDocument();

    await act(async () => {
      handlers.get('loop:thinking_start')?.({
        payload: { step_number: 1, model: 'solver-a', agent_role: 'solver', obligation_id: 'other' },
      });
    });

    expect(screen.getByText('waiting...')).toBeInTheDocument();

    await act(async () => {
      handlers.get('loop:thinking_start')?.({
        payload: { step_number: 2, model: 'solver-a', agent_role: 'solver', obligation_id: 'obl-1' },
      });
      handlers.get('loop:token')?.({
        payload: { text: 'First token.', agent_role: 'solver', obligation_id: 'obl-1' },
      });
      handlers.get('loop:token')?.({
        payload: { text: 'Ignored token.', agent_role: 'solver', obligation_id: 'other' },
      });
    });

    expect(screen.getByText('SOLVER')).toBeInTheDocument();
    expect(screen.getByText(/First token\./)).toBeInTheDocument();
    expect(screen.queryByText(/Ignored token\./)).not.toBeInTheDocument();

    fireEvent.click(screen.getByTitle('Click to focus main view'));
    expect(onFocus).toHaveBeenCalled();

    await act(async () => {
      handlers.get('loop:thinking_end')?.({ payload: { obligation_id: 'other' } });
    });

    expect(screen.getByText('SOLVER')).toBeInTheDocument();

    await act(async () => {
      handlers.get('loop:thinking_end')?.({ payload: { obligation_id: 'obl-1' } });
    });

    expect(screen.getByText('SOLVER')).toBeInTheDocument();

    unmount();
    await waitFor(() => {
      unlistens.forEach((fn) => expect(fn).toHaveBeenCalled());
    });
  });
});
