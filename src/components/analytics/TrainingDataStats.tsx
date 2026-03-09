import { useEffect, useState } from 'react';
import { getTrainingDataStats } from '../../services/tauri';
import { useLoopStore } from '../../stores/loopStore';
import type { TrainingDataStats as TDStats } from '../../types';

export function TrainingDataStats() {
  const [stats, setStats] = useState<TDStats | null>(null);
  const status = useLoopStore((s) => s.status);
  useEffect(() => { getTrainingDataStats().then(setStats).catch(console.error); }, [status]);
  if (!stats) return null;

  const layers = [
    { name: 'Steps', count: stats.total_steps, color: '#22c55e' },
    { name: 'Contrastive', count: stats.contrastive_pairs, color: '#3b82f6' },
    { name: 'Orchestration', count: stats.orchestrator_decisions, color: '#a855f7' },
    { name: 'Council', count: stats.council_findings, color: '#f59e0b' },
    { name: 'Scout', count: stats.scout_queries, color: '#06b6d4' },
    { name: 'Critic', count: stats.critic_evaluations, color: '#ef4444' },
    { name: 'Librarian', count: stats.librarian_actions, color: '#84cc16' },
  ];
  const total = layers.reduce((a, l) => a + l.count, 0);

  return (
    <div className="training-stats">
      <h3>Training Data Generated <span className="stats-scope">all problems</span></h3>
      <div className="stats-total"><span className="total-number">{total.toLocaleString()}</span><span className="total-label"> examples</span></div>
      <div className="layer-bars">
        {layers.map((l) => (
          <div key={l.name} className="layer-row">
            <span className="layer-name">{l.name}</span>
            <div className="layer-bar"><div className="layer-fill" style={{ width: total > 0 ? `${(l.count/total)*100}%` : '0%', background: l.color }} /></div>
            <span className="layer-count">{l.count}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
