use crate::models::proof::LeanFormalizationResult;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const SIDECAR_URL: &str = "http://127.0.0.1:9743";

pub struct SidecarClient {
    client: Client,
    base_url: String,
}

impl Default for SidecarClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SidecarClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url: SIDECAR_URL.to_string(),
        }
    }

    pub async fn health_check(&self) -> Result<bool, reqwest::Error> {
        let resp = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await?;
        Ok(resp.status().is_success())
    }

    /// Check extended health — returns Lean readiness status.
    pub async fn health_extended(&self) -> Result<SidecarHealth, reqwest::Error> {
        self.client
            .get(format!("{}/health", self.base_url))
            .send()
            .await?
            .json()
            .await
    }

    /// Poll until the sidecar is ready (Lean warmed up or not available).
    /// Returns immediately if Lean is not installed. Times out after max_wait.
    pub async fn wait_until_ready(&self, max_wait: std::time::Duration) -> bool {
        let start = std::time::Instant::now();
        loop {
            if let Ok(h) = self.health_extended().await {
                // Ready if: Lean not available (nothing to warm), or Lean is warm
                if !h.lean_available || h.lean_ready {
                    return true;
                }
                tracing::info!(
                    "Sidecar: Lean warming up... ({:.0}s elapsed)",
                    start.elapsed().as_secs_f64()
                );
            } else if start.elapsed() > std::time::Duration::from_secs(5) {
                // Can't even reach sidecar
                tracing::warn!("Sidecar not reachable");
                return false;
            }
            if start.elapsed() > max_wait {
                tracing::warn!(
                    "Sidecar: Lean warmup timed out after {:.0}s",
                    max_wait.as_secs_f64()
                );
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }

    pub async fn validate_step(
        &self,
        request: &ValidateStepRequest,
    ) -> Result<ValidateStepResponse, reqwest::Error> {
        self.client
            .post(format!("{}/validate/step", self.base_url))
            .json(request)
            .send()
            .await?
            .json()
            .await
    }

    /// Pre-submission typed claim check for model self-correction.
    /// Verifies divisibility, inequality, gcd, congruence, for_all claims.
    pub async fn claim_check(&self, claim: &serde_json::Value) -> Option<ClaimCheckResponse> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap_or_default();
        client
            .post(format!("{}/validate/claim_check", self.base_url))
            .json(&serde_json::json!({"claim": claim}))
            .send()
            .await
            .ok()?
            .json::<ClaimCheckResponse>()
            .await
            .ok()
    }

    /// Fast pre-submission equality check for model self-correction.
    /// Returns the symbolic diff so the model can identify its algebraic error.
    /// Uses a short timeout (8s) — this runs before the full validation pipeline.
    pub async fn sympy_check(&self, lhs: &str, rhs: &str) -> Option<SympyCheckResponse> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap_or_default();
        client
            .post(format!("{}/validate/sympy_check", self.base_url))
            .json(&serde_json::json!({"lhs": lhs, "rhs": rhs}))
            .send()
            .await
            .ok()?
            .json::<SympyCheckResponse>()
            .await
            .ok()
    }

    /// Kernel-verify an equality via the persistent Lean REPL.
    /// Returns None on network/timeout errors (agent should proceed without Lean).
    pub async fn lean_check(
        &self,
        lhs: &str,
        rhs: &str,
        variables: &[&str],
    ) -> Option<LeanCheckResponse> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(50))
            .build()
            .unwrap_or_default();
        client
            .post(format!("{}/lean/check", self.base_url))
            .json(&serde_json::json!({
                "lhs": lhs,
                "rhs": rhs,
                "variables": variables,
            }))
            .send()
            .await
            .ok()?
            .json::<LeanCheckResponse>()
            .await
            .ok()
    }

    /// Send an arbitrary Lean 4 command to the persistent REPL.
    pub async fn lean_cmd(&self, cmd: &str, env: Option<i64>) -> Option<LeanCmdResponse> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(50))
            .build()
            .unwrap_or_default();
        let mut body = serde_json::json!({"cmd": cmd});
        if let Some(e) = env {
            body["env"] = serde_json::json!(e);
        }
        client
            .post(format!("{}/lean/cmd", self.base_url))
            .json(&body)
            .send()
            .await
            .ok()?
            .json::<LeanCmdResponse>()
            .await
            .ok()
    }

    /// Send a tactic to the Lean REPL against a proof state.
    pub async fn lean_tactic(&self, tactic: &str, proof_state: i64) -> Option<LeanTacticResponse> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(50))
            .build()
            .unwrap_or_default();
        client
            .post(format!("{}/lean/tactic", self.base_url))
            .json(&serde_json::json!({
                "tactic": tactic,
                "proof_state": proof_state,
            }))
            .send()
            .await
            .ok()?
            .json::<LeanTacticResponse>()
            .await
            .ok()
    }

    /// Formalize a completed proof into a standalone Lean theorem candidate.
    pub async fn formalize_proof(
        &self,
        request: &FormalizeProofRequest,
    ) -> Result<LeanFormalizationResult, reqwest::Error> {
        self.client
            .post(format!("{}/lean/formalize", self.base_url))
            .json(request)
            .send()
            .await?
            .json()
            .await
    }

    /// Search a research source via the sidecar research router.
    pub async fn research_search(
        &self,
        request: &ResearchSearchRequest,
    ) -> Result<serde_json::Value, reqwest::Error> {
        self.client
            .post(format!("{}/research/search", self.base_url))
            .json(request)
            .send()
            .await?
            .json()
            .await
    }

    /// Get a specific item from a research source.
    pub async fn research_get(
        &self,
        source: &str,
        id: &str,
    ) -> Result<serde_json::Value, reqwest::Error> {
        self.client
            .post(format!("{}/research/get", self.base_url))
            .json(&serde_json::json!({"source": source, "id": id}))
            .send()
            .await?
            .json()
            .await
    }

    /// Search multiple research sources in parallel.
    pub async fn research_multi(
        &self,
        query: &str,
        sources: &[String],
        max_per_source: u32,
    ) -> Result<serde_json::Value, reqwest::Error> {
        self.client
            .post(format!("{}/research/multi", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "sources": sources,
                "max_results_per_source": max_per_source,
            }))
            .send()
            .await?
            .json()
            .await
    }

    /// List available research sources.
    pub async fn research_sources(&self) -> Result<serde_json::Value, reqwest::Error> {
        self.client
            .get(format!("{}/research/sources", self.base_url))
            .send()
            .await?
            .json()
            .await
    }

    /// Run a scout briefing — queries multiple research sources for a problem
    /// and returns a compact text briefing for solver prompt injection.
    pub async fn scout_briefing(
        &self,
        query: &str,
        domain: Option<&str>,
        sources: &[String],
    ) -> Result<ScoutBriefingResponse, reqwest::Error> {
        self.client
            .post(format!("{}/agents/scout/query", self.base_url))
            .json(&serde_json::json!({
                "trigger_type": "pre_solve",
                "query": query,
                "domain": domain,
                "sources": sources,
                "max_results": 3,
            }))
            .send()
            .await?
            .json()
            .await
    }

    /// Run a mid-solve scout briefing for a specific stuck obligation.
    pub async fn scout_briefing_mid_solve(
        &self,
        query: &str,
        domain: Option<&str>,
        sources: &[String],
    ) -> Result<ScoutBriefingResponse, reqwest::Error> {
        self.client
            .post(format!("{}/agents/scout/query", self.base_url))
            .json(&serde_json::json!({
                "trigger_type": "mid_solve",
                "query": query,
                "domain": domain,
                "sources": sources,
                "max_results": 3,
            }))
            .send()
            .await?
            .json()
            .await
    }

    /// Run pre-decomposition reconnaissance — searches for THIS problem's
    /// known answer before the decomposer creates an obligation graph.
    pub async fn reconnaissance(
        &self,
        problem_statement: &str,
        problem_source: &str,
        problem_title: Option<&str>,
        domain: Option<&str>,
    ) -> Result<ReconnaissanceResponse, reqwest::Error> {
        self.client
            .post(format!("{}/agents/scout/reconnaissance", self.base_url))
            .json(&serde_json::json!({
                "problem_statement": problem_statement,
                "problem_source": problem_source,
                "problem_title": problem_title,
                "domain": domain,
            }))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?
            .json()
            .await
    }
}

