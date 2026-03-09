export function Header() {
  return (
    <header className="header">
      <div className="header-left">
        <h1 className="header-title">
          <span className="header-logo">ChatDB</span>
          <span className="header-subtitle">Verified Reasoning Engine</span>
        </h1>
      </div>
      <div className="header-right">
        <span className="status-dot running" />
        <span className="header-meta">v0.1.0</span>
      </div>
    </header>
  );
}
