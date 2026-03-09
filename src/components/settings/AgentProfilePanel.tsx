import { useState, useEffect } from 'react';
import { useLoopStore, MODEL_PRESETS } from '../../stores/loopStore';
import { testProfileRoundtrip, saveProfile as apiSaveProfile, listProfiles as apiListProfiles } from '../../services/tauri';
import type { MultiAgentConfig, ModelConfig } from '../../types';

const BUDGET_STOPS = [2000, 4000, 8000, 16000, 32000, 50000, 100000];
const BUDGET_LABELS = ['2k', '4k', '8k', '16k', '32k', '50k', '100k'];

function budgetToIndex(budget: number): number {
  let closest = 0;
  for (let i = 0; i < BUDGET_STOPS.length; i++) {
    if (Math.abs(BUDGET_STOPS[i] - budget) < Math.abs(BUDGET_STOPS[closest] - budget)) closest = i;
  }
  return closest;
}

function BudgetSlider({ value, onChange, label }: { value: number; onChange: (v: number) => void; label: string }) {
  const idx = budgetToIndex(value);
  return (
    <div className="budget-slider-group">
      <label className="budget-label">{label}</label>
      <div className="budget-slider-row">
        <input type="range" min={0} max={BUDGET_STOPS.length - 1} step={1} value={idx}
          onChange={(e) => onChange(BUDGET_STOPS[Number(e.target.value)])} className="budget-slider" />
        <span className="budget-value">{BUDGET_LABELS[idx]}</span>
      </div>
      <div className="budget-ticks">
        {BUDGET_LABELS.map((l, i) => <span key={i} className="budget-tick">{l}</span>)}
      </div>
    </div>
  );
}

