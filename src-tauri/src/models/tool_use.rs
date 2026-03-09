use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRun {
    pub id: String,
    pub attempt_id: String,
    pub branch_id: Option<i32>,
    pub step_id: Option<String>,
    pub step_number: Option<u32>,
    pub obligation_id: Option<String>,
    pub parent_tool_run_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_role: String,
    pub trigger_kind: String,
    pub tool_name: String,
    pub provider: String,
    pub tier: Option<i32>,
    pub query_json: String,
    pub input_hash: Option<String>,
    pub required_by_policy: bool,
    pub status: String,
    pub hit: Option<bool>,
    pub latency_ms: Option<u32>,
    pub error_message: Option<String>,
    pub raw_result_json: Option<String>,
    pub result_summary: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolArtifact {
    pub id: String,
    pub tool_run_id: String,
    pub attempt_id: String,
    pub obligation_id: Option<String>,
    pub artifact_index: i32,
    pub artifact_kind: String,
    pub external_ref: Option<String>,
    pub title: Option<String>,
    pub statement: Option<String>,
    pub formalism: Option<String>,
    pub url: Option<String>,
    pub relevance_score: Option<f64>,
    pub matched: bool,
    pub raw_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoutSession {
    pub id: String,
    pub attempt_id: String,
    pub branch_id: Option<i32>,
    pub problem_id: String,
    pub obligation_id: Option<String>,
    pub scope: String,
    pub trigger_type: String,
    pub routing_plan_json: String,
    pub status: String,
    pub best_resolution_status: Option<String>,
    pub best_tool_run_id: Option<String>,
    pub best_artifact_id: Option<String>,
    pub confidence: Option<f64>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObligationResolution {
    pub id: String,
    pub attempt_id: String,
    pub obligation_id: String,
    pub scout_session_id: String,
    pub status: String,
    pub source_tier: i32,
    pub source_provider: String,
    pub source_reference: String,
    pub source_url: Option<String>,
    pub source_formalism: String,
    pub mapping_text: String,
    pub translation_json: Option<String>,
    pub mechanical_verification: bool,
    pub confidence: f64,
    pub corpus_entry_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionCorpusEntry {
    pub id: String,
    pub canonical_statement: String,
    pub canonical_hash: String,
    pub obligation_type: String,
    pub problem_domain: Option<String>,
    pub signature_json: String,
    pub embedding_json: Option<String>,
    pub source_tier: i32,
    pub source_provider: String,
    pub source_reference: String,
    pub source_url: Option<String>,
    pub source_formalism: String,
    pub mapping_text: String,
    pub translation_json: String,
    pub mechanical_verification: bool,
    pub verification_method: Option<String>,
    pub lean_proof_term: Option<String>,
    pub tags_json: Option<String>,
    pub reuse_count: i32,
    pub last_reused_at: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusUsage {
    pub id: String,
    pub corpus_entry_id: String,
    pub attempt_id: String,
    pub problem_id: String,
    pub obligation_id: String,
    pub scout_session_id: String,
    pub resolution_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicyViolation {
    pub id: String,
    pub attempt_id: String,
    pub branch_id: Option<i32>,
    pub step_id: Option<String>,
    pub step_number: Option<u32>,
    pub obligation_id: Option<String>,
    pub agent_role: String,
    pub policy_name: String,
    pub required_tool: String,
    pub observed_tool_runs_json: Option<String>,
    pub action_taken: String,
    pub notes: Option<String>,
    pub created_at: String,
}
