import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

type EventCallback = (event: { payload: unknown }) => void;

const listenMock = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

describe('ThinkingPanel', () => {
  beforeEach(() => {
    listenMock.mockReset();
    vi.resetModules();
  });

  it('renders streamed thinking text and transitions to last response after thinking ends', async () => {
    const handlers = new Map<string, EventCallback>();
    const unlistens: Array<ReturnType<typeof vi.fn>> = [];

    listenMock.mockImplementation((name: string, callback: EventCallback) => {
      handlers.set(name, callback);
      const unlisten = vi.fn();
      unlistens.push(unlisten);
      return Promise.resolve(unlisten);
    });

    const { ThinkingPanel } = await import('./ThinkingPanel');

    const { unmount } = render(<ThinkingPanel />);

    await act(async () => {
      handlers.get('loop:thinking_start')?.({
        payload: { step_number: 2, model: 'gpt-4o', agent_role: 'solver' },
      });
    });

    expect(screen.getByText('Thinking')).toBeInTheDocument();
    expect(screen.getByText('SOLVER')).toBeInTheDocument();
    expect(screen.getByText(/Step 2/)).toHaveTextContent('gpt-4o');

    await act(async () => {
      handlers.get('loop:token')?.({ payload: { text: 'Consider parity.' } });
      handlers.get('loop:critic_check')?.({
        payload: {
          obligation_id: 'obl-1',
          check_description: 'Try x = 0',
          expected_if_correct: 'Zero should preserve the invariant',
          counterexample_hint: 'Test the base case',
          likely_wrong: true,
        },
      });
      handlers.get('agent:scout_result')?.({
        payload: {
          trigger: 'mid_solve',
          obligation_desc: 'Need contradiction',
          results_count: 2,
          sources: ['arxiv'],
          briefing: 'Search parity lemmas.',
        },
      });
      handlers.get('loop:thinking_end')?.({ payload: undefined });
    });

    expect(screen.getByText('Last Response')).toBeInTheDocument();
    expect(screen.getByText(/Consider parity\./)).toBeInTheDocument();
    expect(screen.getByText(/Critic check: Try x = 0 \[LIKELY WRONG\]/)).toBeInTheDocument();
    expect(screen.getByText(/SCOUT \(mid-solve: Need contradiction\)/)).toBeInTheDocument();
    expect(screen.getByText(/Search parity lemmas\./)).toBeInTheDocument();

    fireEvent.click(screen.getByText('Last Response'));
    expect(screen.queryByText(/Consider parity\./)).not.toBeInTheDocument();

    unmount();
    await waitFor(() => {
      unlistens.forEach((fn) => expect(fn).toHaveBeenCalled());
    });
  });

  it('ignores obligation-scoped streams when no obligation is focused', async () => {
    const handlers = new Map<string, EventCallback>();
    listenMock.mockImplementation((name: string, callback: EventCallback) => {
      handlers.set(name, callback);
      return Promise.resolve(vi.fn());
    });

    const { ThinkingPanel } = await import('./ThinkingPanel');

    const { container } = render(<ThinkingPanel />);
    expect(container).toBeEmptyDOMElement();

    await act(async () => {
      handlers.get('loop:thinking_start')?.({
        payload: { step_number: 12, model: 'gpt-4o', agent_role: 'solver', obligation_id: 'obl-2' },
      });
      handlers.get('loop:token')?.({
        payload: { text: 'Hidden obligation token', obligation_id: 'obl-2' },
      });
    });

    expect(container).toBeEmptyDOMElement();

    await act(async () => {
      handlers.get('loop:thinking_start')?.({
        payload: { step_number: 13, model: 'gpt-4o', agent_role: 'solver' },
      });
      handlers.get('loop:token')?.({
        payload: { text: 'Visible global token' },
      });
    });

    expect(screen.getByText(/Visible global token/)).toBeInTheDocument();
    expect(screen.queryByText(/Hidden obligation token/)).not.toBeInTheDocument();
  });

  it('updates the header to the completed step number after a streamed step finishes', async () => {
    const handlers = new Map<string, EventCallback>();
    listenMock.mockImplementation((name: string, callback: EventCallback) => {
      handlers.set(name, callback);
      return Promise.resolve(vi.fn());
    });

    const { ThinkingPanel } = await import('./ThinkingPanel');

    render(<ThinkingPanel />);

    await act(async () => {
      handlers.get('loop:thinking_start')?.({
        payload: { step_number: 24, model: 'gpt-4o', agent_role: 'solver' },
      });
    });

    expect(screen.getByText(/Step 24/)).toHaveTextContent('gpt-4o');

    await act(async () => {
      handlers.get('loop:step_complete')?.({
        payload: {
          attempt_id: 'attempt-1',
          step_number: 25,
          proposal_type: 'lemma',
          proposal_natural: 'Finished step.',
          verified: true,
          model: 'gpt-4o',
        },
      });
    });

    expect(screen.getByText(/Step 25/)).toHaveTextContent('gpt-4o');
  });

  it('filters token streams by focused obligation and shows the focus badge', async () => {
    const handlers = new Map<string, EventCallback>();
    listenMock.mockImplementation((name: string, callback: EventCallback) => {
      handlers.set(name, callback);
      return Promise.resolve(vi.fn());
    });

    const { ThinkingPanel } = await import('./ThinkingPanel');

    const { container } = render(<ThinkingPanel focusObligationId="obl-1" />);
    expect(container).toBeEmptyDOMElement();

    await act(async () => {
      handlers.get('loop:thinking_start')?.({
        payload: { step_number: 1, model: 'gpt-4o', agent_role: 'solver', obligation_id: 'other' },
      });
    });

    expect(container).toBeEmptyDOMElement();

    await act(async () => {
      handlers.get('loop:thinking_start')?.({
        payload: { step_number: 3, model: 'gpt-4o', agent_role: 'reviewer', obligation_id: 'obl-1' },
      });
      handlers.get('loop:token')?.({ payload: { text: 'Accepted token', obligation_id: 'obl-1' } });
      handlers.get('loop:token')?.({ payload: { text: 'Ignored token', obligation_id: 'other' } });
    });

    expect(screen.getByText('FOCUSED')).toBeInTheDocument();
    expect(screen.getByText('REVIEWER')).toBeInTheDocument();
    expect(screen.getByText(/Accepted token/)).toBeInTheDocument();
    expect(screen.queryByText(/Ignored token/)).not.toBeInTheDocument();
  });
});
