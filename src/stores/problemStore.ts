import { create } from 'zustand';
import type { Problem } from '../types';
import * as api from '../services/tauri';

interface ProblemStore {
  problems: Problem[]; activeProblem: Problem | null; loading: boolean; error: string | null;
  fetchProblems: () => Promise<void>; createProblem: (s: string, d: string) => Promise<Problem>;
  setActiveProblem: (p: Problem | null) => void;
}

export const useProblemStore = create<ProblemStore>((set) => ({
  problems: [], activeProblem: null, loading: false, error: null,
  fetchProblems: async () => {
    set({ loading: true });
    try { const problems = await api.listProblems(); set({ problems, loading: false }); }
    catch (e) { set({ error: String(e), loading: false }); }
  },
  createProblem: async (statement, domain) => {
    const p = await api.createProblem(statement, domain);
    set((s) => ({ problems: [p, ...s.problems], activeProblem: p }));
    return p;
  },
  setActiveProblem: (p) => set({ activeProblem: p }),
}));
