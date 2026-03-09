import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock Tauri invoke before importing the store
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { useLoopStore } from '../loopStore';

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
  // Reset store to initial state
  useLoopStore.setState({
    status: 'idle',
    steps: [],
    currentStep: 0,
    attemptId: null,
    lastError: null,
  });
});

describe('loopStore error handling', () => {
  it('startSolve surfaces error in lastError on IPC failure', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('DB locked'));
    await useLoopStore.getState().startSolve('problem-1');

    const state = useLoopStore.getState();
    expect(state.status).toBe('idle');
    expect(state.lastError).toBe('DB locked');
  });

  it('continueSolve surfaces error and resets status on failure', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('No such attempt'));
    await useLoopStore.getState().continueSolve('problem-1');

    const state = useLoopStore.getState();
    expect(state.status).toBe('idle');
    expect(state.lastError).toBe('No such attempt');
  });

  it('pause surfaces error on IPC failure', async () => {
    useLoopStore.setState({ status: 'running' });
    mockedInvoke.mockRejectedValueOnce(new Error('Not running'));
    await useLoopStore.getState().pause();

    const state = useLoopStore.getState();
    // Status should not change to 'paused' on error
    expect(state.status).toBe('running');
    expect(state.lastError).toBe('Not running');
  });

  it('stop surfaces error on IPC failure', async () => {
    useLoopStore.setState({ status: 'running' });
    mockedInvoke.mockRejectedValueOnce(new Error('Already stopped'));
    await useLoopStore.getState().stop();

    const state = useLoopStore.getState();
    expect(state.status).toBe('running');
    expect(state.lastError).toBe('Already stopped');
  });

  it('startSolve clears previous error on success', async () => {
    useLoopStore.setState({ lastError: 'previous error' });
    mockedInvoke.mockResolvedValueOnce('attempt-123');
    await useLoopStore.getState().startSolve('problem-1');

    const state = useLoopStore.getState();
    expect(state.lastError).toBeNull();
    expect(state.attemptId).toBe('attempt-123');
  });
});
