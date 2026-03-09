import { useState, useEffect } from 'react';
import { exportTrainingData, getExportDirectory } from '../../services/tauri';

interface Props {
  problemId?: string;
}

const EXPORT_TYPES = [
  {
    id: 'step_verification',
    label: 'Step Verification Pairs',
    description: 'Step + prior chain context + verification verdict. For training step verifiers.',
  },
  {
    id: 'planning',
    label: 'Planning Decisions',
    description: 'Obligation decomposition, priorities, and budget allocation. For training planners.',
  },
  {
    id: 'contrastive',
    label: 'Contrastive Pairs',
    description: 'Accepted vs rejected step pairs recorded at the same proof slot. For preference and ranking training.',
  },
  {
    id: 'contradiction',
    label: 'Contradiction Detection',
    description: 'Claim pairs with conflict labels and resolutions. For training conflict detectors.',
  },
  {
    id: 'after_action',
    label: 'After-Action Analysis',
    description: 'Attempt metrics, failure modes, and chain stats. For training reviewers.',
  },
] as const;

export function ExportPanel({ problemId }: Props) {
  const [selectedTypes, setSelectedTypes] = useState<Set<string>>(new Set(['step_verification']));
  const [outputDir, setOutputDir] = useState('');
  const [exporting, setExporting] = useState(false);
  const [results, setResults] = useState<{ type: string; count: number; error?: string }[]>([]);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    getExportDirectory().then(dir => setOutputDir(dir)).catch(() => setOutputDir('exports'));
  }, []);

  const toggleType = (id: string) => {
    setSelectedTypes(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleExport = async () => {
    if (selectedTypes.size === 0) return;
    setExporting(true);
    setResults([]);
    const newResults: typeof results = [];

    for (const type of selectedTypes) {
      const filename = problemId
        ? `${outputDir}/${type}_${problemId.slice(0, 8)}.jsonl`
        : `${outputDir}/${type}_all.jsonl`;
      try {
        const count = await exportTrainingData(type, filename, problemId);
        newResults.push({ type, count });
      } catch (e) {
        newResults.push({ type, count: 0, error: String(e) });
      }
    }
    setResults(newResults);
    setExporting(false);
  };

  return (
    <div className="export-panel">
      <div className="export-header" onClick={() => setExpanded(!expanded)}>
        <span>{expanded ? '\u25BC' : '\u25B6'}</span>
        <h4>Export Training Data</h4>
        <span className="export-scope">{problemId ? 'This problem' : 'All problems'}</span>
      </div>
      {expanded && (
        <div className="export-body">
          <div className="export-types">
            {EXPORT_TYPES.map(et => (
              <label key={et.id} className="export-type-option">
                <input
                  type="checkbox"
                  checked={selectedTypes.has(et.id)}
                  onChange={() => toggleType(et.id)}
                />
                <div>
                  <strong>{et.label}</strong>
                  <span className="export-type-desc">{et.description}</span>
                </div>
              </label>
            ))}
          </div>
          <div className="export-path-row">
            <label>Output directory:</label>
            <input
              type="text"
              value={outputDir}
              onChange={e => setOutputDir(e.target.value)}
              className="export-path-input"
              placeholder="exports"
            />
          </div>
          <button
            className="btn btn-primary"
            onClick={handleExport}
            disabled={exporting || selectedTypes.size === 0}
          >
            {exporting ? 'Exporting...' : `Export ${selectedTypes.size} type(s)`}
          </button>
          {results.length > 0 && (
            <div className="export-results">
              {results.map((r, i) => (
                <div key={i} className={`export-result ${r.error ? 'export-result-error' : 'export-result-ok'}`}>
                  <span className="export-result-type">{r.type}</span>
                  {r.error ? (
                    <span className="export-result-msg">{r.error}</span>
                  ) : (
                    <span className="export-result-msg">{r.count} records exported</span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
