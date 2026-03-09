import type { Conflict } from '../../types';

interface Props {
  conflicts: Conflict[];
}

export function ConflictsPanel({ conflicts }: Props) {
  if (conflicts.length === 0) return null;

  const unresolved = conflicts.filter(c => !c.resolution);

  return (
    <div className="conflicts-panel">
      <h3>
        Conflicts ({conflicts.length})
        {unresolved.length > 0 && (
          <span className="conflict-unresolved-count"> — {unresolved.length} unresolved</span>
        )}
      </h3>
      <div className="conflicts-list">
        {conflicts.map(c => (
          <div key={c.id} className={`conflict-card ${c.resolution ? 'conflict-resolved' : 'conflict-active'}`}>
            <div className="conflict-header">
              <span className={`conflict-severity severity-${c.severity}`}>{c.severity}</span>
              <span className="conflict-type">{c.conflict_type}</span>
              {c.resolution && <span className="conflict-resolution">{c.resolution}</span>}
            </div>
            <div className="conflict-desc">{c.description}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
