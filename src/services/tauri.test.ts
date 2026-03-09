import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

describe('tauri service wrappers', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('createProblem forwards the command name and payload', async () => {
    const expected = { id: 'problem-1' };
    invokeMock.mockResolvedValue(expected);

    const { createProblem } = await import('./tauri');
    const result = await createProblem('Prove x^2 >= 0', 'algebra');

    expect(invokeMock).toHaveBeenCalledWith('create_problem', {
      statement: 'Prove x^2 >= 0',
      domain: 'algebra',
    });
    expect(result).toBe(expected);
  });

  it('saveProfile normalizes undefined optionals to null for Tauri IPC', async () => {
    const expected = { id: 'profile-1' };
    invokeMock.mockResolvedValue(expected);

    const { saveProfile } = await import('./tauri');
    const result = await saveProfile('default', '{"models":[]}');

    expect(invokeMock).toHaveBeenCalledWith('save_profile', {
      name: 'default',
      configJson: '{"models":[]}',
      description: null,
      isDefault: null,
    });
    expect(result).toBe(expected);
  });

  it('getLoopStatus uses the expected backend command name', async () => {
    invokeMock.mockResolvedValue({ running: true, attempt_id: 'attempt-1' });

    const { getLoopStatus } = await import('./tauri');
    await getLoopStatus();

    expect(invokeMock).toHaveBeenCalledWith('get_loop_status');
  });
});
