import type { ObligationEvent } from '../../types';

interface Props {
  obligations: ObligationEvent[];
  onSelect?: (id: string) => void;
}

const priorityLabel = (p: number): string => {
  if (p >= 0.9) return 'S+';
  if (p >= 0.7) return 'S';
  if (p >= 0.5) return 'A';
  if (p >= 0.3) return 'B';
  return 'C';
};

const statusClass = (s: string): string => {
  if (s === 'open') return 'ob-open';
  if (s.includes('proved')) return 'ob-proved';
  if (s.includes('refuted')) return 'ob-refuted';
  return 'ob-other';
};

interface CardProps {
  ob: ObligationEvent;
  onClick?: () => void;
}

const hasClosingMajority = (yes?: number, total?: number): boolean =>
  yes != null && total != null && total >= 3 && yes * 2 > total;

function ObligationCard({ ob, onClick }: CardProps) {
  return (
    <div
      className={`obligation-card ${statusClass(ob.status)} ${onClick ? 'ob-clickable' : ''}`}
      onClick={onClick}
    >
      <div className="ob-header">
        <span className="ob-priority">[{priorityLabel(ob.priority)}]</span>
        <span className="ob-type">{ob.obligation_type}</span>
        <span className={`ob-status ${statusClass(ob.status)}`}>
          {ob.status === 'open' ? 'OPEN'
            : ob.status === 'assigned' ? 'ACTIVE'
            : ob.status.includes('proved') ? 'CLOSED'
            : ob.status.toUpperCase()}
        </span>
      </div>
      <div className="ob-desc">{ob.description}</div>

      {/* Progress bar for open obligations */}
      {ob.steps_spent != null && ob.max_steps != null && (ob.status === 'open' || ob.status === 'assigned') && (
        <div className="ob-progress">
          <div
            className="ob-progress-bar"
            style={{ width: `${Math.min(100, (ob.steps_spent / ob.max_steps) * 100)}%` }}
          />
          <span className="ob-progress-label">{ob.steps_spent}/{ob.max_steps}</span>
        </div>
      )}

      {/* Closure note for closed obligations */}
      {ob.status !== 'open' && ob.status !== 'assigned' && ob.closure_note && (
        <div className="ob-closure-note">{ob.closure_note}</div>
      )}
      {ob.status !== 'open' && ob.status !== 'assigned' && ob.closed_by_step != null && (
        <div className="ob-closed-by">closed at step #{ob.closed_by_step}</div>
      )}

      {/* Satisfaction tally */}
      {ob.signals && ob.signals.length > 0 && (
        <div className="ob-tally">
          <div className="ob-tally-header">
            <span className="ob-tally-label">Votes: </span>
            <span className={`ob-tally-count ${hasClosingMajority(ob.tally_yes, ob.tally_total) ? 'tally-majority' : ''}`}>
              {ob.tally_yes ?? 0}/{ob.tally_total ?? 0}
            </span>
          </div>
          <div className="ob-signals">
            {ob.signals.map((sig, i) => (
              <div key={i} className={`ob-signal ${sig.satisfies ? 'sig-yes' : 'sig-no'}`}>
                <span className="sig-icon">{sig.satisfies ? '✓' : '✗'}</span>
                <span className="sig-source">{sig.source}</span>
                {sig.note && <span className="sig-note">{sig.note}</span>}
              </div>
            ))}
          </div>
        </div>
      )}

      {ob.assigned_model && (
        <div className="ob-model">{ob.assigned_model}</div>
      )}
    </div>
  );
}

export function ObligationQueue({ obligations, onSelect }: Props) {
  const open = obligations
    .filter(o => o.status === 'open' || o.status === 'assigned')
    .sort((a, b) => b.priority - a.priority);

  const closed = obligations
    .filter(o => o.status !== 'open' && o.status !== 'assigned')
    .sort((a, b) => {
      // Proved before refuted, then by closure step descending (most recent first)
      if (a.status.includes('proved') && !b.status.includes('proved')) return -1;
      if (!a.status.includes('proved') && b.status.includes('proved')) return 1;
      return (b.closed_by_step ?? 0) - (a.closed_by_step ?? 0);
    });

  if (obligations.length === 0) {
    return (
      <div className="obligation-queue">
        <h3>Obligations</h3>
        <div className="queue-empty">No obligations</div>
      </div>
    );
  }

  return (
    <div className="obligation-queue">
      <h3>Obligations ({obligations.length})</h3>

      {/* OPEN section */}
      <div className="ob-section">
        <div className="ob-section-header ob-section-open">
          <span className="ob-section-label">OPEN</span>
          <span className="ob-section-count">{open.length}</span>
        </div>
        {open.length === 0
          ? <div className="queue-empty queue-empty--small">All obligations resolved</div>
          : open.map(ob => <ObligationCard key={ob.id} ob={ob} onClick={onSelect ? () => onSelect(ob.id) : undefined} />)
        }
      </div>

      {/* CLOSED section — only shown when there are closed obligations */}
      {closed.length > 0 && (
        <div className="ob-section ob-section--closed">
          <div className="ob-section-header ob-section-closed">
            <span className="ob-section-label">CLOSED</span>
            <span className="ob-section-count">{closed.length}</span>
          </div>
          {closed.map(ob => <ObligationCard key={ob.id} ob={ob} onClick={onSelect ? () => onSelect(ob.id) : undefined} />)}
        </div>
      )}
    </div>
  );
}
