import { useState, useEffect } from 'react';
import {
  listResearchApiKeys,
  setResearchApiKey,
  deleteResearchApiKey,
  toggleResearchApiKey,
  researchSearch,
  researchSources,
} from '../../services/tauri';
import type { ResearchApiKeyInfo, ResearchSource } from '../../types';

/**
 * Service metadata: display name, env var hint, description.
 * Matches the service IDs used in the Python config.py and DB.
 */
const SERVICE_META: Record<string, { name: string; envVar: string; hint: string; isEmail?: boolean }> = {
  semantic_scholar: { name: 'Semantic Scholar', envVar: 'SEMANTIC_SCHOLAR_API_KEY', hint: 'API key for higher rate limits (optional)' },
  openalex: { name: 'OpenAlex', envVar: 'OPENALEX_EMAIL', hint: 'Email for polite pool (optional)', isEmail: true },
  wolfram_alpha: { name: 'Wolfram|Alpha', envVar: 'WOLFRAM_APP_ID', hint: 'AppID from developer.wolframalpha.com (required)' },
  huggingface: { name: 'HuggingFace', envVar: 'HUGGINGFACE_TOKEN', hint: 'Token for private repos (optional)' },
  ncbi: { name: 'PubMed / NCBI', envVar: 'NCBI_API_KEY', hint: 'API key for higher rate limits (optional)' },
  unpaywall_email: { name: 'Unpaywall', envVar: 'UNPAYWALL_EMAIL', hint: 'Email address (required for Unpaywall)', isEmail: true },
};

const SERVICE_ORDER = ['wolfram_alpha', 'semantic_scholar', 'unpaywall_email', 'openalex', 'huggingface', 'ncbi'];

interface Props {
  open: boolean;
  onClose: () => void;
}