#[derive(Debug, Deserialize)]
pub struct ReconnaissanceResponse {
    pub known_answer: Option<String>,
    #[serde(default)]
    pub candidate_answers: Vec<String>,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub proof_strategies: Vec<String>,
    #[serde(default)]
    pub key_references: Vec<String>,
    #[serde(default)]
    pub briefing: String,
}

#[derive(Debug, Deserialize)]
pub struct SidecarHealth {
    pub status: String,
    #[serde(default)]
    pub lean_available: bool,
    #[serde(default)]
    pub lean_warming_up: bool,
    #[serde(default)]
    pub lean_ready: bool,
    #[serde(default)]
    pub lean_warmup_attempts: u32,
}

#[derive(Debug, Serialize)]
pub struct ValidateStepRequest {
    pub proposal_type: String,
    pub proposal_natural: String,
    pub proposal_formal: Option<String>,
    /// Solver-produced Lean 4 expression — validated directly by Lean kernel,
    /// bypassing the SymPy→Lean translation layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_formal_lean: Option<String>,
    pub goal_state: String,
    /// When true, Lean council validation is requested (sampled, advisory).
    #[serde(default)]
    pub run_lean: bool,
    /// Problem domain (e.g., "algebra", "number_theory") for verifier selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem_domain: Option<String>,
    /// Typed claim object — dispatches to typed verifier instead of equality-only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_claim: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ValidateStepResponse {
    pub all_passed: bool,
    pub sympy: Option<ValidatorResultResp>,
    pub pint: Option<ValidatorResultResp>,
    pub lean: Option<ValidatorResultResp>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ValidatorResultResp {
    pub passed: bool,
    pub message: String,
    #[serde(default)]
    pub raw_output: String,
    #[serde(default)]
    pub wall_time_ms: u32,
}

#[derive(Debug, Deserialize)]
pub struct SympyCheckResponse {
    pub is_equal: bool,
    pub diff: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimCheckResponse {
    pub verified: bool,
    pub reason: String,
    #[serde(default)]
    pub wall_time_ms: u32,
}

#[derive(Debug, Deserialize)]
pub struct LeanCheckResponse {
    pub passed: bool,
    pub error: Option<String>,
    pub env: Option<i64>,
    #[serde(default)]
    pub wall_time_ms: u32,
}

#[derive(Debug, Deserialize)]
pub struct LeanCmdResponse {
    pub env: Option<i64>,
    #[serde(default)]
    pub messages: Vec<serde_json::Value>,
    #[serde(default)]
    pub sorries: Vec<serde_json::Value>,
    #[serde(default)]
    pub has_errors: bool,
}

#[derive(Debug, Deserialize)]
pub struct LeanTacticResponse {
    pub proof_state: Option<i64>,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub is_complete: bool,
    #[serde(default)]
    pub messages: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Clone)]
pub struct FormalizeProofRequest {
    pub problem_id: String,
    pub problem_statement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem_domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem_formal_statement: Option<String>,
    pub attempt_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_answer: Option<String>,
    pub verified_chain: Vec<FormalizeProofStep>,
    #[serde(default)]
    pub obligations: Vec<FormalizeProofObligation>,
}

#[derive(Debug, Serialize, Clone)]
pub struct FormalizeProofStep {
    pub step_number: u32,
    pub proposal_type: String,
    pub natural: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formal: Option<String>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obligation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obligation_desc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obligation_type: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct FormalizeProofObligation {
    pub id: String,
    pub description: String,
    pub obligation_type: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct ScoutBriefingResponse {
    pub query: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub sources_queried: Vec<String>,
    #[serde(default)]
    pub results_count: u32,
    /// Compact text briefing ready for solver prompt injection.
    #[serde(default)]
    pub briefing: String,
}

#[derive(Debug, Serialize)]
pub struct ResearchSearchRequest {
    pub source: String,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields_of_study: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db: Option<String>,
}
