import { beforeEach, describe, expect, it, vi } from 'vitest';

const listenMock = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

describe('events service', () => {
  beforeEach(() => {
    listenMock.mockReset();
    vi.resetModules();
  });

  it('registers listeners and cleans them up', async () => {
    const unlisten = vi.fn();
    listenMock.mockImplementation((_name, _cb) => Promise.resolve(unlisten));

    const { onLoopEvent } = await import('./events');
    const stop = onLoopEvent(() => {});

    expect(listenMock).toHaveBeenCalled();
    expect(listenMock).toHaveBeenCalledWith('loop:started', expect.any(Function));

    stop();
    await Promise.resolve();
    await Promise.resolve();

    expect(unlisten).toHaveBeenCalledTimes(listenMock.mock.calls.length);
  });

  it('maps valid tauri payloads into typed app events', async () => {
    let capturedCallback: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation((name: string, callback: (event: { payload: unknown }) => void) => {
      if (name === 'loop:step_complete') {
        capturedCallback = callback;
      }
      return Promise.resolve(() => {});
    });

    const handler = vi.fn();
    const { onLoopEvent } = await import('./events');
    onLoopEvent(handler);

    capturedCallback?.({
      payload: {
        attempt_id: 'attempt-1',
        step_number: 2,
        proposal_type: 'lemma',
        proposal_natural: 'A valid step',
        verified: true,
        model: 'solver-a',
      },
    });

    expect(handler).toHaveBeenCalledWith({
      type: 'loop:step_complete',
      payload: {
        attempt_id: 'attempt-1',
        step_number: 2,
        proposal_type: 'lemma',
        proposal_natural: 'A valid step',
        verified: true,
        model: 'solver-a',
      },
    });
  });

  it('drops invalid payloads at the event boundary and warns', async () => {
    let capturedCallback: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation((name: string, callback: (event: { payload: unknown }) => void) => {
      if (name === 'loop:step_complete') {
        capturedCallback = callback;
      }
      return Promise.resolve(() => {});
    });

    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const handler = vi.fn();
    const { listenKnownEvent } = await import('./events');

    await listenKnownEvent('loop:step_complete', handler);
    capturedCallback?.({ payload: { step_number: 2 } });

    expect(handler).not.toHaveBeenCalled();
    expect(warnSpy).toHaveBeenCalledWith(
      '[events] ignored invalid payload for loop:step_complete',
      { step_number: 2 },
    );
  });

  it('accepts newly contracted debug events', async () => {
    let capturedCallback: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation((name: string, callback: (event: { payload: unknown }) => void) => {
      if (name === 'loop:answer_mismatch') {
        capturedCallback = callback;
      }
      return Promise.resolve(() => {});
    });

    const handler = vi.fn();
    const { listenKnownEvent } = await import('./events');

    await listenKnownEvent('loop:answer_mismatch', handler);
    capturedCallback?.({
      payload: {
        step_number: 11,
        proposed_answer: 'c = 3',
        known_answer: 'c = 4',
      },
    });

    expect(handler).toHaveBeenCalledWith({
      step_number: 11,
      proposed_answer: 'c = 3',
      known_answer: 'c = 4',
    });
  });
});
