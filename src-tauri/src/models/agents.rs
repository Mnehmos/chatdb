use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MultiAgentConfig {
    pub models: Vec<ModelConfig>,
    /// Optional separate model for audit/review/critic (defaults to solver model if None).
    #[serde(default)]
    pub reviewer_model: Option<ModelConfig>,
    /// Optional separate model for adversarial node challenges.
    /// Must be from a different model family than the solver for challenges to fire.
    #[serde(default)]
    pub adversary_model: Option<ModelConfig>,
    /// Optional separate model for the adversarial critic (obligation counterexample checks).
    /// Falls back to reviewer_model, then solver if not configured.
    #[serde(default)]
    pub critic_model: Option<ModelConfig>,
    /// Optional separate model for failure classification (mechanical vs. logical).
    /// When configured, runs after a failed attempt and classifies the failure mode
    /// to inform the retry strategy.
    #[serde(default)]
    pub discerner_model: Option<ModelConfig>,
    /// Optional separate model for strategic problem decomposition.
    /// Decomposes the problem into a typed obligation graph before solving begins.
    /// Falls back to reviewer_model, then solver if not configured.
    #[serde(default)]
    pub decomposer_model: Option<ModelConfig>,
    pub max_total_cost: u64,
    pub failure_threshold: u32,
    pub use_critic: bool,
    pub critic_skip_threshold: f32,
    pub use_council: bool,
    pub council_models: Vec<String>,
    /// Which research API sources the scout agent should query before solving.
    /// All sources enabled by default. Available: "arxiv", "semantic_scholar", "oeis", "wolfram", "loogle".
    #[serde(default = "default_scout_sources")]
    pub scout_sources: Vec<String>,
    /// Deprecated: use scout_sources instead. Kept for backward compatibility with saved profiles.
    #[serde(default = "default_true", skip_serializing)]
    pub use_scout: bool,
    pub use_patterns: bool,
    pub allow_self_modify: bool,
    pub max_attempts: u32,
    pub min_exploration_coverage: f64,
    pub min_conclusion_confidence: f64,
    /// Tool policy enforcement mode: "soft_and_hard", "soft_only", "disabled".
    #[serde(default = "default_tool_policy_mode")]
    pub tool_policy_mode: String,
    /// Enable obligation-level scout preflight before decomposition/solver.
    #[serde(default = "default_true")]
    pub obligation_scout_enabled: bool,
    /// Enable problem-level scout preflight before the main loop.
    #[serde(default = "default_true")]
    pub problem_scout_enabled: bool,
    /// Maximum number of scout sessions to run in parallel.
    #[serde(default = "default_scout_parallelism")]
    pub max_scout_parallelism: u32,
    /// When only one obligation is eligible and multiple solver models are configured,
    /// dispatch all solver models against that single obligation in parallel.
    #[serde(default = "default_true")]
    pub same_obligation_fanin_enabled: bool,
    /// Maximum number of solver workers to dispatch in a same-obligation fan-in round.
    #[serde(default = "default_fanin_workers")]
    pub max_fanin_workers: u32,
}

fn default_tool_policy_mode() -> String {
    "soft_and_hard".to_string()
}
fn default_true() -> bool {
    true
}
fn default_scout_sources() -> Vec<String> {
    vec![
        "arxiv".to_string(),
        "semantic_scholar".to_string(),
        "oeis".to_string(),
        "wolfram".to_string(),
        "loogle".to_string(),
        "tavily".to_string(),
    ]
}
fn default_scout_parallelism() -> u32 {
    4
}
fn default_fanin_workers() -> u32 {
    3
}

impl Default for MultiAgentConfig {
    fn default() -> Self {
        Self {
            models: vec![ModelConfig::default()],
            reviewer_model: None,
            adversary_model: None,
            critic_model: None,
            discerner_model: None,
            decomposer_model: None,
            max_total_cost: 100_000,
            failure_threshold: 5,
            use_critic: true,
            critic_skip_threshold: 0.8,
            use_council: true,
            council_models: vec![],
            scout_sources: vec![
                "arxiv".to_string(),
                "semantic_scholar".to_string(),
                "oeis".to_string(),
                "wolfram".to_string(),
                "loogle".to_string(),
                "tavily".to_string(),
            ],
            use_scout: true,
            use_patterns: true,
            allow_self_modify: false,
            max_attempts: 5,
            min_exploration_coverage: 0.6,
            min_conclusion_confidence: 0.7,
            tool_policy_mode: default_tool_policy_mode(),
            obligation_scout_enabled: true,
            problem_scout_enabled: true,
            max_scout_parallelism: 4,
            same_obligation_fanin_enabled: true,
            max_fanin_workers: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: String,
    pub model: String,
    pub api_key_ref: String,
    pub temperature: f32,
    pub max_budget_tokens: u64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            api_key_ref: "ANTHROPIC_API_KEY".to_string(),
            temperature: 0.3,
            max_budget_tokens: 50_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub config_json: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDataStats {
    pub total_steps: u64,
    pub verified_steps: u64,
    pub rejected_steps: u64,
    pub contrastive_pairs: u64,
    pub orchestrator_decisions: u64,
    pub council_sessions: u64,
    pub council_findings: u64,
    pub critic_evaluations: u64,
    pub scout_queries: u64,
    pub librarian_actions: u64,
}

#[cfg(test)]
mod tests {
    use super::ModelConfig;

    #[test]
    fn model_config_defaults_reasoning_level_to_high_when_missing() {
        let cfg: ModelConfig = serde_json::from_value(serde_json::json!({
            "provider": "anthropic",
            "model": "claude-sonnet-4-6",
            "api_key_ref": "ANTHROPIC_API_KEY",
            "temperature": 0.3,
            "max_budget_tokens": 50000
        }))
        .expect("legacy profiles should still deserialize");

        assert_eq!(cfg.reasoning_level, "high");
    }
}
