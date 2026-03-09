import { useState } from 'react';
import { useAgentStore } from '../../stores/agentStore';

export function AgentDashboard() {
  const { orchestratorLog, criticLog, councilSessions, scoutResults } = useAgentStore();
  const [scoutExpanded, setScoutExpanded] = useState(false);
  const total = orchestratorLog.length + criticLog.length + councilSessions.length + scoutResults.length;
  if (!total) return <div className="agent-dashboard empty"><p className="muted">Agent activity appears here during solving.</p></div>;

  return (
    <div className="agent-dashboard">
      <h3>Agent Activity</h3>
      <div className="agent-grid">
        <div className="agent-panel"><h4>Orchestrator</h4><span className="count">{orchestratorLog.length}</span></div>
        <div className="agent-panel"><h4>Critic</h4><span className="count">{criticLog.length}</span></div>
        <div className="agent-panel"><h4>Council</h4><span className="count">{councilSessions.length}</span></div>
        <div
          className="agent-panel agent-panel-scout"
          onClick={() => setScoutExpanded(!scoutExpanded)}
          style={{ cursor: scoutResults.length ? 'pointer' : 'default' }}
        >
          <h4>Scout</h4>
          <span className="count">{scoutResults.length}</span>
        </div>
      </div>
      {scoutExpanded && scoutResults.length > 0 && (
        <div className="scout-results-panel">
          {scoutResults.map((r, i) => (
            <div key={i} className="scout-result-entry">
              <div className="scout-result-header">
                <span className={`scout-trigger-badge ${r.trigger === 'mid_solve' ? 'mid-solve' : 'pre-solve'}`}>
                  {r.trigger === 'mid_solve' ? 'MID-SOLVE' : 'PRE-SOLVE'}
                </span>
                <span className="scout-sources">{(r.sources || []).join(', ')}</span>
                <span className="scout-count">{r.results_count} results</span>
              </div>
              {r.obligation_desc && (
                <div className="scout-obligation">Stuck on: {r.obligation_desc}</div>
              )}
              {r.briefing && (
                <pre className="scout-briefing">{r.briefing}</pre>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
