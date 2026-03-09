import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let loopStoreState: Record<string, unknown>;

const useLoopStoreMock = Object.assign(
  vi.fn(() => loopStoreState),
  {
    setState: vi.fn(),
  },
);

vi.mock('../../stores/loopStore', () => ({
  MODEL_PRESETS: [
    {
      label: 'Sonnet 4.6',
      config: { provider: 'anthropic', model: 'claude-sonnet-4-6', api_key_ref: 'ANTHROPIC_API_KEY', temperature: 0.3, max_budget_tokens: 50_000 },
    },
  ],
  useLoopStore: useLoopStoreMock,
}));

vi.mock('../../services/tauri', () => ({
  testProfileRoundtrip: vi.fn(),
  saveProfile: vi.fn(),
  listProfiles: vi.fn(),
}));

describe('AgentProfilePanel', () => {
  beforeEach(() => {
    loopStoreState = {
      activeProfile: null,
      profiles: [{ id: 'profile-1', name: 'Default', is_default: true, config_json: '{}', created_at: '', updated_at: '' }],
      fullConfig: {
        models: [{ provider: 'anthropic', model: 'claude-sonnet-4-6', api_key_ref: 'ANTHROPIC_API_KEY', temperature: 0.3, max_budget_tokens: 50_000 }],
        reviewer_model: undefined,
        adversary_model: undefined,
        critic_model: undefined,
        discerner_model: undefined,
        decomposer_model: undefined,
        max_total_cost: 100_000,
        failure_threshold: 5,
        use_critic: false,
        critic_skip_threshold: 0.8,
        use_council: false,
        council_models: [],
        scout_sources: [],
        use_patterns: true,
        allow_self_modify: false,
        max_attempts: 5,
        min_exploration_coverage: 0.6,
        min_conclusion_confidence: 0.7,
      },
      setFullConfig: vi.fn(),
      loadProfiles: vi.fn(),
      setActiveProfile: vi.fn(),
      deleteProfileById: vi.fn(),
    };
    useLoopStoreMock.mockImplementation(() => loopStoreState);
    useLoopStoreMock.setState.mockReset();
  });

  it('does not render when closed', async () => {
    const { AgentProfilePanel } = await import('./AgentProfilePanel');

    const { container } = render(<AgentProfilePanel open={false} onClose={vi.fn()} />);

    expect(container).toBeEmptyDOMElement();
  });

  it('loads profiles, saves a new profile, and forwards profile selection', async () => {
    const api = await import('../../services/tauri');
    vi.mocked(api.saveProfile).mockResolvedValue({
      id: 'profile-2',
      name: 'Alpha',
      is_default: false,
      config_json: '{}',
      created_at: '',
      updated_at: '',
    } as any);
    vi.mocked(api.listProfiles).mockResolvedValue([
      { id: 'profile-1', name: 'Default', is_default: true, config_json: '{}', created_at: '', updated_at: '' },
      { id: 'profile-2', name: 'Alpha', is_default: false, config_json: '{}', created_at: '', updated_at: '' },
    ] as any);

    const { AgentProfilePanel } = await import('./AgentProfilePanel');

    render(<AgentProfilePanel open={true} onClose={vi.fn()} />);

    expect(loopStoreState.loadProfiles).toHaveBeenCalled();

    fireEvent.change(screen.getByDisplayValue('-- Custom --'), {
      target: { value: 'profile-1' },
    });
    expect(loopStoreState.setActiveProfile).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'profile-1' }),
    );

    fireEvent.change(screen.getByPlaceholderText('Profile name...'), {
      target: { value: 'Alpha' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(api.saveProfile).toHaveBeenCalled();
    });

    expect(api.saveProfile).toHaveBeenCalledWith(
      'Alpha',
      expect.stringContaining('"reviewer_model":null'),
    );
    expect(api.listProfiles).toHaveBeenCalled();
    expect(useLoopStoreMock.setState).toHaveBeenCalledWith({
      activeProfile: expect.objectContaining({ id: 'profile-2', name: 'Alpha' }),
      profiles: expect.arrayContaining([expect.objectContaining({ id: 'profile-2' })]),
    });
    expect(screen.getByText(/Saved "Alpha"/)).toBeInTheDocument();
  });
});
