import { create } from 'zustand';
import type { AttemptSummary, Claim, Conflict, AfterActionReportRecord, Step, ObligationEvent } from '../types';
import { listAttempts, getClaimsForAttempt, getConflictsForAttempt, getReportForAttempt, getReportsForProblem, deleteAttempt as apiDeleteAttempt, getAttemptSteps, getObligationGraph } from '../services/tauri';

interface WorkspaceState {
  // Attempt list for current problem
  attempts: AttemptSummary[];
  attemptsLoading: boolean;
  loadAttempts: (problemId: string) => Promise<void>;

  // Steps for current attempt (loaded from DB for past attempts)
  dbSteps: Step[];
  loadDbSteps: (attemptId: string) => Promise<void>;

  // Obligations for current attempt (loaded from DB for past attempts)
  dbObligations: ObligationEvent[];
  loadDbObligations: (attemptId: string) => Promise<void>;

  // Claims for current attempt
  claims: Claim[];
  loadClaims: (attemptId: string) => Promise<void>;

  // Conflicts for current attempt
  conflicts: Conflict[];
  loadConflicts: (attemptId: string) => Promise<void>;

  // Reports
  reports: AfterActionReportRecord[];
  loadReports: (problemId: string) => Promise<void>;
  currentReport: AfterActionReportRecord | null;
  loadCurrentReport: (attemptId: string) => Promise<void>;

  // Delete attempt
  deleteAttempt: (attemptId: string, problemId: string) => Promise<void>;

  // Reset on navigation
  clearAttemptData: () => void;
}

export const useWorkspaceStore = create<WorkspaceState>((set) => ({
  attempts: [],
  attemptsLoading: false,
  loadAttempts: async (problemId) => {
    set({ attemptsLoading: true });
    try {
      const attempts = await listAttempts(problemId);
      set({ attempts, attemptsLoading: false });
    } catch (e) {
      console.error('Failed to load attempts:', e);
      set({ attempts: [], attemptsLoading: false });
    }
  },

  dbSteps: [],
  loadDbSteps: async (attemptId) => {
    try {
      const steps = await getAttemptSteps(attemptId);
      set({ dbSteps: steps });
    } catch (e) {
      console.error('Failed to load attempt steps:', e);
      set({ dbSteps: [] });
    }
  },

  dbObligations: [],
  loadDbObligations: async (attemptId) => {
    try {
      const [raw, steps] = await Promise.all([
        getObligationGraph(attemptId),
        getAttemptSteps(attemptId),
      ]);
      // Compute vote tallies from steps per obligation
      const tallyMap = new Map<string, { yes: number; total: number }>();
      for (const s of steps) {
        if (!s.obligation_id) continue;
        const t = tallyMap.get(s.obligation_id) ?? { yes: 0, total: 0 };
        t.total++;
        if (s.verified) t.yes++;
        tallyMap.set(s.obligation_id, t);
      }
      // Map DB obligations to ObligationEvent shape
      const obligations: ObligationEvent[] = raw.map(o => {
        const tally = tallyMap.get(o.id);
        return {
          id: o.id,
          description: o.description,
          obligation_type: o.obligation_type,
          priority: o.priority,
          status: o.status as ObligationEvent['status'],
          assigned_model: o.assigned_model ?? undefined,
          steps_spent: o.steps_spent,
          max_steps: o.max_steps,
          tally_yes: tally?.yes ?? 0,
          tally_total: tally?.total ?? 0,
        };
      });
      set({ dbObligations: obligations });
    } catch (e) {
      console.error('Failed to load obligations:', e);
      set({ dbObligations: [] });
    }
  },

  claims: [],
  loadClaims: async (attemptId) => {
    try {
      const claims = await getClaimsForAttempt(attemptId);
      set({ claims });
    } catch (e) {
      console.error('Failed to load claims:', e);
      set({ claims: [] });
    }
  },

  conflicts: [],
  loadConflicts: async (attemptId) => {
    try {
      const conflicts = await getConflictsForAttempt(attemptId);
      set({ conflicts });
    } catch (e) {
      console.error('Failed to load conflicts:', e);
      set({ conflicts: [] });
    }
  },

  reports: [],
  loadReports: async (problemId) => {
    try {
      const reports = await getReportsForProblem(problemId);
      set({ reports });
    } catch (e) {
      console.error('Failed to load reports:', e);
      set({ reports: [] });
    }
  },

  currentReport: null,
  loadCurrentReport: async (attemptId) => {
    try {
      const report = await getReportForAttempt(attemptId);
      set({ currentReport: report });
    } catch {
      set({ currentReport: null });
    }
  },

  deleteAttempt: async (attemptId, problemId) => {
    try {
      await apiDeleteAttempt(attemptId);
    } catch (e) {
      console.error('Failed to delete attempt:', e);
      alert(`Delete failed: ${e}`);
      return;
    }
    try {
      const attempts = await listAttempts(problemId);
      set({ attempts });
    } catch {
      set({ attempts: [] });
    }
  },

  clearAttemptData: () => set({ claims: [], conflicts: [], currentReport: null, dbSteps: [], dbObligations: [] }),
}));