function TemperatureSlider({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  return (
    <div className="slider-row">
      <label>Temperature</label>
      <input type="range" min={0} max={1} step={0.05} value={value}
        onChange={(e) => onChange(Number(e.target.value))} />
      <span>{value.toFixed(2)}</span>
    </div>
  );
}

function ModelSelector({ config, onChange, label }: { config: ModelConfig; onChange: (m: ModelConfig) => void; label: string }) {
  const idx = MODEL_PRESETS.findIndex(p => p.config.provider === config.provider && p.config.model === config.model);
  return (
    <div className="slider-row">
      <label>{label}</label>
      <select className="model-select" value={idx >= 0 ? idx : 0}
        onChange={(e) => {
          const preset = MODEL_PRESETS[Number(e.target.value)].config;
          onChange({ ...preset, temperature: config.temperature, max_budget_tokens: config.max_budget_tokens });
        }}>
        {MODEL_PRESETS.map((p, i) => <option key={i} value={i}>{p.label}</option>)}
      </select>
    </div>
  );
}

function Toggle({ active, onToggle, label }: { active: boolean; onToggle: () => void; label: string }) {
  return (
    <div className="config-toggle">
      <label>{label}</label>
      <div className={`toggle-switch ${active ? 'active' : ''}`} onClick={onToggle} />
    </div>
  );
}

interface Props {
  open: boolean;
  onClose: () => void;
}

export function AgentProfilePanel({ open, onClose }: Props) {
  const {
    activeProfile, profiles, fullConfig, setFullConfig,
    loadProfiles, setActiveProfile, deleteProfileById,
  } = useLoopStore();

  const [profileName, setProfileName] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [useCustomReviewer, setUseCustomReviewer] = useState(!!fullConfig.reviewer_model);
  const [useAdversary, setUseAdversary] = useState(!!fullConfig.adversary_model);
  const [useCritic, setUseCritic] = useState(!!fullConfig.critic_model);
  const [useDiscerner, setUseDiscerner] = useState(!!fullConfig.discerner_model);
  const [saveStatus, setSaveStatus] = useState<{ type: 'idle' | 'saving' | 'ok' | 'error'; msg?: string }>({ type: 'idle' });

  useEffect(() => { loadProfiles(); }, []);
  useEffect(() => {
    setUseCustomReviewer(!!fullConfig.reviewer_model);
    setUseAdversary(!!fullConfig.adversary_model);
    setUseCritic(!!fullConfig.critic_model);
    setUseDiscerner(!!fullConfig.discerner_model);
  }, [fullConfig]);

  if (!open) return null;

  const config = fullConfig;
  const solver = config.models[0] || MODEL_PRESETS[0].config;

  const update = (patch: Partial<MultiAgentConfig>) => {
    setFullConfig({ ...config, ...patch });
  };

  const updateSolver = (patch: Partial<ModelConfig>) => {
    update({ models: [{ ...solver, ...patch }] });
  };

  const reviewer = config.reviewer_model || { ...solver };
  const adversary = config.adversary_model || { ...MODEL_PRESETS[0].config };
  const critic = config.critic_model || { ...solver };
  const discerner = config.discerner_model || {
    ...MODEL_PRESETS[0].config,
    temperature: 0.05,
    max_budget_tokens: 4000,
  };

  return (
    <div className="profile-panel-overlay" onClick={onClose}>
      <div className="profile-panel" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="profile-panel-header">
          <h3>Agent Configuration</h3>
          <button className="btn btn-sm" onClick={onClose}>Close</button>
        </div>

        {/* Profile Bar */}
        <div className="profile-bar">
          <select className="profile-select"
            value={activeProfile?.id || ''}
            onChange={(e) => {
              const p = profiles.find(pr => pr.id === e.target.value);
              setActiveProfile(p || null);
            }}>
            <option value="">-- Custom --</option>
            {profiles.map(p => (
              <option key={p.id} value={p.id}>{p.name}{p.is_default ? ' *' : ''}</option>
            ))}
          </select>
          <input type="text" className="profile-name-input" placeholder="Profile name..."
            value={profileName} onChange={(e) => setProfileName(e.target.value)} />
          <button className="btn btn-sm btn-primary"
            disabled={!profileName.trim() || saveStatus.type === 'saving'}
            onClick={async () => {
              const name = profileName.trim();
              setSaveStatus({ type: 'saving', msg: 'Preparing config...' });
              try {
                // 1. Build config JSON directly (bypass store)
                const cfg = { ...fullConfig };
                const sanitized: Record<string, unknown> = {
                  ...cfg,
                  reviewer_model: cfg.reviewer_model ?? null,
                  adversary_model: cfg.adversary_model ?? null,
                  critic_model: cfg.critic_model ?? null,
                  discerner_model: cfg.discerner_model ?? null,
                  decomposer_model: cfg.decomposer_model ?? null,
                  scout_sources: cfg.scout_sources ?? [],
                };
                const configJson = JSON.stringify(sanitized);
                console.log('[profile-save] name:', name);
                console.log('[profile-save] json_len:', configJson.length);
                console.log('[profile-save] models:', (cfg.models || []).length);
                console.log('[profile-save] config preview:', configJson.slice(0, 300));

                // 2. Call Tauri directly
                setSaveStatus({ type: 'saving', msg: 'Calling save_profile...' });
                const saved = await apiSaveProfile(name, configJson);
                console.log('[profile-save] SUCCESS:', saved);

                // 3. Refresh profiles list
                setSaveStatus({ type: 'saving', msg: 'Refreshing profiles...' });
                const allProfiles = await apiListProfiles();
                console.log('[profile-save] profiles after save:', allProfiles.length);

                // 4. Update store
                useLoopStore.setState({
                  activeProfile: saved,
                  profiles: allProfiles,
                });

                setSaveStatus({ type: 'ok', msg: `Saved "${name}" (id: ${saved.id.slice(0, 8)})` });
                setProfileName('');
                setTimeout(() => setSaveStatus({ type: 'idle' }), 5000);
              } catch (e: unknown) {
                const msg = e instanceof Error ? e.message : String(e);
                console.error('[profile-save] FAILED:', e);
                console.error('[profile-save] Error type:', typeof e);
                console.error('[profile-save] Error details:', JSON.stringify(e, null, 2));
                setSaveStatus({ type: 'error', msg: `Save failed: ${msg}` });
              }
            }}>
            {saveStatus.type === 'saving' ? 'Saving...' : 'Save'}
          </button>
          {activeProfile && (
            <button className="btn btn-sm btn-danger" onClick={() => deleteProfileById(activeProfile.id)}>
              Del
            </button>
          )}
          <button className="btn btn-sm" title="Test profile save round-trip"
            onClick={async () => {
              setSaveStatus({ type: 'saving', msg: 'Running diagnostic...' });
              try {
                const raw = await testProfileRoundtrip();
                const result = JSON.parse(raw);
                if (result.passed) {
                  setSaveStatus({ type: 'ok', msg: `Diagnostic passed (${result.profiles_count} profiles)` });
                } else {
                  setSaveStatus({ type: 'error', msg: `Diagnostic failed: ${result.issues.join(', ')}` });
                }
                setTimeout(() => setSaveStatus({ type: 'idle' }), 5000);
              } catch (e) {
                setSaveStatus({ type: 'error', msg: `Diagnostic error: ${e}` });
              }
            }}>
            Test
          </button>
        </div>
        {saveStatus.type !== 'idle' && (
          <div className={`profile-save-status profile-save-${saveStatus.type}`}
            style={saveStatus.type === 'error' ? { background: '#3a1111', border: '1px solid #ff4444', padding: '8px 12px', margin: '8px 0', borderRadius: '4px', whiteSpace: 'pre-wrap', fontSize: '12px' } : undefined}>
            {saveStatus.type === 'saving' && (saveStatus.msg || 'Saving...')}
            {saveStatus.type === 'ok' && saveStatus.msg}
            {saveStatus.type === 'error' && (
              <div>
                <strong style={{ color: '#ff6666' }}>Error:</strong>{' '}
                <span style={{ color: '#ffaaaa' }}>{saveStatus.msg}</span>
                <br />
                <button className="btn btn-sm" style={{ marginTop: '6px' }} onClick={() => setSaveStatus({ type: 'idle' })}>Dismiss</button>
              </div>
            )}
          </div>
        )}

        {/* Solver Section */}
        <section className="agent-config-section">
          <h4>Solver</h4>
          <ModelSelector config={solver} onChange={(m) => update({ models: [m] })} label="Model" />
          <TemperatureSlider value={solver.temperature} onChange={(t) => updateSolver({ temperature: t })} />
          <BudgetSlider value={solver.max_budget_tokens} onChange={(b) => updateSolver({ max_budget_tokens: b })} label="Token Budget" />
        </section>

        {/* Reviewer Section */}
        <section className="agent-config-section">
          <h4>Reviewer</h4>
          <Toggle active={useCustomReviewer} label="Custom reviewer model"
            onToggle={() => {
              if (useCustomReviewer) {
                update({ reviewer_model: undefined });
                setUseCustomReviewer(false);
              } else {
                update({ reviewer_model: { ...solver } });
                setUseCustomReviewer(true);
              }
            }} />
          {useCustomReviewer && (
            <>
              <ModelSelector config={reviewer} label="Model"
                onChange={(m) => update({ reviewer_model: m })} />
              <TemperatureSlider value={reviewer.temperature}
                onChange={(t) => update({ reviewer_model: { ...reviewer, temperature: t } })} />
              <BudgetSlider value={reviewer.max_budget_tokens} label="Token Budget"
                onChange={(b) => update({ reviewer_model: { ...reviewer, max_budget_tokens: b } })} />
            </>
          )}
          {!useCustomReviewer && <span className="config-hint">Uses solver model</span>}
        </section>

        {/* Adversary Section */}
        <section className="agent-config-section">
          <h4>Adversary</h4>
          <Toggle active={useAdversary} label="Enable adversarial challenger"
            onToggle={() => {
              if (useAdversary) {
                update({ adversary_model: undefined });
                setUseAdversary(false);
              } else {
                update({ adversary_model: { ...MODEL_PRESETS[0].config } });
                setUseAdversary(true);
              }
            }} />
          {useAdversary && (
            <>
              <ModelSelector config={adversary} label="Model"
                onChange={(m) => update({ adversary_model: m })} />
              <TemperatureSlider value={adversary.temperature}
                onChange={(t) => update({ adversary_model: { ...adversary, temperature: t } })} />
              <BudgetSlider value={adversary.max_budget_tokens} label="Token Budget"
                onChange={(b) => update({ adversary_model: { ...adversary, max_budget_tokens: b } })} />
            </>
          )}
          {!useAdversary && <span className="config-hint">No adversarial challenges</span>}
        </section>

        {/* Critic Section */}
        <section className="agent-config-section">
          <h4>Critic</h4>
          <Toggle active={useCritic} label="Custom critic model"
            onToggle={() => {
              if (useCritic) {
                update({ critic_model: undefined });
                setUseCritic(false);
              } else {
                update({ critic_model: { ...solver } });
                setUseCritic(true);
              }
            }} />
          {useCritic && (
            <>
              <ModelSelector config={critic} label="Model"
                onChange={(m) => update({ critic_model: m })} />
              <TemperatureSlider value={critic.temperature}
                onChange={(t) => update({ critic_model: { ...critic, temperature: t } })} />
              <BudgetSlider value={critic.max_budget_tokens} label="Token Budget"
                onChange={(b) => update({ critic_model: { ...critic, max_budget_tokens: b } })} />
            </>
          )}
          {!useCritic && <span className="config-hint">Uses reviewer model</span>}
        </section>

        {/* Scout Section */}
        <section className="agent-config-section">
          <h4>Scout</h4>
          <p className="config-hint" style={{ marginBottom: 8 }}>
            Queries research APIs before solving begins. Results are injected into every solver prompt.
            The solver can also call these tools on-demand during each step.
          </p>
          {(() => {
            const sources = config.scout_sources ?? [];
            const SCOUT_SOURCES = [
              { id: 'arxiv', label: 'arXiv', desc: 'Mathematics preprints (free)' },
              { id: 'semantic_scholar', label: 'Semantic Scholar', desc: 'Academic papers (free)' },
              { id: 'oeis', label: 'OEIS', desc: 'Integer sequences (free)' },
              { id: 'loogle', label: 'Loogle', desc: 'Lean 4 Mathlib search (free)' },
              { id: 'wolfram', label: 'Wolfram Alpha', desc: 'Computation (API key required)' },
              { id: 'tavily', label: 'Tavily', desc: 'AI web search with extracted content (API key required)' },
            ];
            const toggle = (id: string) => {
              const next = sources.includes(id)
                ? sources.filter(s => s !== id)
                : [...sources, id];
              update({ scout_sources: next });
            };
            return (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {SCOUT_SOURCES.map(s => (
                  <label key={s.id} style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer' }}>
                    <input type="checkbox" checked={sources.includes(s.id)}
                      onChange={() => toggle(s.id)} style={{ accentColor: '#4a9eff' }} />
                    <span style={{ fontWeight: 500 }}>{s.label}</span>
                    <span className="config-hint" style={{ marginLeft: 4 }}>{s.desc}</span>
                  </label>
                ))}
                {sources.length === 0 && (
                  <span className="config-hint">No sources selected — scout disabled</span>
                )}
                {sources.length > 0 && (
                  <span className="config-hint" style={{ marginTop: 4 }}>
                    Scout will query: {sources.join(', ')}
                  </span>
                )}
              </div>
            );
          })()}
        </section>

        {/* Discerner Section */}
        <section className="agent-config-section">
          <h4>Discerner</h4>
          <Toggle active={useDiscerner} label="Enable failure classifier"
            onToggle={() => {
              if (useDiscerner) {
                update({ discerner_model: undefined });
                setUseDiscerner(false);
              } else {
                update({ discerner_model: { ...MODEL_PRESETS[0].config, temperature: 0.05, max_budget_tokens: 4000 } });
                setUseDiscerner(true);
              }
            }} />
          {useDiscerner && (
            <>
              <ModelSelector config={discerner} label="Model"
                onChange={(m) => update({ discerner_model: m })} />
              <TemperatureSlider value={discerner.temperature}
                onChange={(t) => update({ discerner_model: { ...discerner, temperature: t } })} />
              <BudgetSlider value={discerner.max_budget_tokens} label="Token Budget"
                onChange={(b) => update({ discerner_model: { ...discerner, max_budget_tokens: b } })} />
            </>
          )}
          {!useDiscerner && <span className="config-hint">Failures classified by heuristic rules only</span>}
        </section>

        {/* Advanced Section */}
        <section className="agent-config-section">
          <button className="btn btn-sm" onClick={() => setShowAdvanced(!showAdvanced)}>
            {showAdvanced ? '- Hide' : '+ Show'} Advanced
          </button>
          {showAdvanced && (
            <div className="advanced-config">
              <Toggle active={config.use_critic} label="Critic (obligation checks)"
                onToggle={() => update({ use_critic: !config.use_critic })} />
              <Toggle active={config.use_council} label="Council review"
                onToggle={() => update({ use_council: !config.use_council })} />
              <Toggle active={config.use_patterns} label="Pattern injection"
                onToggle={() => update({ use_patterns: !config.use_patterns })} />

              <div className="slider-row">
                <label>Max attempts</label>
                <input type="number" className="config-number" min={1} max={20} value={config.max_attempts}
                  onChange={(e) => update({ max_attempts: Number(e.target.value) || 5 })} />
              </div>
              <div className="slider-row">
                <label>Fail threshold</label>
                <input type="number" className="config-number" min={1} max={20} value={config.failure_threshold}
                  onChange={(e) => update({ failure_threshold: Number(e.target.value) || 5 })} />
              </div>

              <div className="slider-row">
                <label>Min coverage</label>
                <input type="range" min={0} max={1} step={0.05} value={config.min_exploration_coverage}
                  onChange={(e) => update({ min_exploration_coverage: Number(e.target.value) })} />
                <span>{config.min_exploration_coverage.toFixed(2)}</span>
              </div>
              <div className="slider-row">
                <label>Min confidence</label>
                <input type="range" min={0} max={1} step={0.05} value={config.min_conclusion_confidence}
                  onChange={(e) => update({ min_conclusion_confidence: Number(e.target.value) })} />
                <span>{config.min_conclusion_confidence.toFixed(2)}</span>
              </div>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
