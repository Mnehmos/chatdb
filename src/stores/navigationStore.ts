import { create } from 'zustand';
import type { ViewLevel } from '../types';

interface NavigationState {
  level: ViewLevel;
  problemId: string | null;
  attemptId: string | null;
  stepId: string | null;
  obligationId: string | null;

  goToLibrary: () => void;
  goToWorkspace: (problemId: string) => void;
  goToAttempt: (problemId: string, attemptId: string) => void;
  goToStep: (problemId: string, attemptId: string, stepId: string) => void;
  goToObligation: (problemId: string, attemptId: string, obligationId: string) => void;
  goBack: () => void;
}

export const useNavigationStore = create<NavigationState>((set, get) => ({
  level: 'library',
  problemId: null,
  attemptId: null,
  stepId: null,
  obligationId: null,

  goToLibrary: () => set({ level: 'library', problemId: null, attemptId: null, stepId: null, obligationId: null }),

  goToWorkspace: (problemId) => set({ level: 'workspace', problemId, attemptId: null, stepId: null, obligationId: null }),

  goToAttempt: (problemId, attemptId) => set({ level: 'attempt', problemId, attemptId, stepId: null, obligationId: null }),

  goToStep: (problemId, attemptId, stepId) => set({ level: 'step', problemId, attemptId, stepId, obligationId: null }),

  goToObligation: (problemId, attemptId, obligationId) => set({ level: 'obligation', problemId, attemptId, obligationId, stepId: null }),

  goBack: () => {
    const { level } = get();
    switch (level) {
      case 'step':
        set({ level: 'attempt', stepId: null });
        break;
      case 'obligation':
        set({ level: 'attempt', obligationId: null });
        break;
      case 'attempt':
        set({ level: 'workspace', attemptId: null });
        break;
      case 'workspace':
        set({ level: 'library', problemId: null });
        break;
      default:
        break;
    }
  },
}));
