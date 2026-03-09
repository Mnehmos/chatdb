use serde::{Deserialize, Serialize};

/// Structured mathematical assertion extracted from a verified step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub id: String,
    pub step_id: String,
    pub attempt_id: String,
    pub claim_type: String,
    pub object: String,
    pub scope_type: Option<String>,
    pub scope_param: Option<String>,
    pub scope_constraint: Option<String>,
    pub direction: Option<String>,
    pub value: Option<String>,
    pub natural_text: String,
    pub confidence: f64,
    pub verified: bool,
    pub superseded_by: Option<String>,
    pub created_at: String,
}

/// Unified DAG edge representing a relationship between any two entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagEdge {
    pub id: i64,
    pub source_id: String,
    pub target_id: String,
    pub source_type: String,
    pub target_type: String,
    pub edge_type: String,
    pub metadata: Option<String>,
    pub created_at: String,
}

/// Detected contradiction between two claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub id: String,
    pub attempt_id: String,
    pub claim_a_id: String,
    pub claim_b_id: String,
    pub conflict_type: String,
    pub severity: String,
    pub description: String,
    pub resolution: Option<String>,
    pub resolution_step: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
}

/// Persistent structured post-mortem for an attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AfterActionReportRecord {
    pub id: String,
    pub attempt_id: String,
    pub problem_id: String,
    pub coverage: Option<f64>,
    pub soundness: Option<f64>,
    pub budget_efficiency: Option<f64>,
    pub death_spirals: i32,
    pub contradictions: i32,
    pub obligations_total: i32,
    pub obligations_closed: i32,
    pub findings_json: Option<String>,
    pub recommendations: Option<String>,
    pub training_label: Option<String>,
    pub created_at: String,
}

/// Summary of an attempt with aggregated stats for the workspace view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptSummary {
    pub id: String,
    pub problem_id: String,
    pub attempt_number: Option<i32>,
    pub status: String,
    pub strategy: Option<String>,
    pub step_count: i32,
    pub steps_verified: i32,
    pub steps_rejected: i32,
    pub coverage: Option<f64>,
    pub efficiency: Option<f64>,
    pub models_used: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
}

/// Full detail of a single step, including related claims and DAG edges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDetail {
    pub id: String,
    pub attempt_id: String,
    pub parent_step_id: Option<String>,
    pub step_number: u32,
    pub model: String,
    pub goal_state: String,
    pub proposal_type: String,
    pub proposal_natural: String,
    pub proposal_formal: Option<String>,
    pub proposal_formal_lean: Option<String>,
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
    pub obligation_id: Option<String>,
    pub approach: Option<String>,
    pub failure_class: Option<String>,
    pub generator_model: Option<String>,
    pub created_at: String,
    pub claims: Vec<Claim>,
    pub edges: Vec<DagEdge>,
}
