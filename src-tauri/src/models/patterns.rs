use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub id: String,
    pub name: String,
    pub description: String,
    pub domain: Option<String>,
    pub trigger: String,
    pub strategy: String,
    pub source_steps: String,
    pub success_count: u32,
    pub failure_count: u32,
    pub technique_class: Option<String>,
    pub deprecated: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
}
