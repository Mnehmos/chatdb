import { create } from 'zustand';
import type { Step, LoopStatus, MultiAgentConfig, ModelConfig, AuditResult, ReviewResult, ExtractedPattern, ObligationEvent, CriticCheckEvent, AgentProfile, DiscernerFinding } from '../types';
import * as api from '../services/tauri';

type WarmupStatus = 'idle' | 'waiting' | 'ready' | 'timeout';

interface LoopStore {
  status: LoopStatus; attemptId: string | null; steps: Step[]; currentStep: number;
  selectedModel: ModelConfig;
  reviewerModel: ModelConfig | null;
  adversaryModel: ModelConfig | null;
  criticModel: ModelConfig | null;
  discernerModel: ModelConfig | null;
  decomposerModel: ModelConfig | null;
  fullConfig: MultiAgentConfig;
  activeProfile: AgentProfile | null;
  profiles: AgentProfile[];
  auditResults: AuditResult[];
  reviewResult: ReviewResult | null;
  reviewing: boolean;
  attemptNumber: number;
  maxAttempts: number;
  extractedPatterns: ExtractedPattern[];
  warmupStatus: WarmupStatus;
  obligations: ObligationEvent[];
  criticChecks: CriticCheckEvent[];
  discernerFindings: DiscernerFinding[];
  lastError: string | null;
  setExtractedPatterns: (p: ExtractedPattern[]) => void;
  setWarmupStatus: (s: WarmupStatus) => void;
  addObligation: (o: ObligationEvent) => void;
  updateObligation: (id: string, update: Partial<ObligationEvent>) => void;
  addCriticCheck: (c: CriticCheckEvent) => void;
  addDiscernerFinding: (f: DiscernerFinding) => void;
  startSolve: (pid: string) => Promise<void>;
  continueSolve: (pid: string, attemptId?: string) => Promise<void>;
  pause: () => Promise<void>;
  stop: () => Promise<void>; addStep: (s: Step) => void; setStatus: (s: LoopStatus) => void;
  setSelectedModel: (m: ModelConfig) => void;
  setReviewerModel: (m: ModelConfig | null) => void;
  setAdversaryModel: (m: ModelConfig | null) => void;
  setCriticModel: (m: ModelConfig | null) => void;
  setDiscernerModel: (m: ModelConfig | null) => void;
  setDecomposerModel: (m: ModelConfig | null) => void;
  setFullConfig: (c: MultiAgentConfig) => void;
  loadProfiles: () => Promise<void>;
  setActiveProfile: (p: AgentProfile | null) => void;
  saveCurrentAsProfile: (name: string, description?: string) => Promise<void>;
  deleteProfileById: (id: string) => Promise<void>;
  addAudit: (a: AuditResult) => void;
  setReview: (r: ReviewResult) => void;
  setReviewing: (b: boolean) => void;
  setAttemptInfo: (num: number, max: number, id: string) => void;
  resetForNewAttempt: (attemptId: string, attemptNumber: number) => void;
  loadSteps: (problemId: string) => Promise<void>;
  syncStatus: () => Promise<void>;
}

const CONFIG_STORAGE_KEY = 'chatdb_last_config';

function persistConfig(config: MultiAgentConfig) {
  try { localStorage.setItem(CONFIG_STORAGE_KEY, JSON.stringify(config)); } catch { /* ignore */ }
}

function loadPersistedConfig(): MultiAgentConfig | null {
  try {
    const raw = localStorage.getItem(CONFIG_STORAGE_KEY);
    if (raw) return JSON.parse(raw) as MultiAgentConfig;
  } catch { /* ignore */ }
  return null;
}

const DEFAULT_CONFIG: MultiAgentConfig = {
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
  scout_sources: ['arxiv', 'semantic_scholar', 'oeis', 'wolfram', 'loogle', 'tavily'],
  use_patterns: true,
  allow_self_modify: false,
  max_attempts: 5,
  min_exploration_coverage: 0.6,
  min_conclusion_confidence: 0.7,
};

