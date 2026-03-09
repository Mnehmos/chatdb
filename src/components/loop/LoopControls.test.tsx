import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { MultiAgentConfig, ModelConfig, Problem } from '../../types';

let loopStoreState: Record<string, unknown>;
let problemStoreState: { activeProblem: Problem | null };

const useLoopStoreMock = Object.assign(vi.fn(() => loopStoreState), {
  getState: vi.fn(() => loopStoreState),
});
const useProblemStoreMock = vi.fn(() => problemStoreState);

vi.mock('../../stores/loopStore', () => ({
  MODEL_PRESETS: [
    {
      label: 'Sonnet 4.6',
      config: { provider: 'anthropic', model: 'claude-sonnet-4-6' },
    },
  ],
  useLoopStore: useLoopStoreMock,
}));

vi.mock('../../stores/problemStore', () => ({
  useProblemStore: useProblemStoreMock,
}));

vi.mock('../../services/tauri', () => ({
  runManualReview: vi.fn(),
}));

vi.mock('../analytics/AfterActionReport', () => ({
  AfterActionReport: () => null,
}));

vi.mock('../settings/AgentProfilePanel', () => ({
  AgentProfilePanel: () => null,
}));

vi.mock('../settings/ResearchApiPanel', () => ({
  ResearchApiPanel: () => null,
}));

const selectedModel: ModelConfig = {
  provider: 'anthropic',
  model: 'claude-sonnet-4-6',
  api_key_ref: 'ANTHROPIC_API_KEY',
  temperature: 0.3,
  max_budget_tokens: 50_000,
};

const fullConfig: MultiAgentConfig = {
  models: [selectedModel],
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
};

function createLoopStoreState(overrides: Record<string, unknown> = {}) {
  return {
    status: 'idle',
    startSolve: vi.fn(),
    continueSolve: vi.fn(),
    pause: vi.fn(),
    stop: vi.fn(),
    currentStep: 0,
    steps: [],
    selectedModel,
    activeProfile: null,
    attemptNumber: 1,
    maxAttempts: 5,
    warmupStatus: 'idle',
    adversaryModel: null,
    reviewerModel: null,
    discernerModel: null,
    lastError: null,
    fullConfig,
    ...overrides,
  };
}

function createActiveProblem(): Problem {
  return {
    id: 'problem-1',
    statement: 'Prove x^2 >= 0',
    domain: 'algebra',
    source: 'user',
    status: 'open',
    created_at: '2026-03-06T00:00:00Z',
    total_attempts: 0,
    total_steps: 0,
  };
}

describe('LoopControls', () => {
  beforeEach(() => {
    loopStoreState = createLoopStoreState();
    problemStoreState = { activeProblem: createActiveProblem() };
    useLoopStoreMock.mockImplementation(() => loopStoreState);
    useLoopStoreMock.getState.mockImplementation(() => loopStoreState);
    useProblemStoreMock.mockImplementation(() => problemStoreState);
  });

  it('shows the normal solve controls without an error banner when there is no loop error', async () => {
    const { LoopControls } = await import('./LoopControls');

    render(<LoopControls />);

    expect(screen.getByRole('button', { name: 'Solve' })).toBeInTheDocument();
    expect(screen.queryByText('DB locked')).not.toBeInTheDocument();
  });

  it('renders the latest loop error from the store so backend failures are visible', async () => {
    loopStoreState = createLoopStoreState({ lastError: 'DB locked' });
    useLoopStoreMock.mockImplementation(() => loopStoreState);
    useLoopStoreMock.getState.mockImplementation(() => loopStoreState);

    const { LoopControls } = await import('./LoopControls');

    render(<LoopControls />);

    expect(screen.getByText('DB locked')).toBeInTheDocument();
  });
});