export function ResearchApiPanel({ open, onClose }: Props) {
  const [keys, setKeys] = useState<ResearchApiKeyInfo[]>([]);
  const [sources, setSources] = useState<ResearchSource[]>([]);
  const [editingService, setEditingService] = useState<string | null>(null);
  const [editValue, setEditValue] = useState('');
  const [editLabel, setEditLabel] = useState('');
  const [saveStatus, setSaveStatus] = useState<{ type: 'idle' | 'saving' | 'ok' | 'error'; msg?: string }>({ type: 'idle' });

  // Test search state
  const [testSource, setTestSource] = useState('arxiv');
  const [testQuery, setTestQuery] = useState('');
  const [testResult, setTestResult] = useState<string>('');
  const [testLoading, setTestLoading] = useState(false);

  const loadKeys = async () => {
    try {
      const k = await listResearchApiKeys();
      setKeys(k);
    } catch {
      // keys table might not exist yet
    }
  };

  const loadSources = async () => {
    try {
      const s = await researchSources();
      setSources(s.sources || []);
    } catch {
      // sidecar might not be running
    }
  };

  useEffect(() => {
    if (open) {
      loadKeys();
      loadSources();
    }
  }, [open]);

  if (!open) return null;

  const getKeyForService = (service: string) => keys.find(k => k.service === service);

  const handleSave = async (service: string) => {
    if (!editValue.trim()) return;
    setSaveStatus({ type: 'saving' });
    try {
      await setResearchApiKey(service, editValue.trim(), editLabel.trim() || undefined);
      setSaveStatus({ type: 'ok', msg: `Saved ${SERVICE_META[service]?.name || service}` });
      setEditingService(null);
      setEditValue('');
      setEditLabel('');
      await loadKeys();
      setTimeout(() => setSaveStatus({ type: 'idle' }), 3000);
    } catch (e) {
      setSaveStatus({ type: 'error', msg: String(e) });
    }
  };

  const handleDelete = async (service: string) => {
    try {
      await deleteResearchApiKey(service);
      await loadKeys();
    } catch (e) {
      setSaveStatus({ type: 'error', msg: String(e) });
    }
  };

  const handleToggle = async (service: string, active: boolean) => {
    try {
      await toggleResearchApiKey(service, active);
      await loadKeys();
    } catch (e) {
      setSaveStatus({ type: 'error', msg: String(e) });
    }
  };

  const handleTest = async () => {
    if (!testQuery.trim()) return;
    setTestLoading(true);
    setTestResult('');
    try {
      const result = await researchSearch(testSource, testQuery.trim(), 3);
      setTestResult(JSON.stringify(result, null, 2));
    } catch (e) {
      setTestResult(`Error: ${e}`);
    } finally {
      setTestLoading(false);
    }
  };

  return (
    <div className="profile-panel-overlay" onClick={onClose}>
      <div className="profile-panel" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="profile-panel-header">
          <h3>Research APIs</h3>
          <button className="btn btn-sm" onClick={onClose}>Close</button>
        </div>

        {saveStatus.type !== 'idle' && (
          <div className={`profile-save-status profile-save-${saveStatus.type}`}
            style={saveStatus.type === 'error' ? { background: '#3a1111', border: '1px solid #ff4444', padding: '8px 12px', borderRadius: '4px', fontSize: '12px' } : undefined}>
            {saveStatus.type === 'saving' && 'Saving...'}
            {saveStatus.type === 'ok' && saveStatus.msg}
            {saveStatus.type === 'error' && (
              <div>
                <strong style={{ color: '#ff6666' }}>Error:</strong>{' '}
                <span style={{ color: '#ffaaaa' }}>{saveStatus.msg}</span>
              </div>
            )}
          </div>
        )}

        {/* Free sources (no key needed) */}
        <section className="agent-config-section">
          <h4>Free Sources (no key needed)</h4>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
            {sources.filter(s => !s.requires_key && !s.key_optional && !s.requires_email).map(s => (
              <span key={s.id} style={{
                padding: '3px 8px', borderRadius: '4px', fontSize: '11px',
                fontFamily: 'var(--font-mono)', background: 'var(--bg-tertiary)',
                color: 'var(--green)', border: '1px solid var(--border)',
              }}>
                {s.name}
              </span>
            ))}
            {sources.length === 0 && (
              <span style={{ fontSize: '11px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                arXiv, OEIS, Crossref, Loogle, Reservoir (always available)
              </span>
            )}
          </div>
        </section>

        {/* API Key Management */}
        <section className="agent-config-section">
          <h4>API Keys</h4>
          {SERVICE_ORDER.map(service => {
            const meta = SERVICE_META[service];
            const existing = getKeyForService(service);
            const isEditing = editingService === service;

            return (
              <div key={service} style={{
                padding: '10px 0', borderBottom: '1px solid var(--border)',
              }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '4px' }}>
                  <span style={{ fontFamily: 'var(--font-mono)', fontSize: '12px', fontWeight: 600, color: 'var(--text-primary)', flex: 1 }}>
                    {meta.name}
                  </span>
                  {existing && (
                    <>
                      <span style={{
                        fontFamily: 'var(--font-mono)', fontSize: '10px',
                        color: existing.active ? 'var(--green)' : 'var(--text-muted)',
                      }}>
                        {existing.key_masked}
                      </span>
                      <div
                        className={`toggle-switch ${existing.active ? 'active' : ''}`}
                        style={{ width: '28px', height: '16px' }}
                        onClick={() => handleToggle(service, !existing.active)}
                      />
                      <button className="btn btn-sm" style={{ fontSize: '10px', padding: '2px 6px', color: 'var(--red)' }}
                        onClick={() => handleDelete(service)}>
                        Del
                      </button>
                    </>
                  )}
                  {!existing && !isEditing && (
                    <button className="btn btn-sm" style={{ fontSize: '10px', padding: '2px 8px' }}
                      onClick={() => { setEditingService(service); setEditValue(''); setEditLabel(''); }}>
                      + Add
                    </button>
                  )}
                </div>
                <div style={{ fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                  {meta.hint} ({meta.envVar})
                </div>

                {isEditing && (
                  <div style={{ marginTop: '8px', display: 'flex', flexDirection: 'column', gap: '6px' }}>
                    <input
                      type={meta.isEmail ? 'email' : 'password'}
                      placeholder={meta.isEmail ? 'your@email.com' : 'Paste API key...'}
                      value={editValue}
                      onChange={(e) => setEditValue(e.target.value)}
                      style={{
                        padding: '6px 10px', background: 'var(--bg-tertiary)',
                        border: '1px solid var(--border)', borderRadius: 'var(--radius)',
                        color: 'var(--text-primary)', fontFamily: 'var(--font-mono)', fontSize: '12px',
                      }}
                    />
                    <div style={{ display: 'flex', gap: '6px' }}>
                      <input
                        type="text"
                        placeholder="Label (optional)"
                        value={editLabel}
                        onChange={(e) => setEditLabel(e.target.value)}
                        style={{
                          flex: 1, padding: '4px 8px', background: 'var(--bg-tertiary)',
                          border: '1px solid var(--border)', borderRadius: 'var(--radius)',
                          color: 'var(--text-primary)', fontFamily: 'var(--font-mono)', fontSize: '11px',
                        }}
                      />
                      <button className="btn btn-sm btn-primary" style={{ fontSize: '10px' }}
                        disabled={!editValue.trim()}
                        onClick={() => handleSave(service)}>
                        Save
                      </button>
                      <button className="btn btn-sm" style={{ fontSize: '10px' }}
                        onClick={() => { setEditingService(null); setEditValue(''); }}>
                        Cancel
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </section>

        {/* Test Search */}
        <section className="agent-config-section">
          <h4>Test Search</h4>
          <div style={{ display: 'flex', gap: '6px', marginBottom: '8px' }}>
            <select
              value={testSource}
              onChange={(e) => setTestSource(e.target.value)}
              style={{
                padding: '4px 8px', background: 'var(--bg-tertiary)',
                border: '1px solid var(--border)', borderRadius: 'var(--radius)',
                color: 'var(--text-primary)', fontFamily: 'var(--font-mono)', fontSize: '11px',
              }}
            >
              <option value="arxiv">arXiv</option>
              <option value="semantic_scholar">Semantic Scholar</option>
              <option value="oeis">OEIS</option>
              <option value="crossref">Crossref</option>
              <option value="openalex">OpenAlex</option>
              <option value="wolfram_alpha">Wolfram|Alpha</option>
              <option value="huggingface_models">HuggingFace Models</option>
              <option value="loogle">Loogle (Mathlib)</option>
              <option value="pubmed">PubMed</option>
            </select>
            <input
              type="text"
              placeholder="Search query..."
              value={testQuery}
              onChange={(e) => setTestQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleTest()}
              style={{
                flex: 1, padding: '4px 8px', background: 'var(--bg-tertiary)',
                border: '1px solid var(--border)', borderRadius: 'var(--radius)',
                color: 'var(--text-primary)', fontFamily: 'var(--font-mono)', fontSize: '11px',
              }}
            />
            <button className="btn btn-sm btn-primary" disabled={testLoading || !testQuery.trim()} onClick={handleTest}>
              {testLoading ? '...' : 'Test'}
            </button>
          </div>
          {testResult && (
            <pre style={{
              background: 'var(--bg-tertiary)', border: '1px solid var(--border)',
              borderRadius: 'var(--radius)', padding: '8px', fontSize: '10px',
              fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)',
              maxHeight: '200px', overflow: 'auto', whiteSpace: 'pre-wrap',
            }}>
              {testResult}
            </pre>
          )}
        </section>
      </div>
    </div>
  );
}
