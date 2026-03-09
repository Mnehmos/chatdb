import { create } from 'zustand';
import type {
  AgentCouncilFindingPayload,
  AgentCriticEvaluationPayload,
  AgentOrchestratorEventPayload,
  AgentScoutResultPayload,
} from '../types';

export interface TimestampedAgentEvent<T> {
  ts: string;
  data: T;
}

interface AgentStore {
  orchestratorLog: TimestampedAgentEvent<AgentOrchestratorEventPayload>[];
  criticLog: TimestampedAgentEvent<AgentCriticEvaluationPayload>[];
  councilSessions: AgentCouncilFindingPayload[];
  scoutResults: AgentScoutResultPayload[];
  addOrchestratorEvent: (event: AgentOrchestratorEventPayload) => void;
  addCriticEvent: (event: AgentCriticEvaluationPayload) => void;
  addCouncilSession: (event: AgentCouncilFindingPayload) => void;
  addScoutResult: (event: AgentScoutResultPayload) => void;
}

export const useAgentStore = create<AgentStore>((set) => ({
  orchestratorLog: [],
  criticLog: [],
  councilSessions: [],
  scoutResults: [],
  addOrchestratorEvent: (event) => set((state) => ({
    orchestratorLog: [
      ...state.orchestratorLog,
      { ts: new Date().toISOString(), data: event },
    ],
  })),
  addCriticEvent: (event) => set((state) => ({
    criticLog: [
      ...state.criticLog,
      { ts: new Date().toISOString(), data: event },
    ],
  })),
  addCouncilSession: (event) => set((state) => ({
    councilSessions: [...state.councilSessions, event],
  })),
  addScoutResult: (event) => set((state) => ({
    scoutResults: [...state.scoutResults, event],
  })),
}));
