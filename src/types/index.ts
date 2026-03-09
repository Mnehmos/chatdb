export interface Problem {
  id: string; statement: string; formal_statement?: string; domain: string;
  source: string; status: 'open' | 'solved' | 'abandoned';
  created_at: string; solved_at?: string; total_attempts: number; total_steps: number;
  known_answer?: string;
  // V10: Management System additions
  title?: string; difficulty?: string; metadata?: string;
}
export interface Step {
  id: string; attempt_id: string; parent_step_id?: string; step_number: number;
  model: string; goal_state: string; proposal_type: string;
  proposal_natural: string; proposal_formal?: string; proposal_reasoning?: string;
  verified: boolean; rejection_reason?: string;
  sympy_passed?: boolean; pint_passed?: boolean; lean_passed?: boolean;
  critic_prediction?: string; created_at: string;
  // Adversarial challenge results
  challenge_model?: string;
  challenge_flaw_found?: boolean;
  challenge_attack?: string;
  challenge_confidence?: number;
  challenge_fatal?: boolean;
  // Obligation targeting
  obligation_id?: string;
  obligation_desc?: string;
  obligation_type?: string;
  // V14: Solver round
  solver_round_id?: string;
  // V15: Fan-in metadata
  solver_worker_id?: string;
  solver_dispatch_mode?: string;
  stale_sibling?: boolean;
}
export interface Pattern {
  id: string; name: string; description: string; domain?: string;
  trigger: string; strategy: string; success_count: number; failure_count: number;
  technique_class?: string; deprecated: boolean;
}
export interface MultiAgentConfig {
  models: ModelConfig[]; reviewer_model?: ModelConfig; adversary_model?: ModelConfig; critic_model?: ModelConfig; discerner_model?: ModelConfig; decomposer_model?: ModelConfig; max_total_cost: number; failure_threshold: number;
  use_critic: boolean; critic_skip_threshold: number;
  use_council: boolean; council_models: string[];
  /** Which research API sources the scout agent queries before solving. Empty = disabled. */
  scout_sources: string[];
  use_patterns: boolean; allow_self_modify: boolean;
  max_attempts: number; min_exploration_coverage: number; min_conclusion_confidence: number;
  same_obligation_fanin_enabled?: boolean;
  max_fanin_workers?: number;
}
export interface ModelConfig {
  provider: string; model: string; api_key_ref: string;
  temperature: number; max_budget_tokens: number;
}
export interface TrainingDataStats {
  total_steps: number; verified_steps: number; rejected_steps: number;
  contrastive_pairs: number; orchestrator_decisions: number;
  council_sessions: number; council_findings: number;
  critic_evaluations: number; scout_queries: number; librarian_actions: number;
}
export interface TrainingRow {
  id: string; attempt_id: string; problem_id: string; step_number: number; model: string;
  proposal_type: string; proposal_natural: string; proposal_formal?: string;
  verified: boolean; rejection_reason?: string;
  sympy_passed?: boolean; pint_passed?: boolean; lean_passed?: boolean;
  obligation_id?: string; obligation_desc?: string; obligation_type?: string;
  stale_sibling?: boolean;
  semantic_redundant?: boolean;
  created_at: string; problem_statement: string; problem_domain?: string;
}
export interface AfterActionReport {
  problem_id: string; problem_statement: string; problem_domain: string;
  attempt_id: string; total_steps: number; verified_steps: number;
  rejected_steps: number; accuracy_pct: number;
  total_tokens_in: number; total_tokens_out: number; total_wall_ms: number;
  models_used: string[]; verified_chain: ChainStep[];
  failure_modes: FailureMode[]; started_at: string;
  proof_complete: boolean; open_obligations: number;
  final_answer?: string;
}
export interface LeanFormalizationResult {
  success: boolean;
  lean_source: string;
  errors: string[];
  attempts: number;
}
export interface ChainStep {
  step_number: number; natural: string; formal?: string; model: string;
}
export interface FailureMode { reason: string; count: number; }
export * from './generated/loopEvents';
export type LoopStatus = 'idle' | 'running' | 'paused' | 'finished';
export interface AgentProfile {
  id: string;
  name: string;
  description?: string;
  config_json: string;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}
