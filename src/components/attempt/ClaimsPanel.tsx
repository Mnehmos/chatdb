import type { Claim } from '../../types';

interface Props {
  claims: Claim[];
}

export function ClaimsPanel({ claims }: Props) {
  if (claims.length === 0) return null;

  const byType = claims.reduce((acc, c) => {
    acc[c.claim_type] = (acc[c.claim_type] || 0) + 1;
    return acc;
  }, {} as Record<string, number>);

  return (
    <div className="claims-panel">
      <h3>Claims ({claims.length})</h3>
      <div className="claims-summary">
        {Object.entries(byType).map(([type, count]) => (
          <span key={type} className="claim-type-badge">{type}: {count}</span>
        ))}
      </div>
      <div className="claims-list">
        {claims.map(c => (
          <div key={c.id} className={`claim-card ${c.superseded_by ? 'claim-superseded' : ''}`}>
            <div className="claim-header">
              <span className="claim-type">{c.claim_type}</span>
              <span className="claim-object">{c.object}</span>
              {c.direction && <span className="claim-direction">{c.direction}</span>}
              <span className="claim-confidence">{(c.confidence * 100).toFixed(0)}%</span>
            </div>
            <div className="claim-natural">{c.natural_text}</div>
            {c.scope_constraint && (
              <div className="claim-scope">{c.scope_type}: {c.scope_param} ({c.scope_constraint})</div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
