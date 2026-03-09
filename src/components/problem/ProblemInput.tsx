import { useState } from 'react';
import { useProblemStore } from '../../stores/problemStore';

export function ProblemInput() {
  const [statement, setStatement] = useState('');
  const [domain, setDomain] = useState('algebra');
  const [submitting, setSubmitting] = useState(false);
  const { createProblem } = useProblemStore();
  const trimmedStatement = statement.trim();

  const handleSubmit = async () => {
    if (!trimmedStatement || submitting) return;
    setSubmitting(true);
    try {
      await createProblem(trimmedStatement, domain);
      setStatement('');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="problem-input">
      <h2>New Problem</h2>
      <div className="input-group">
        <label>Statement</label>
        <textarea value={statement} onChange={(e) => setStatement(e.target.value)}
          placeholder="Prove that the sum of the first n natural numbers is n(n+1)/2" rows={4} />
      </div>
      <div className="input-group">
        <label>Domain</label>
        <select value={domain} onChange={(e) => setDomain(e.target.value)}>
          <option value="algebra">Algebra</option><option value="analysis">Analysis</option>
          <option value="number_theory">Number Theory</option><option value="combinatorics">Combinatorics</option>
        </select>
      </div>
      <button className="btn btn-primary" onClick={handleSubmit} disabled={!trimmedStatement || submitting}>
        Start Solving
      </button>
    </div>
  );
}