export interface SystemHealth {
  sidecar_reachable: boolean;
  lean_available: boolean;
  lean_ready: boolean;
  lean_warming_up: boolean;
  lean_warmup_attempts: number;
  active_attempt: string | null;
  loop_running: boolean;
}

// ---- Management System types (V10) ----

export interface Claim {
  id: string; step_id: string; attempt_id: string; claim_type: string;
  object: string; scope_type?: string; scope_param?: string; scope_constraint?: string;
  direction?: string; value?: string; natural_text: string; confidence: number;
  verified: boolean; superseded_by?: string; created_at: string;
}
export interface DagEdge {
  id: number; source_id: string; target_id: string;
  source_type: string; target_type: string; edge_type: string;
  metadata?: string; created_at: string;
}
export interface Conflict {
  id: string; attempt_id: string; claim_a_id: string; claim_b_id: string;
  conflict_type: string; severity: string; description: string;
  resolution?: string; resolution_step?: string; resolved_at?: string; created_at: string;
}
export interface AfterActionReportRecord {
  id: string; attempt_id: string; problem_id: string;
  coverage?: number; soundness?: number; budget_efficiency?: number;
  death_spirals: number; contradictions: number;
  obligations_total: number; obligations_closed: number;
  findings_json?: string; recommendations?: string; training_label?: string;
  created_at: string;
}
export interface AttemptSummary {
  id: string; problem_id: string; attempt_number?: number;
  status: string; strategy?: string;
  step_count: number; steps_verified: number; steps_rejected: number;
  coverage?: number; efficiency?: number; models_used?: string;
  started_at: string; ended_at?: string;
}
export type ViewLevel = 'library' | 'workspace' | 'attempt' | 'obligation' | 'step';

export interface ProofNode {
  id: string;
  attempt_id: string;
  branch_id: number;
  node_type: string;
  parent_ids?: string;
  content: string;
  formal_content?: string;
  technique_class?: string;
  construction_family?: string;
  status: string;
  validator_used?: string;
  validator_result?: string;
  model_id?: string;
  obligation_ref?: string;
  opens_obligations?: string;
  step_id?: string;
  token_cost?: number;
  sequence_number: number;
  created_at: string;
  verified_at?: string;
}

export interface SatisfactionSignal {
  id: string;
  obligation_id: string;
  step_id?: string;
  source: string;
  model_id?: string;
  satisfies: boolean;
  confidence: number;
  note?: string;
  created_at: string;
}

export interface ObligationFull {
  id: string;
  attempt_id: string;
  branch_id: number;
  parent_node_id: string;
  description: string;
  obligation_type: string;
  priority: number;
  confidence: number;
  source_layer?: number;
  status: string;
  assigned_model?: string;
  closure_node_id?: string;
  closure_type?: string;
  escalation_level: number;
  steps_spent: number;
  max_steps: number;
  search_space?: string;
  superseded_by?: string;
  retraction_reason?: string;
  depends_on?: string;
  decomposition_id?: string;
  satisfaction_criteria?: string;
  // V15: Fan-in collaboration
  assigned_models_json?: string;
  active_solver_round_id?: string;
  created_at: string;
  closed_at?: string;
}

// ---- Tool Run types ----

export interface ToolRun {
  id: string;
  attempt_id: string;
  branch_id?: number;
  step_id?: string;
  step_number?: number;
  obligation_id?: string;
  session_id?: string;
  agent_role: string;
  trigger_kind: string;
  tool_name: string;
  provider: string;
  status: string;
  latency_ms?: number;
  error_message?: string;
  result_summary?: string;
  query_json: string;
  created_at: string;
  completed_at?: string;
}

// ---- Research API types ----

export interface ResearchApiKeyInfo {
  id: string;
  service: string;
  key_masked: string;
  label?: string;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface ResearchSource {
  id: string;
  name: string;
  capabilities: string[];
  requires_key: boolean;
  key_optional?: boolean;
  requires_email?: boolean;
}

export interface ResearchSearchResult {
  query: string;
  total?: number;
  returned?: number;
  results?: unknown[];
  error?: string;
  [key: string]: unknown;
}
