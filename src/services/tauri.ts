import { invoke } from '@tauri-apps/api/core';
import type { Problem, Pattern, TrainingDataStats, MultiAgentConfig, Step, TrainingRow, AfterActionReport, AgentProfile, AttemptSummary, Claim, Conflict, DagEdge, AfterActionReportRecord, ObligationFull, ProofNode, SatisfactionSignal, ToolRun, LeanFormalizationResult } from '../types';

export const createProblem = (statement: string, domain: string) =>
  invoke<Problem>('create_problem', { statement, domain });
export const getProblem = (id: string) => invoke<Problem>('get_problem', { id });
export const listProblems = (status?: string) => invoke<Problem[]>('list_problems', { status });
export const startSolve = (problemId: string, config?: MultiAgentConfig, force?: boolean) =>
  invoke<string>('start_solve', { problemId, config, force });
export const continueSolve = (problemId: string, config?: MultiAgentConfig, attemptId?: string) =>
  invoke<string>('continue_solve', { problemId, config, attemptId });
export const pauseSolve = () => invoke<void>('pause_solve');
export const stopSolve = () => invoke<void>('stop_solve');
export const getLoopStatus = () =>
  invoke<{ running: boolean; attempt_id: string | null }>('get_loop_status');
export const getVerifiedChain = (attemptId: string) =>
  invoke<[string,number,string,string,string][]>('get_verified_chain', { attemptId });
export const searchPatterns = (query: string) => invoke<Pattern[]>('search_patterns', { query });
export const getTrainingDataStats = () => invoke<TrainingDataStats>('get_training_data_stats');
export const getProblemSteps = (problemId: string) => invoke<Step[]>('get_problem_steps', { problemId });
export const getAttemptSteps = (attemptId: string) => invoke<Step[]>('get_attempt_steps', { attemptId });
// Raw DB obligation — map to ObligationEvent client-side
export interface DbObligation {
  id: string; attempt_id: string; description: string; obligation_type: string;
  priority: number; status: string; assigned_model: string | null;
  steps_spent: number; max_steps: number;
}
export const getObligationGraph = (attemptId: string) =>
  invoke<DbObligation[]>('get_obligation_graph', { attemptId });
export const getObligationDetail = (obligationId: string) =>
  invoke<ObligationFull>('get_obligation_detail', { obligationId });
export const getObligationProofNodes = (obligationId: string) =>
  invoke<ProofNode[]>('get_obligation_proof_nodes', { obligationId });
export const getObligationSignals = (obligationId: string) =>
  invoke<SatisfactionSignal[]>('get_obligation_signals', { obligationId });
export const getToolRunsForStep = (stepId: string) =>
  invoke<ToolRun[]>('get_tool_runs_for_step', { stepId });
export const getToolRunsForObligation = (obligationId: string) =>
  invoke<ToolRun[]>('get_tool_runs_for_obligation', { obligationId });
export const listAllSteps = (limit?: number) => invoke<TrainingRow[]>('list_all_steps', { limit });
export const getAfterActionReport = (problemId: string) => invoke<AfterActionReport>('get_after_action_report', { problemId });
export const formalizeProof = (problemId: string) => invoke<LeanFormalizationResult>('formalize_proof', { problemId });
export const runManualReview = (problemId: string, config?: MultiAgentConfig) =>
  invoke<Record<string, unknown>>('run_manual_review', { problemId, config });
export const saveProfile = (name: string, configJson: string, description?: string, isDefault?: boolean) =>
  invoke<AgentProfile>('save_profile', {
    name,
    configJson,
    // Tauri IPC: explicitly pass null instead of undefined for Option<T> params
    description: description ?? null,
    isDefault: isDefault ?? null,
  });
export const testProfileRoundtrip = () => invoke<string>('test_profile_roundtrip');
export const loadProfile = (id: string) => invoke<AgentProfile>('load_profile', { id });
export const listProfiles = () => invoke<AgentProfile[]>('list_profiles');
export const deleteProfile = (id: string) => invoke<void>('delete_profile', { id });
export const setDefaultProfile = (id: string) => invoke<void>('set_default_profile', { id });
export const getDefaultProfile = () => invoke<AgentProfile | null>('get_default_profile');
export const getDiagnosticEvents = (attemptId: string, sinceSequence?: number) =>
  invoke<{ id: number; attempt_id: string; event_type: string; payload: string; agent_role: string; sequence_number: number; created_at: string }[]>('get_diagnostic_events', { attemptId, sinceSequence });