export const MODEL_PRESETS: { label: string; config: ModelConfig }[] = [
  { label: 'Sonnet 4.6', config: { provider: 'anthropic', model: 'claude-sonnet-4-6', api_key_ref: 'ANTHROPIC_API_KEY', temperature: 0.3, max_budget_tokens: 50_000 } },
  { label: 'Opus 4.6', config: { provider: 'anthropic', model: 'claude-opus-4-6', api_key_ref: 'ANTHROPIC_API_KEY', temperature: 0.3, max_budget_tokens: 100_000 } },
  { label: 'Haiku 4.5', config: { provider: 'anthropic', model: 'claude-haiku-4-5-20251001', api_key_ref: 'ANTHROPIC_API_KEY', temperature: 0.3, max_budget_tokens: 20_000 } },
  { label: 'GPT-4o (OpenAI)', config: { provider: 'openai', model: 'gpt-4o', api_key_ref: 'OPENAI_API_KEY', temperature: 0.3, max_budget_tokens: 50_000 } },
  { label: 'o3-mini (OpenAI)', config: { provider: 'openai', model: 'o3-mini', api_key_ref: 'OPENAI_API_KEY', temperature: 0.3, max_budget_tokens: 50_000 } },
  { label: 'GPT-5 Nano (OpenAI)', config: { provider: 'openai', model: 'gpt-5-nano', api_key_ref: 'OPENAI_API_KEY', temperature: 0.2, max_budget_tokens: 50_000 } },
  { label: 'Sonnet 4.6 (OpenRouter)', config: { provider: 'openrouter', model: 'anthropic/claude-sonnet-4-6', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.3, max_budget_tokens: 50_000 } },
  { label: 'GPT-4o (OpenRouter)', config: { provider: 'openrouter', model: 'openai/gpt-4o', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.3, max_budget_tokens: 50_000 } },
  { label: 'GPT-5 Nano (OpenRouter)', config: { provider: 'openrouter', model: 'openai/gpt-5-nano', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.2, max_budget_tokens: 50_000 } },
  { label: 'GPT OSS 20B (OpenRouter)', config: { provider: 'openrouter', model: 'openai/gpt-oss-20b', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'GPT OSS 120B Nitro (OpenRouter)', config: { provider: 'openrouter', model: 'openai/gpt-oss-120b:nitro', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'Gemini 3 Flash (Gemini API)', config: { provider: 'gemini', model: 'gemini-3-flash-preview', api_key_ref: 'GEMINI_API_KEY', temperature: 0.2, max_budget_tokens: 50_000 } },
  { label: 'Gemini 3.1 Pro (Gemini API)', config: { provider: 'gemini', model: 'gemini-3.1-pro-preview', api_key_ref: 'GEMINI_API_KEY', temperature: 0.2, max_budget_tokens: 100_000 } },
  { label: 'Gemini 3 Flash (OpenRouter)', config: { provider: 'openrouter', model: 'google/gemini-3-flash-preview', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.2, max_budget_tokens: 50_000 } },
  { label: 'Gemini 3.1 Pro (OpenRouter)', config: { provider: 'openrouter', model: 'google/gemini-3.1-pro-preview', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.2, max_budget_tokens: 100_000 } },
  { label: 'Gemini 3.1 Flash Lite (OpenRouter)', config: { provider: 'openrouter', model: 'google/gemini-3.1-flash-lite-preview', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.2, max_budget_tokens: 50_000 } },
  { label: 'MiniMax M2.5 (OpenRouter)', config: { provider: 'openrouter', model: 'minimax/minimax-m2.5', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'DeepSeek V3.2 (OpenRouter)', config: { provider: 'openrouter', model: 'deepseek/deepseek-v3.2', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'GLM-5 (OpenRouter)', config: { provider: 'openrouter', model: 'z-ai/glm-5', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.3, max_budget_tokens: 50_000 } },
  { label: 'Kimi K2.5 (OpenRouter)', config: { provider: 'openrouter', model: 'moonshotai/kimi-k2.5', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'Qwen3 235B A22B (OpenRouter)', config: { provider: 'openrouter', model: 'qwen/qwen3-235b-a22b-2507', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'Qwen3.5 27B (OpenRouter)', config: { provider: 'openrouter', model: 'qwen/qwen3.5-27b', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'Qwen3.5 35B A3B (OpenRouter)', config: { provider: 'openrouter', model: 'qwen/qwen3.5-35b-a3b', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'Qwen3.5 397B A17B (OpenRouter)', config: { provider: 'openrouter', model: 'qwen/qwen3.5-397b-a17b', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'Qwen3.5 Plus 02-15 (OpenRouter)', config: { provider: 'openrouter', model: 'qwen/qwen3.5-plus-02-15', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'Mistral Nemo (OpenRouter)', config: { provider: 'openrouter', model: 'mistralai/mistral-nemo', api_key_ref: 'OPENROUTER_API_KEY', temperature: 0.4, max_budget_tokens: 50_000 } },
  { label: 'GPT-5.2 (OpenAI)', config: { provider: 'openai', model: 'gpt-5.2-2025-12-11', api_key_ref: 'OPENAI_API_KEY', temperature: 0.3, max_budget_tokens: 100_000 } },
  { label: 'GPT-5.4 (ChatGPT Sub)', config: { provider: 'chatgpt', model: 'gpt-5.4', api_key_ref: 'CHATGPT_OAUTH_TOKEN', temperature: 0.3, max_budget_tokens: 100_000 } },
  { label: 'GPT-5.3 Codex (ChatGPT Sub)', config: { provider: 'chatgpt', model: 'gpt-5.3-codex', api_key_ref: 'CHATGPT_OAUTH_TOKEN', temperature: 0.3, max_budget_tokens: 100_000 } },
  { label: 'GPT-5.3 Codex Spark (ChatGPT Sub)', config: { provider: 'chatgpt', model: 'gpt-5.3-codex-spark', api_key_ref: 'CHATGPT_OAUTH_TOKEN', temperature: 0.2, max_budget_tokens: 50_000 } },
  { label: 'GPT-5.2 Codex (ChatGPT Sub)', config: { provider: 'chatgpt', model: 'gpt-5.2-codex', api_key_ref: 'CHATGPT_OAUTH_TOKEN', temperature: 0.3, max_budget_tokens: 100_000 } },
];

const _persisted = loadPersistedConfig();
const _initConfig = _persisted || { ...DEFAULT_CONFIG };

export const useLoopStore = create<LoopStore>((set, get) => ({
  status: 'idle', attemptId: null, steps: [], currentStep: 0,
  selectedModel: _initConfig.models[0] || MODEL_PRESETS[0].config,
  reviewerModel: _initConfig.reviewer_model || null,
  adversaryModel: _initConfig.adversary_model || null,
  criticModel: _initConfig.critic_model || null,
  discernerModel: _initConfig.discerner_model || null,
  decomposerModel: _initConfig.decomposer_model || null,
  fullConfig: _initConfig,
  activeProfile: null,
  profiles: [],
  auditResults: [], reviewResult: null, reviewing: false,
  attemptNumber: 1, maxAttempts: 5, extractedPatterns: [], warmupStatus: 'idle' as WarmupStatus,
  obligations: [], criticChecks: [], discernerFindings: [], lastError: null,
  setExtractedPatterns: (p) => set({ extractedPatterns: p }),
  setWarmupStatus: (s) => set({ warmupStatus: s }),
  addObligation: (o) => set((s) => ({ obligations: [...s.obligations, o] })),
  updateObligation: (id, update) => set((s) => ({
    obligations: s.obligations.map(o => o.id === id ? { ...o, ...update } : o),
  })),
  addCriticCheck: (c) => set((s) => ({ criticChecks: [...s.criticChecks, c] })),
  addDiscernerFinding: (f) => set((s) => ({ discernerFindings: [...s.discernerFindings, f] })),
  startSolve: async (problemId) => {
    try {
      set({ status: 'running', steps: [], currentStep: 0, auditResults: [], reviewResult: null, reviewing: false, attemptNumber: 1, extractedPatterns: [], warmupStatus: 'idle' as WarmupStatus, obligations: [], criticChecks: [], discernerFindings: [], lastError: null });
      const config = get().fullConfig;
      const id = await api.startSolve(problemId, config, true);
      set({ attemptId: id });
    } catch (e: any) { console.error('startSolve error:', e); set({ status: 'idle', lastError: e?.message || String(e) }); }
  },
  continueSolve: async (problemId, attemptId?) => {
    try {
      const config = get().fullConfig;
      const id = await api.continueSolve(problemId, config, attemptId);
      set({ status: 'running', attemptId: id, lastError: null });
    } catch (e: any) { console.error('continueSolve error:', e); set({ status: 'idle', lastError: e?.message || String(e) }); }
  },
  pause: async () => {
    try { await api.pauseSolve(); set({ status: 'paused' }); }
    catch (e: any) { console.error('pause error:', e); set({ lastError: e?.message || String(e) }); }
  },
  stop: async () => {
    try { await api.stopSolve(); set({ status: 'idle' }); }
    catch (e: any) { console.error('stop error:', e); set({ lastError: e?.message || String(e) }); }
  },
  addStep: (step) => set((s) => {
    // Upsert by step_number: if a step with the same number exists (e.g., adversarial
    // downgrade), replace it instead of appending a duplicate.
    const idx = s.steps.findIndex(
      (existing) => existing.step_number === step.step_number && existing.attempt_id === step.attempt_id
    );
    if (idx >= 0) {
      const updated = [...s.steps];
      updated[idx] = step;
      return { steps: updated, currentStep: step.step_number };
    }
    return { steps: [...s.steps, step], currentStep: step.step_number };
  }),
  setStatus: (status) => set({ status }),
  setSelectedModel: (m) => {
    set({ selectedModel: m });
    set((s) => { const c = { ...s.fullConfig, models: [m] }; persistConfig(c); return { fullConfig: c }; });
  },
  setReviewerModel: (m) => {
    set({ reviewerModel: m });
    set((s) => { const c = { ...s.fullConfig, reviewer_model: m || undefined }; persistConfig(c); return { fullConfig: c }; });
  },
  setAdversaryModel: (m) => {
    set({ adversaryModel: m });
    set((s) => { const c = { ...s.fullConfig, adversary_model: m || undefined }; persistConfig(c); return { fullConfig: c }; });
  },
  setCriticModel: (m) => {
    set({ criticModel: m });
    set((s) => { const c = { ...s.fullConfig, critic_model: m || undefined }; persistConfig(c); return { fullConfig: c }; });
  },
  setDiscernerModel: (m) => {
    set({ discernerModel: m });
    set((s) => { const c = { ...s.fullConfig, discerner_model: m || undefined }; persistConfig(c); return { fullConfig: c }; });
  },
  setDecomposerModel: (m) => {
    set({ decomposerModel: m });
    set((s) => { const c = { ...s.fullConfig, decomposer_model: m || undefined }; persistConfig(c); return { fullConfig: c }; });
  },
  setFullConfig: (c) => {
    persistConfig(c);
    set({
      fullConfig: c,
      selectedModel: c.models[0] || MODEL_PRESETS[0].config,
      reviewerModel: c.reviewer_model || null,
      adversaryModel: c.adversary_model || null,
      criticModel: c.critic_model || null,
      discernerModel: c.discerner_model || null,
      decomposerModel: c.decomposer_model || null,
    });
  },
  loadProfiles: async () => {
    try {
      const profiles = await api.listProfiles();
      set({ profiles });
      const defaultProfile = profiles.find(p => p.is_default);
      if (defaultProfile) {
        const config = JSON.parse(defaultProfile.config_json) as MultiAgentConfig;
        set({
          activeProfile: defaultProfile,
          fullConfig: config,
          selectedModel: config.models[0] || MODEL_PRESETS[0].config,
          reviewerModel: config.reviewer_model || null,
          adversaryModel: config.adversary_model || null,
          criticModel: config.critic_model || null,
          discernerModel: config.discerner_model || null,
        });
      }
    } catch { /* no profiles yet — first run */ }
  },
  setActiveProfile: (p) => {
    if (p) {
      try {
        const config = JSON.parse(p.config_json) as MultiAgentConfig;
        set({
          activeProfile: p,
          fullConfig: { ...DEFAULT_CONFIG, ...config },
          selectedModel: config.models?.[0] || MODEL_PRESETS[0].config,
          reviewerModel: config.reviewer_model || null,
          adversaryModel: config.adversary_model || null,
          criticModel: config.critic_model || null,
          discernerModel: config.discerner_model || null,
        });
      } catch (e) {
        console.error('[profile] Failed to parse config_json:', e);
        set({ activeProfile: p });
      }
    } else {
      set({ activeProfile: null });
    }
  },
  saveCurrentAsProfile: async (name, description) => {
    const { fullConfig, profiles } = get();
    // Sanitize: replace undefined with null so JSON.stringify includes the keys.
    // Rust serde treats null as None for Option<T>, but JSON.stringify drops undefined entirely.
    const sanitized = {
      ...fullConfig,
      reviewer_model: fullConfig.reviewer_model ?? null,
      adversary_model: fullConfig.adversary_model ?? null,
      critic_model: fullConfig.critic_model ?? null,
      discerner_model: fullConfig.discerner_model ?? null,
      decomposer_model: fullConfig.decomposer_model ?? null,
    };
    const configJson = JSON.stringify(sanitized);
    console.log('[profile-save] name:', name, 'json_len:', configJson.length, 'models:', sanitized.models.length);
    const saved = await api.saveProfile(name, configJson, description);
    console.log('[profile-save] success:', saved.id, saved.name);
    set({ activeProfile: saved, profiles: [...profiles.filter(p => p.id !== saved.id), saved] });
  },
  deleteProfileById: async (id) => {
    await api.deleteProfile(id);
    const { profiles, activeProfile } = get();
    set({
      profiles: profiles.filter(p => p.id !== id),
      activeProfile: activeProfile?.id === id ? null : activeProfile,
    });
  },
  addAudit: (a) => set((s) => ({ auditResults: [...s.auditResults, a] })),
  setReview: (r) => set({ reviewResult: r, reviewing: false }),
  setReviewing: (b) => set({ reviewing: b }),
  setAttemptInfo: (num, max, id) => set({ attemptNumber: num, maxAttempts: max, attemptId: id }),
  resetForNewAttempt: (attemptId, attemptNumber) => set({
    currentStep: 0, attemptId, attemptNumber,
    auditResults: [], reviewResult: null, reviewing: false, status: 'running',
    obligations: [], criticChecks: [], discernerFindings: [],
  }),
  loadSteps: async (problemId) => {
    try {
      // Now returns ALL steps across ALL attempts for continuity
      const steps = await api.getProblemSteps(problemId);
      const last = steps[steps.length - 1];
      // Count distinct attempts for attempt numbering
      const attemptIds = [...new Set(steps.map(s => s.attempt_id))];
      const attemptNum = attemptIds.length || 1;
      // Check actual backend status to avoid desync
      const { running, attempt_id } = await api.getLoopStatus();
      if (running) {
        set({ steps, currentStep: last?.step_number ?? 0, status: 'running', attemptId: attempt_id, attemptNumber: attemptNum });
      } else {
        set({ steps, currentStep: last?.step_number ?? 0, status: steps.length > 0 ? 'finished' : 'idle', attemptNumber: attemptNum });
      }
    } catch {
      set({ steps: [], currentStep: 0 });
    }
  },
  syncStatus: async () => {
    try {
      const { running, attempt_id } = await api.getLoopStatus();
      if (running) {
        set({ status: 'running', attemptId: attempt_id });
      } else if (get().status === 'running') {
        // Backend stopped but we thought it was running — mark finished
        set({ status: 'finished' });
      }
    } catch { /* ignore */ }
  },
}));
