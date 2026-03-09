use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofNode {
    pub id: String,
    pub attempt_id: String,
    pub branch_id: i32,
    pub node_type: String,
    pub parent_ids: Option<String>,
    pub content: String,
    pub formal_content: Option<String>,
    pub technique_class: Option<String>,
    pub construction_family: Option<String>,
    pub status: String,
    pub validator_used: Option<String>,
    pub validator_result: Option<String>,
    pub model_id: Option<String>,
    pub obligation_ref: Option<String>,
    pub opens_obligations: Option<String>,
    pub step_id: Option<String>,
    pub token_cost: Option<u32>,
    pub sequence_number: u32,
    pub created_at: String,
    pub verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Obligation {
    pub id: String,
    pub attempt_id: String,
    pub branch_id: i32,
    pub parent_node_id: String,
    pub description: String,
    pub obligation_type: String,
    pub priority: f64,
    pub confidence: f64,
    pub source_layer: Option<i32>,
    pub status: String,
    pub assigned_model: Option<String>,
    pub closure_node_id: Option<String>,
    pub closure_type: Option<String>,
    pub escalation_level: i32,
    pub steps_spent: i32,
    pub max_steps: i32,
    pub search_space: Option<String>,
    pub superseded_by: Option<String>,
    pub retraction_reason: Option<String>,
    pub depends_on: Option<String>,
    pub decomposition_id: Option<String>,
    pub satisfaction_criteria: Option<String>,
    // V14: Scout metadata
    pub signature_json: Option<String>,
    pub embedding_json: Option<String>,
    pub scout_status: Option<String>,
    pub last_scout_session_id: Option<String>,
    pub last_scout_confidence: Option<f64>,
    pub resolved_externally: bool,
    pub resolved_by_corpus_id: Option<String>,
    pub external_reference: Option<String>,
    pub scout_last_checked_at: Option<String>,
    // V15: Fan-in collaboration metadata
    pub assigned_models_json: Option<String>,
    pub active_solver_round_id: Option<String>,
    pub created_at: String,
    pub closed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagEvent {
    pub id: i64,
    pub attempt_id: String,
    pub event_type: String,
    pub payload: String,
    pub agent_role: String,
    pub sequence_number: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub id: i64,
    pub attempt_id: String,
    pub parent_branch: i32,
    pub fork_step: Option<i32>,
    pub fork_reason: Option<String>,
    pub direction: Option<String>,
    pub status: String,
    pub conclusion: Option<String>,
    pub conclusion_value: Option<String>,
    pub step_count: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechniqueEntry {
    pub id: i64,
    pub problem_class: String,
    pub technique_family: String,
    pub description: String,
    pub source: String,
    pub success_count: i32,
    pub failure_count: i32,
    pub last_used_at: Option<String>,
    pub created_at: String,
}