export const getSystemHealth = () =>
  invoke<import('../types').SystemHealth>('get_system_health');

// ---- Management System commands ----
export const listAttempts = (problemId: string) =>
  invoke<AttemptSummary[]>('list_attempts', { problemId });
export const getClaimsForAttempt = (attemptId: string) =>
  invoke<Claim[]>('get_claims_for_attempt', { attemptId });
export const getClaimsForStep = (stepId: string) =>
  invoke<Claim[]>('get_claims_for_step', { stepId });
export const getConflictsForAttempt = (attemptId: string) =>
  invoke<Conflict[]>('get_conflicts_for_attempt', { attemptId });
export const getDagEdgesFrom = (sourceId: string) =>
  invoke<DagEdge[]>('get_dag_edges_from', { sourceId });
export const getDagEdgesTo = (targetId: string) =>
  invoke<DagEdge[]>('get_dag_edges_to', { targetId });
export const getReportForAttempt = (attemptId: string) =>
  invoke<AfterActionReportRecord>('get_report_for_attempt', { attemptId });
export const getReportsForProblem = (problemId: string) =>
  invoke<AfterActionReportRecord[]>('get_reports_for_problem', { problemId });
export const searchProblems = (query: string, domain?: string, status?: string, difficulty?: string) =>
  invoke<Problem[]>('search_problems', { query, domain, status, difficulty });
export const updateProblemTitle = (id: string, title: string) =>
  invoke<void>('update_problem_title', { id, title });
export const deleteAttempt = (attemptId: string) =>
  invoke<void>('delete_attempt', { attemptId });

// ---- Export commands ----
export const exportTrainingData = (exportType: string, outputPath: string, problemId?: string) =>
  invoke<number>('export_training_data', { problemId, exportType, outputPath });
export const getExportDirectory = () =>
  invoke<string>('get_export_directory');

// ---- Backfill commands ----
export const backfillDagEdges = () =>
  invoke<number>('backfill_dag_edges');

// ---- Research API commands ----
export const setResearchApiKey = (service: string, keyValue: string, label?: string) =>
  invoke<import('../types').ResearchApiKeyInfo>('set_research_api_key', {
    service, keyValue, label: label ?? null,
  });
export const listResearchApiKeys = () =>
  invoke<import('../types').ResearchApiKeyInfo[]>('list_research_api_keys');
export const deleteResearchApiKey = (service: string) =>
  invoke<void>('delete_research_api_key', { service });
export const toggleResearchApiKey = (service: string, active: boolean) =>
  invoke<void>('toggle_research_api_key', { service, active });
export const researchSearch = (source: string, query: string, maxResults?: number, sort?: string) =>
  invoke<Record<string, unknown>>('research_search', {
    source, query, maxResults: maxResults ?? null, sort: sort ?? null,
  });
export const researchGet = (source: string, id: string) =>
  invoke<Record<string, unknown>>('research_get', { source, id });
export const researchMulti = (query: string, sources: string[], maxPerSource?: number) =>
  invoke<Record<string, unknown>>('research_multi', {
    query, sources, maxPerSource: maxPerSource ?? null,
  });
export const researchSources = () =>
  invoke<{ sources: import('../types').ResearchSource[] }>('research_sources');

// ChatGPT OAuth
export interface DeviceFlowInfo {
  device_auth_id: string; user_code: string; verification_url: string; interval: number;
}
export interface OAuthStatus { authenticated: boolean; expires_at: number | null; }
export interface PollResult { complete: boolean; error: string | null; }
export const chatgptOAuthStart = () => invoke<DeviceFlowInfo>('chatgpt_oauth_start');
export const chatgptOAuthPoll = (deviceAuthId: string, userCode: string) => invoke<PollResult>('chatgpt_oauth_poll', { deviceAuthId, userCode });
export const chatgptOAuthStatus = () => invoke<OAuthStatus>('chatgpt_oauth_status');
export const chatgptOAuthLogout = () => invoke<void>('chatgpt_oauth_logout');
