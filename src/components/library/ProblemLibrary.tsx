import { useState, useMemo } from 'react';
import { useProblemStore } from '../../stores/problemStore';
import { useNavigationStore } from '../../stores/navigationStore';
import { ProblemInput } from '../problem/ProblemInput';
import { ExportPanel } from '../export/ExportPanel';
import { stripLatex } from '../../utils/latex';

export function ProblemLibrary() {
  const { problems } = useProblemStore();
  const { goToWorkspace } = useNavigationStore();
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState<string>('');
  const [domainFilter, setDomainFilter] = useState<string>('');
  const [showNewForm, setShowNewForm] = useState(false);

  const domains = useMemo(() => {
    const d = new Set(problems.map(p => p.domain).filter(Boolean));
    return Array.from(d).sort();
  }, [problems]);

  const filtered = useMemo(() => {
    return problems.filter(p => {
      if (search) {
        const q = search.toLowerCase();
        const matchesSearch = (p.title || '').toLowerCase().includes(q)
          || p.statement.toLowerCase().includes(q)
          || (p.source || '').toLowerCase().includes(q);
        if (!matchesSearch) return false;
      }
      if (statusFilter && p.status !== statusFilter) return false;
      if (domainFilter && p.domain !== domainFilter) return false;
      return true;
    });
  }, [problems, search, statusFilter, domainFilter]);

  return (
    <div className="problem-library">
      <div className="library-header">
        <h2>Problem Library</h2>
        <button className="btn btn-primary" onClick={() => setShowNewForm(!showNewForm)}>
          + New Problem
        </button>
      </div>

      {showNewForm && (
        <div className="library-new-form">
          <ProblemInput />
        </div>
      )}

      <div className="library-filters">
        <input
          type="text"
          className="filter-search"
          placeholder="Search problems..."
          value={search}
          onChange={e => setSearch(e.target.value)}
        />
        <select value={statusFilter} onChange={e => setStatusFilter(e.target.value)}>
          <option value="">All Status</option>
          <option value="open">Open</option>
          <option value="solved">Solved</option>
          <option value="abandoned">Abandoned</option>
        </select>
        <select value={domainFilter} onChange={e => setDomainFilter(e.target.value)}>
          <option value="">All Domains</option>
          {domains.map(d => <option key={d} value={d}>{d}</option>)}
        </select>
      </div>

      <table className="library-table">
        <thead>
          <tr>
            <th>Status</th>
            <th>Title / Statement</th>
            <th>Source</th>
            <th>Domain</th>
            <th>Difficulty</th>
            <th>Attempts</th>
            <th>Steps</th>
          </tr>
        </thead>
        <tbody>
          {filtered.length === 0 && (
            <tr><td colSpan={7} className="library-empty">No problems found</td></tr>
          )}
          {filtered.map(p => (
            <tr key={p.id} className="library-row" onClick={() => goToWorkspace(p.id)}>
              <td>
                <span className={`status-dot status-${p.status}`} />
              </td>
              <td className="library-title">
                {p.title || stripLatex(p.statement, 80)}
              </td>
              <td>{p.source || '-'}</td>
              <td>{p.domain || '-'}</td>
              <td>{p.difficulty || '-'}</td>
              <td>{p.total_attempts}</td>
              <td>{p.total_steps}</td>
            </tr>
          ))}
        </tbody>
      </table>

      {/* Bulk export (all problems) */}
      <ExportPanel />
    </div>
  );
}
