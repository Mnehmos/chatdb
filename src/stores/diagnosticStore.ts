import { create } from 'zustand';
import type { DiagnosticEvent, DiagnosticCategory, DiagnosticSeverity, SystemHealth } from '../types';
import * as api from '../services/tauri';

const MAX_EVENTS = 500;

interface DiagnosticStore {
  events: DiagnosticEvent[];
  health: SystemHealth | null;
  filterCategory: DiagnosticCategory | null;
  filterSeverity: DiagnosticSeverity | null;
  expanded: Set<number>;
  open: boolean;
  addEvent: (e: DiagnosticEvent) => void;
  setHealth: (h: SystemHealth) => void;
  setFilterCategory: (c: DiagnosticCategory | null) => void;
  setFilterSeverity: (s: DiagnosticSeverity | null) => void;
  toggleExpanded: (idx: number) => void;
  setOpen: (open: boolean) => void;
  clear: () => void;
  refreshHealth: () => Promise<void>;
}

export const useDiagnosticStore = create<DiagnosticStore>((set) => ({
  events: [],
  health: null,
  filterCategory: null,
  filterSeverity: null,
  expanded: new Set(),
  open: (() => { try { return localStorage.getItem('diag_open') === '1'; } catch { return false; } })(),
  addEvent: (e) => set((s) => {
    const next = [...s.events, e];
    if (next.length > MAX_EVENTS) next.splice(0, next.length - MAX_EVENTS);
    return { events: next };
  }),
  setHealth: (h) => set({ health: h }),
  setFilterCategory: (c) => set({ filterCategory: c }),
  setFilterSeverity: (s) => set({ filterSeverity: s }),
  toggleExpanded: (idx) => set((s) => {
    const next = new Set(s.expanded);
    if (next.has(idx)) next.delete(idx); else next.add(idx);
    return { expanded: next };
  }),
  setOpen: (open) => { try { localStorage.setItem('diag_open', open ? '1' : '0'); } catch {} set({ open }); },
  clear: () => set({
    events: [],
    expanded: new Set(),
    filterCategory: null,
    filterSeverity: null,
  }),
  refreshHealth: async () => {
    try {
      const h = await api.getSystemHealth();
      set({ health: h });
    } catch { /* ignore */ }
  },
}));
