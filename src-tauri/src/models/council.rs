use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouncilSession {
    pub id: String,
    pub trigger_type: String,
    pub problem_id: String,
    pub attempt_id: Option<String>,
    pub council_models: Vec<String>,
    pub transcript: String,
    pub findings_count: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouncilFinding {
    pub id: String,
    pub session_id: String,
    pub finding_type: String,
    pub summary: String,
    pub detail: String,
    pub consensus: String,
    pub dissent: Option<String>,
    pub target_agent: Option<String>,
    pub acted_on: bool,
}
