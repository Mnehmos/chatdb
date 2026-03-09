import { useEffect, useRef } from 'react';
import { useDiagnosticStore } from '../../stores/diagnosticStore';
import type { DiagnosticCategory, DiagnosticSeverity, DiagnosticEvent } from '../../types';

const CATEGORY_LABELS: Record<DiagnosticCategory, string> = {
  model: 'Model',
  mechanical: 'Mechanical',
  validator: 'Validator',
  gate: 'Gate',
  info: 'Info',
};

const SEVERITY_COLORS: Record<DiagnosticSeverity, string> = {
  info: 'var(--color-text-dim, #888)',
  warn: '#e5a60d',
  error: '#e05555',
  fatal: '#ff2222',
};

const CATEGORY_COLORS: Record<DiagnosticCategory, string> = {
  model: '#e05555',
  mechanical: '#e5a60d',
  gate: '#c4b54e',
  validator: '#6ec6ff',
  info: 'var(--color-text-dim, #888)',
};

function formatTime(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
  } catch { return ts; }
}

function HealthDot({ ok, label }: { ok: boolean | null; label: string }) {
  const color = ok === null ? '#666' : ok ? '#4caf50' : '#e05555';
  return (
    <span className="health-indicator">
      <span className="health-dot" style={{ background: color }} />
      {label}
    </span>
  );
}

function EventRow({ event, index }: { event: DiagnosticEvent; index: number }) {
  const { expanded, toggleExpanded } = useDiagnosticStore();
  const isExpanded = expanded.has(index);
  const sevColor = SEVERITY_COLORS[event.severity];
  const catColor = CATEGORY_COLORS[event.category];

  return (
    <div
      className={`diagnostic-event diagnostic-event-${event.category} ${event.severity === 'fatal' ? 'diagnostic-fatal' : ''}`}
      onClick={() => toggleExpanded(index)}
    >
      <span className="diag-time">{formatTime(event.ts)}</span>
      <span className="diag-severity" style={{ color: sevColor }}>
        {event.severity.toUpperCase()}
      </span>
      <span className="diag-source" style={{ color: catColor }}>
        [{event.source}]
      </span>
      {event.step_number != null && (
        <span className="diag-step">S{event.step_number}</span>
      )}
      <span className="diag-message">{event.message}</span>
      {isExpanded && event.detail !== undefined && (
        <pre className="diag-detail">
          {typeof event.detail === 'string' ? event.detail : JSON.stringify(event.detail, null, 2)}
        </pre>
      )}
    </div>
  );
}

export function DiagnosticPanel() {
  const {
    events, health, filterCategory, filterSeverity, open,
    setFilterCategory, setFilterSeverity, setOpen, clear, refreshHealth,
  } = useDiagnosticStore();
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new events
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [events.length]);

  // Refresh health periodically when panel is open
  useEffect(() => {
    if (!open) return;
    refreshHealth();
    const interval = setInterval(refreshHealth, 5000);
    return () => clearInterval(interval);
  }, [open]);

  // Count errors by category
  const modelErrors = events.filter(e => e.category === 'model' && (e.severity === 'error' || e.severity === 'fatal' || e.severity === 'warn')).length;
  const mechErrors = events.filter(e => e.category === 'mechanical' && (e.severity === 'error' || e.severity === 'fatal' || e.severity === 'warn')).length;

  // Filter events
  const filtered = events.filter(e => {
    if (filterCategory && e.category !== filterCategory) return false;
    if (filterSeverity && e.severity !== filterSeverity) return false;
    return true;
  });

  if (!open) {
    return (
      <div className="diagnostic-toggle" onClick={() => setOpen(true)}>
        <span>Diagnostics</span>
        {modelErrors > 0 && <span className="error-count-badge model-badge">{modelErrors}</span>}
        {mechErrors > 0 && <span className="error-count-badge mech-badge">{mechErrors}</span>}
      </div>
    );
  }

  return (
    <div className="diagnostic-panel">
      {/* Header */}
      <div className="diagnostic-header">
        <div className="diagnostic-title">
          <span>Diagnostics</span>
          {modelErrors > 0 && <span className="error-count-badge model-badge" title="Model errors">M:{modelErrors}</span>}
          {mechErrors > 0 && <span className="error-count-badge mech-badge" title="Mechanical errors">I:{mechErrors}</span>}
        </div>
        <div className="diagnostic-health">
          {health && (
            <>
              <HealthDot ok={health.sidecar_reachable} label="Sidecar" />
              <HealthDot ok={health.lean_available ? (health.lean_ready ? true : null) : false} label="Lean" />
              <HealthDot ok={health.loop_running} label="Loop" />
            </>
          )}
        </div>
        <div className="diagnostic-controls">
          <button className="btn btn-xs" onClick={clear}>Clear</button>
          <button className="btn btn-xs" onClick={() => setOpen(false)}>Close</button>
        </div>
      </div>

      {/* Filters */}
      <div className="diagnostic-filters">
        <div className="filter-group">
          <button className={`filter-btn ${!filterCategory ? 'active' : ''}`} onClick={() => setFilterCategory(null)}>All</button>
          {(Object.keys(CATEGORY_LABELS) as DiagnosticCategory[]).map(c => (
            <button key={c} className={`filter-btn ${filterCategory === c ? 'active' : ''}`}
              style={{ borderColor: CATEGORY_COLORS[c] }}
              onClick={() => setFilterCategory(filterCategory === c ? null : c)}>
              {CATEGORY_LABELS[c]}
            </button>
          ))}
        </div>
        <div className="filter-group">
          {(['info', 'warn', 'error', 'fatal'] as DiagnosticSeverity[]).map(s => (
            <button key={s} className={`filter-btn ${filterSeverity === s ? 'active' : ''}`}
              style={{ borderColor: SEVERITY_COLORS[s] }}
              onClick={() => setFilterSeverity(filterSeverity === s ? null : s)}>
              {s.toUpperCase()}
            </button>
          ))}
        </div>
        <span className="event-count">{filtered.length}/{events.length}</span>
      </div>

      {/* Event List */}
      <div className="diagnostic-body" ref={scrollRef}>
        {filtered.length === 0 && (
          <div className="diagnostic-empty">No diagnostic events yet. Start a solve to see real-time diagnostics.</div>
        )}
        {filtered.map((e, i) => (
          <EventRow key={`${e.ts}-${i}`} event={e} index={i} />
        ))}
      </div>
    </div>
  );
}
