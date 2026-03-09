import { useProblemStore } from '../../stores/problemStore';
import { useNavigationStore } from '../../stores/navigationStore';
import { stripLatex } from '../../utils/latex';

export function Sidebar() {
  const { problems } = useProblemStore();
  const { level, problemId, goToLibrary, goToWorkspace } = useNavigationStore();

  return (
    <aside className="sidebar">
      <div className="sidebar-section">
        <h3>Problems</h3>
        {level !== 'library' && (
          <button className="btn btn-sm" onClick={goToLibrary}>Library</button>
        )}
      </div>
      <div className="problem-list">
        {problems.length === 0 && (
          <div className="sidebar-empty">No problems yet</div>
        )}
        {problems.map((p) => (
          <div
            key={p.id}
            className={`problem-card ${problemId === p.id ? 'active' : ''}`}
            onClick={() => goToWorkspace(p.id)}
          >
            <span
              className="status-indicator"
              style={{
                background: p.status === 'solved' ? 'var(--green)'
                  : p.status === 'abandoned' ? 'var(--red)'
                  : 'var(--yellow)'
              }}
            />
            <div className="problem-card-content">
              <p className="problem-statement">
                {p.title || stripLatex(p.statement, 60)}
              </p>
              <span className="problem-meta">{p.total_steps} steps</span>
            </div>
          </div>
        ))}
      </div>
    </aside>
  );
}
