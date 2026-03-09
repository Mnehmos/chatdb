use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    pub id: String,
    pub statement: String,
    pub formal_statement: Option<String>,
    pub domain: String,
    pub source: String,
    pub status: String,
    pub created_at: String,
    pub solved_at: Option<String>,
    pub total_attempts: u32,
    pub total_steps: u32,
    pub known_answer: Option<String>,
    // V10: Management System additions
    pub title: Option<String>,
    pub difficulty: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub attempt_id: String,
    pub parent_step_id: Option<String>,
    pub step_number: u32,
    pub model: String,
    pub goal_state: String,
    pub proposal_type: String,
    pub proposal_natural: String,
    pub proposal_formal: Option<String>,
    pub proposal_reasoning: Option<String>,
    pub verified: bool,
    pub rejection_reason: Option<String>,
    pub sympy_passed: Option<bool>,
    pub pint_passed: Option<bool>,
    pub lean_passed: Option<bool>,
    pub challenge_model: Option<String>,
    pub challenge_flaw_found: Option<bool>,
    pub challenge_attack: Option<String>,
    pub challenge_confidence: Option<f64>,
    pub challenge_fatal: Option<bool>,
    // V14: Obligation linkage
    pub obligation_id: Option<String>,
    pub obligation_desc: Option<String>,
    pub obligation_type: Option<String>,
    // V14: Solver round
    pub solver_round_id: Option<String>,
    // V15: Fan-in metadata
    pub solver_worker_id: Option<String>,
    pub solver_dispatch_mode: Option<String>,
    pub stale_sibling: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingRow {
    pub id: String,
    pub attempt_id: String,
    pub problem_id: String,
    pub step_number: u32,
    pub model: String,
    pub proposal_type: String,
    pub proposal_natural: String,
    pub proposal_formal: Option<String>,
    pub verified: bool,
    pub rejection_reason: Option<String>,
    pub sympy_passed: Option<bool>,
    pub pint_passed: Option<bool>,
    pub lean_passed: Option<bool>,
    pub obligation_id: Option<String>,
    pub obligation_desc: Option<String>,
    pub obligation_type: Option<String>,
    pub stale_sibling: bool,
    pub semantic_redundant: bool,
    pub created_at: String,
    pub problem_statement: String,
    pub problem_domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AfterActionReport {
    pub problem_id: String,
    pub problem_statement: String,
    pub problem_domain: String,
    pub attempt_id: String,
    pub total_steps: u32,
    pub verified_steps: u32,
    pub rejected_steps: u32,
    pub accuracy_pct: f64,
    pub total_tokens_in: u64,
    pub total_tokens_out: u64,
    pub total_wall_ms: u64,
    pub models_used: Vec<String>,
    pub verified_chain: Vec<ChainStep>,
    pub failure_modes: Vec<FailureMode>,
    pub started_at: String,
    pub proof_complete: bool,
    pub open_obligations: u32,
    pub final_answer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeanFormalizationResult {
    pub success: bool,
    pub lean_source: String,
    pub errors: Vec<String>,
    pub attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStep {
    pub step_number: u32,
    pub natural: String,
    pub formal: Option<String>,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureMode {
    pub reason: String,
    pub count: u32,
}
