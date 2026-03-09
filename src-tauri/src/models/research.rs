use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoutQuery {
    pub id: String,
    pub trigger_type: String,
    pub query_text: String,
    pub sources_queried: String,
    pub results_summary: Option<String>,
    pub helped: Option<bool>,
}
