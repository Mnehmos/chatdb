pub mod audit;
pub mod claim_extractor;
pub mod context_enricher;
pub mod contradiction;
pub mod critic;
pub mod decomposer;
pub mod discerner;
pub mod evidence;
pub mod json_parse;
pub mod obligation_queue;
pub mod orchestrator;
pub mod patterns;
pub mod research;
pub mod response_guard;
pub mod review;
pub mod satisfaction;
pub mod solver;
pub(crate) mod step;
pub mod worker_pool;

use crate::api::llm_client::LlmClient;
use crate::contracts::loop_events::{
    AgentOrchestratorEventPayload, AgentScoutResultPayload, DiagnosticCategory, DiagnosticEvent,
    DiagnosticSeverity, EmptyPayload, ExtractedPattern, LoopAttemptStartPayload,
    LoopOuterCompletePayload, LoopPatternsExtractedPayload, LoopRetryPayload,
    LoopReviewStartPayload, LoopSidecarWarmupPayload, LoopStartedData, LoopStartedPayload,
    LoopStepCompletePayload, LoopThinkingEndPayload, LoopThinkingStartPayload, LoopTokenPayload,
    ReviewFinding, ReviewResult as ContractReviewResult, ScoutTrigger, SidecarWarmupStatus,
};
use crate::models::agents::MultiAgentConfig;
use crate::verification::VerificationPipeline;
use crate::AppState;
use std::sync::Arc;
use tauri::Emitter;

/// Resolve an API key from env var or OAuth token store.
/// For `CHATGPT_OAUTH_TOKEN`, tries the in-memory OAuth token first.
pub fn resolve_api_key(api_key_ref: &str) -> Result<String, String> {
    if api_key_ref == "CHATGPT_OAUTH_TOKEN" {
        if let Some(token) = crate::api::chatgpt_oauth::get_token_sync() {
            return Ok(token);
        }
        return Err(
            "No ChatGPT OAuth token. Please authenticate via Settings → ChatGPT.".to_string(),
        );
    }
    std::env::var(api_key_ref).map_err(|_| format!("Missing env var: {}", api_key_ref))
}

/// Emit a structured diagnostic event for the real-time log checker.
/// Categories: "model" (LLM produced wrong output), "mechanical" (infra/API failure),
/// "validator" (CAS/Lean result), "gate" (obligation/answer gate), "info" (neutral status).
/// Severities: "info", "warn", "error", "fatal".
fn emit_diagnostic(
    app: &tauri::AppHandle,
    category: &str,
    severity: &str,
    source: &str,
    step_number: Option<u32>,
    message: &str,
    detail: serde_json::Value,
    attempt_id: &str,
) {
    let category = match category {
        "model" => DiagnosticCategory::Model,
        "mechanical" => DiagnosticCategory::Mechanical,
        "validator" => DiagnosticCategory::Validator,
        "gate" => DiagnosticCategory::Gate,
        _ => DiagnosticCategory::Info,
    };
    let severity = match severity {
        "warn" => DiagnosticSeverity::Warn,
        "error" => DiagnosticSeverity::Error,
        "fatal" => DiagnosticSeverity::Fatal,
        _ => DiagnosticSeverity::Info,
    };
    let _ = app.emit(
        "loop:diagnostic",
        DiagnosticEvent {
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            category,
            severity,
            source: source.to_string(),
            step_number,
            message: message.to_string(),
            detail: Some(detail),
            attempt_id: Some(attempt_id.to_string()),
        },
    );
}

/// Truncate a string to at most `max` characters (for log messages).
fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

pub type StepEvent = LoopStepCompletePayload;
pub type LoopEvent = LoopStartedPayload;

/// Outcome of a single attempt run — used by the outer loop to decide retry.
#[derive(Debug, Clone)]
pub struct AttemptOutcome {
    pub attempt_id: String,
    pub steps_processed: u32,
    pub review: Option<review::ReviewResult>,
    pub stopped_by_user: bool,
    pub proof_complete: bool,
    /// Failure classification from the Discerner (None if Discerner not configured or attempt succeeded).
    pub discerner_verdict: Option<discerner::FailureClassification>,
}

pub struct LoopEngine {
    state: Arc<AppState>,
    config: MultiAgentConfig,
    problem_id: String,
    attempt_id: String,
    starting_step: u32,
    /// Structural constraints for this attempt — extracted from prior failed attempts.
    /// These are injected into every solver prompt and are non-negotiable.
    attempt_constraints: Vec<String>,
}

impl LoopEngine {
    pub fn new(
        state: Arc<AppState>,
        config: MultiAgentConfig,
        problem_id: String,
        attempt_id: String,
        starting_step: u32,
    ) -> Self {
        Self {
            state,
            config,
            problem_id,
            attempt_id,
            starting_step,
            attempt_constraints: Vec::new(),
        }
    }

    pub fn with_constraints(mut self, constraints: Vec<String>) -> Self {
        self.attempt_constraints = constraints;
        self
    }

    pub async fn run(&self, app_handle: tauri::AppHandle) -> Result<AttemptOutcome, String> {
        tracing::info!(
            "Loop engine started for problem {} attempt {} from step {}",
            self.problem_id,
            self.attempt_id,
            self.starting_step
        );
        emit_diagnostic(
            &app_handle,
            "info",
            "info",
            "engine",
            None,
            &format!("Loop started: attempt from step {}", self.starting_step),
            serde_json::json!({"problem_id": &self.problem_id}),
            &self.attempt_id,
        );

        // Build solver worker pool from all configured models
        if self.config.models.is_empty() {
            return Err("No models configured".to_string());
        }
        let mut solver_workers: Vec<step::SolverWorker> = Vec::new();
        for (i, mcfg) in self.config.models.iter().enumerate() {
            let key = resolve_api_key(&mcfg.api_key_ref)?;
            let budget = (mcfg.max_budget_tokens as u32).clamp(2_000, 16_000);
            let client = LlmClient::with_budget(&mcfg.provider, &mcfg.model, &key, budget)
                .with_temperature(mcfg.temperature);
            tracing::info!(
                "Solver worker {}: {} (budget: {}, temp: {})",
                i,
                mcfg.model,
                budget,
                mcfg.temperature
            );
            solver_workers.push(step::SolverWorker {
                worker_id: format!("solver-{}", i),
                model_name: mcfg.model.clone(),
                llm: client,
            });
        }
        // Primary solver is always workers[0] for backward compat
        let model_cfg = self.config.models.first().ok_or_else(|| {
            "No models configured. Add at least one model to the profile.".to_string()
        })?;
        let api_key = resolve_api_key(&model_cfg.api_key_ref)?;
        let solver_budget = (model_cfg.max_budget_tokens as u32).clamp(2_000, 16_000);
        let llm = solver_workers[0].llm.clone();
        let model_name = llm.model_name();
        emit_diagnostic(
            &app_handle,
            "info",
            "info",
            "engine",
            None,
            &format!(
                "Solver pool: {} worker(s), primary: {} (budget: {}, temp: {})",
                solver_workers.len(),
                model_name,
                solver_budget,
                model_cfg.temperature
            ),
            serde_json::json!({"model": &model_name, "budget": solver_budget, "workers": solver_workers.len()}),
            &self.attempt_id,
        );

        // Build separate reviewer LLM client (for audit, review, critic, obligation checks)
        // Falls back to solver model if no reviewer_model configured
        let reviewer_llm = if let Some(ref rev_cfg) = self.config.reviewer_model {
            let rev_key = resolve_api_key(&rev_cfg.api_key_ref)?;
            let rev_budget = (rev_cfg.max_budget_tokens as u32).clamp(2_000, 16_000);
            tracing::info!(
                "Reviewer: {}/{} (max_output_tokens: {}, temperature: {})",
                rev_cfg.provider,
                rev_cfg.model,
                rev_budget,
                rev_cfg.temperature
            );
            LlmClient::with_budget(&rev_cfg.provider, &rev_cfg.model, &rev_key, rev_budget)
                .with_temperature(rev_cfg.temperature)
        } else {
            tracing::info!("No reviewer model configured — using solver model for reviews");
            LlmClient::with_budget(
                &model_cfg.provider,
                &model_cfg.model,
                &api_key,
                solver_budget,
            )
            .with_temperature(model_cfg.temperature)
        };

        // Build separate adversary LLM client for adversarial node challenges.
        // Falls back to reviewer, then solver if not configured.
        // Adversarial challenges only fire when adversary is non-familial to solver.
        let adversary_llm = if let Some(ref adv_cfg) = self.config.adversary_model {
            let adv_key = resolve_api_key(&adv_cfg.api_key_ref)?;
            let adv_budget = (adv_cfg.max_budget_tokens as u32).clamp(2_000, 16_000);
            tracing::info!(
                "Adversary: {}/{} (max_output_tokens: {}, temperature: {})",
                adv_cfg.provider,
                adv_cfg.model,
                adv_budget,
                adv_cfg.temperature
            );
            LlmClient::with_budget(&adv_cfg.provider, &adv_cfg.model, &adv_key, adv_budget)
                .with_temperature(adv_cfg.temperature)
        } else if let Some(ref rev_cfg) = self.config.reviewer_model {
            tracing::info!("No adversary model configured — using reviewer model for challenges");
            let rev_key = resolve_api_key(&rev_cfg.api_key_ref)?;
            let rev_budget = (rev_cfg.max_budget_tokens as u32).clamp(2_000, 16_000);
            LlmClient::with_budget(&rev_cfg.provider, &rev_cfg.model, &rev_key, rev_budget)
                .with_temperature(rev_cfg.temperature)
        } else {
            tracing::info!("No adversary model configured — using solver model (challenges will be skipped as same-family)");
            LlmClient::with_budget(
                &model_cfg.provider,
                &model_cfg.model,
                &api_key,
                solver_budget,
            )
            .with_temperature(model_cfg.temperature)
        };
        let adversary_model_name = adversary_llm.model_name();

        // Build separate critic LLM client (for obligation counterexample checks).
        // Falls back to reviewer, then solver if not configured.
        let critic_llm = if let Some(ref crt_cfg) = self.config.critic_model {
            let crt_key = resolve_api_key(&crt_cfg.api_key_ref)?;
            let crt_budget = (crt_cfg.max_budget_tokens as u32).clamp(2_000, 16_000);
            tracing::info!(
                "Critic: {}/{} (max_output_tokens: {}, temperature: {})",
                crt_cfg.provider,
                crt_cfg.model,
                crt_budget,
                crt_cfg.temperature
            );
            emit_diagnostic(
                &app_handle,
                "info",
                "info",
                "engine",
                None,
                &format!(
                    "Critic: {} (budget: {}, temp: {})",
                    crt_cfg.model, crt_budget, crt_cfg.temperature
                ),
                serde_json::json!({"model": &crt_cfg.model}),
                &self.attempt_id,
            );
            LlmClient::with_budget(&crt_cfg.provider, &crt_cfg.model, &crt_key, crt_budget)
                .with_temperature(crt_cfg.temperature)
        } else if let Some(ref rev_cfg) = self.config.reviewer_model {
            tracing::info!("No critic model configured — using reviewer model for critic checks");
            let rev_key = resolve_api_key(&rev_cfg.api_key_ref)?;
            let rev_budget = (rev_cfg.max_budget_tokens as u32).clamp(2_000, 16_000);
            LlmClient::with_budget(&rev_cfg.provider, &rev_cfg.model, &rev_key, rev_budget)
                .with_temperature(rev_cfg.temperature)
        } else {
            tracing::info!("No critic model configured — using solver model for critic checks");
            LlmClient::with_budget(
                &model_cfg.provider,
                &model_cfg.model,
                &api_key,
                solver_budget,
            )
            .with_temperature(model_cfg.temperature)
        };

        // Build optional Discerner LLM client — for failure classification after failed attempts.
        // Discerner is skipped entirely when discerner_model is not configured.
        let discerner_llm: Option<LlmClient> =
            if let Some(ref dis_cfg) = self.config.discerner_model {
                let dis_key = resolve_api_key(&dis_cfg.api_key_ref)?;
                // Cap at 4k — Discerner only needs a short JSON output
                let dis_budget = (dis_cfg.max_budget_tokens as u32).clamp(512, 4_000);
                tracing::info!(
                    "Discerner: {}/{} (max_output_tokens: {}, temperature: {})",
                    dis_cfg.provider,
                    dis_cfg.model,
                    dis_budget,
                    dis_cfg.temperature
                );
                emit_diagnostic(
                    &app_handle,
                    "info",
                    "info",
                    "engine",
                    None,
                    &format!(
                        "Discerner: {} (budget: {}, temp: {})",
                        dis_cfg.model, dis_budget, dis_cfg.temperature
                    ),
                    serde_json::json!({"model": &dis_cfg.model}),
                    &self.attempt_id,
                );
                Some(
                    LlmClient::with_budget(&dis_cfg.provider, &dis_cfg.model, &dis_key, dis_budget)
                        .with_temperature(dis_cfg.temperature),
                )
            } else {
                None
            };

        // Build optional Decomposer LLM client — for strategic problem decomposition.
        // Falls back to reviewer, then solver if not configured.
        let decomposer_llm: Option<LlmClient> =
            if let Some(ref dec_cfg) = self.config.decomposer_model {
                let dec_key = resolve_api_key(&dec_cfg.api_key_ref)?;
                let dec_budget = (dec_cfg.max_budget_tokens as u32).clamp(1_000, 8_000);
                tracing::info!(
                    "Decomposer: {}/{} (budget: {}, temp: {})",
                    dec_cfg.provider,
                    dec_cfg.model,
                    dec_budget,
                    dec_cfg.temperature
                );
                emit_diagnostic(
                    &app_handle,
                    "info",
                    "info",
                    "engine",
                    None,
                    &format!(
                        "Decomposer: {} (budget: {}, temp: {})",
                        dec_cfg.model, dec_budget, dec_cfg.temperature
                    ),
                    serde_json::json!({"model": &dec_cfg.model}),
                    &self.attempt_id,
                );
                Some(
                    LlmClient::with_budget(&dec_cfg.provider, &dec_cfg.model, &dec_key, dec_budget)
                        .with_temperature(dec_cfg.temperature),
                )
            } else if let Some(ref rev_cfg) = self.config.reviewer_model {
                // Fall back to reviewer model for decomposition
                let rev_key = resolve_api_key(&rev_cfg.api_key_ref).ok();
                if let Some(key) = rev_key {
                    let rev_budget = (rev_cfg.max_budget_tokens as u32).clamp(1_000, 8_000);
                    tracing::info!("Decomposer: using reviewer model {}", rev_cfg.model);
                    Some(
                        LlmClient::with_budget(&rev_cfg.provider, &rev_cfg.model, &key, rev_budget)
                            .with_temperature(rev_cfg.temperature),
                    )
                } else {
                    None
                }
            } else {
                // Fall back to solver model
                tracing::info!("Decomposer: using solver model");
                Some(
                    LlmClient::with_budget(
                        &model_cfg.provider,
                        &model_cfg.model,
                        &api_key,
                        solver_budget,
                    )
                    .with_temperature(model_cfg.temperature),
                )
            };

        let pipeline = VerificationPipeline::new();
        let orchestrator = crate::loop_engine::orchestrator::Orchestrator::new();

        // Quick sidecar reachability check — never block on Lean warmup.
        // Lean warms up passively in the sidecar; validation uses it when ready.
        {
            let sidecar = crate::api::sidecar::SidecarClient::new();
            match sidecar.health_extended().await {
                Ok(h) => {
                    if h.lean_ready {
                        let _ = app_handle.emit(
                            "loop:sidecar_warmup",
                            LoopSidecarWarmupPayload {
                                status: SidecarWarmupStatus::Ready,
                            },
                        );
                        tracing::info!("Sidecar ready (Lean warm)");
                        emit_diagnostic(
                            &app_handle,
                            "info",
                            "info",
                            "sidecar",
                            None,
                            "Sidecar ready (Lean warm)",
                            serde_json::json!({"lean": true}),
                            &self.attempt_id,
                        );
                    } else if h.lean_warming_up {
                        let _ = app_handle.emit(
                            "loop:sidecar_warmup",
                            LoopSidecarWarmupPayload {
                                status: SidecarWarmupStatus::Warming,
                            },
                        );
                        tracing::info!("Sidecar reachable, Lean still warming — proceeding (Lean used when ready)");
                        emit_diagnostic(
                            &app_handle,
                            "info",
                            "warn",
                            "sidecar",
                            None,
                            "Lean still warming — proceeding without",
                            serde_json::json!({"lean_warming": true}),
                            &self.attempt_id,
                        );
                    } else {
                        let _ = app_handle.emit(
                            "loop:sidecar_warmup",
                            LoopSidecarWarmupPayload {
                                status: SidecarWarmupStatus::Ready,
                            },
                        );
                        tracing::info!("Sidecar ready (Lean not available — SymPy only)");
                        emit_diagnostic(
                            &app_handle,
                            "info",
                            "info",
                            "sidecar",
                            None,
                            "Sidecar ready (Lean not available — SymPy only)",
                            serde_json::json!({"lean": false}),
                            &self.attempt_id,
                        );
                    }
                }
                Err(_) => {
                    // Sidecar not reachable — wait up to 10s for it to come up
                    let _ = app_handle.emit(
                        "loop:sidecar_warmup",
                        LoopSidecarWarmupPayload {
                            status: SidecarWarmupStatus::Waiting,
                        },
                    );
                    tracing::warn!("Sidecar not reachable, waiting up to 10s...");
                    let start = std::time::Instant::now();
                    let mut reachable = false;
                    while start.elapsed() < std::time::Duration::from_secs(10) {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        if sidecar.health_check().await.unwrap_or(false) {
                            reachable = true;
                            break;
                        }
                    }
                    if reachable {
                        let _ = app_handle.emit(
                            "loop:sidecar_warmup",
                            LoopSidecarWarmupPayload {
                                status: SidecarWarmupStatus::Ready,
                            },
                        );
                        tracing::info!("Sidecar came up after {}s", start.elapsed().as_secs());
                        emit_diagnostic(
                            &app_handle,
                            "info",
                            "info",
                            "sidecar",
                            None,
                            &format!("Sidecar came up after {}s", start.elapsed().as_secs()),
                            serde_json::json!({}),
                            &self.attempt_id,
                        );
                    } else {
                        let _ = app_handle.emit(
                            "loop:sidecar_warmup",
                            LoopSidecarWarmupPayload {
                                status: SidecarWarmupStatus::Timeout,
                            },
                        );
                        tracing::error!("Sidecar unreachable after 10s — validation will fail");
                        emit_diagnostic(
                            &app_handle,
                            "mechanical",
                            "error",
                            "sidecar",
                            None,
                            "Sidecar unreachable after 10s — validation will fail",
                            serde_json::json!({"waited_secs": 10}),
                            &self.attempt_id,
                        );
                    }
                }
            }
        }

        let problem = self
            .state
            .db
            .get_problem(&self.problem_id)
            .map_err(|e| e.to_string())?;
        let step_number: u32 = self.starting_step;
        let failures: Vec<(String, String)> = Vec::new();
        let max_steps = self.starting_step + 100;

        let failure_buffer = discerner::FailureBuffer::new(10);

        // Load technique registry for this problem's domain
        let techniques = self
            .state
            .db
            .get_techniques_for_class(&problem.domain)
            .unwrap_or_default();
        if !techniques.is_empty() {
            tracing::info!(
                "Loaded {} techniques for domain '{}'",
                techniques.len(),
                problem.domain
            );
        }

        // Outcome tracking
        let proof_complete = false;
        let stopped_by_user = false;

        // Exploration audit state
        let verified_since_audit: u32 = 0;
        let last_audit: Option<audit::AuditResult> = None;
        let verified_count: u32 = 0;
        let consecutive_failures: u32 = 0;

        // Obligation queue: drives solver step selection by priority.
        // When obligations exist, the queue picks the highest-priority unblocked one
        // and assigns the solver to it. Falls back to freeform prompt when empty.
        let pivot_tracker = obligation_queue::PivotTracker::new();
        let selected_obligation: Option<obligation_queue::SelectedObligation> = None;

        // Real-time claim extraction monitor — runs regex on every streamed token
        let claim_monitor =
            std::sync::Arc::new(std::sync::Mutex::new(claim_extractor::StreamMonitor::new()));

        // Load prior council findings for this problem (from previous attempts)
        let prior_findings = self
            .state
            .db
            .get_findings_for_problem(&self.problem_id)
            .unwrap_or_default();
        if !prior_findings.is_empty() {
            tracing::info!(
                "Injecting {} prior findings into solver prompt",
                prior_findings.len()
            );
        }

        // Track which pattern IDs were injected during this attempt (for success/failure feedback)
        let all_injected_pattern_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // === Branch initialization ===
        // Create the main branch (branch 0) for this attempt.
        let main_branch_id = self
            .state
            .db
            .create_branch(&self.attempt_id, 0, None, Some("initial"), Some("main"))
            .unwrap_or(0);
        let current_branch_id: i32 = main_branch_id as i32;
        tracing::info!(
            "Created main branch {} for attempt {}",
            current_branch_id,
            self.attempt_id
        );

        // Record attempt start as dag_event
        let _ = self.state.db.append_dag_event(
            &self.attempt_id, "attempt_started",
            &serde_json::json!({"problem_id": &self.problem_id, "model": &model_name, "branch_id": current_branch_id}).to_string(),
            "loop_engine",
        );

        // Emit start event
        let _ = app_handle.emit(
            "loop:started",
            LoopEvent {
                event: "started".into(),
                attempt_id: self.attempt_id.clone(),
                data: LoopStartedData {
                    problem: problem.statement.clone(),
                },
            },
        );

        // Resolve scout sources early — needed for both reconnaissance and scout briefing.
        // Prefer scout_sources vec, fall back to legacy use_scout flag.
        let effective_scout_sources: Vec<String> = if !self.config.scout_sources.is_empty() {
            self.config.scout_sources.clone()
        } else if self.config.use_scout {
            vec!["arxiv".to_string(), "semantic_scholar".to_string()]
        } else {
            vec![]
        };

        // === Pre-Decomposition Reconnaissance ===
        // Search for THIS problem's known answer BEFORE the decomposer creates
        // an obligation graph. If found, the decomposer builds obligations
        // around the suspected answer. If not found, the graph is flagged PROVISIONAL.
        // The suspected answer can be DISPROVED during the run — it's a hypothesis, not a fact.
        let mut suspected_answer: Option<step::SuspectedAnswer> = problem
            .known_answer
            .as_ref()
            .map(|a| step::SuspectedAnswer {
                value: a.clone(),
                source: "db".to_string(),
                confidence: 1.0,
                disproved: false,
                disproval_reason: None,
            });
        let mut recon_briefing = String::new();

        tracing::info!(
            "Reconnaissance: scout_sources={:?}, known_answer={:?}",
            effective_scout_sources,
            problem.known_answer
        );
        if !effective_scout_sources.is_empty() {
            tracing::info!("Reconnaissance: calling sidecar...");
            let sidecar_recon = crate::api::sidecar::SidecarClient::new();
            match sidecar_recon
                .reconnaissance(
                    &problem.statement,
                    &problem.source,
                    problem.title.as_deref(),
                    Some(&problem.domain),
                )
                .await
            {
                Ok(resp) => {
                    tracing::info!("Reconnaissance result: known_answer={:?}, confidence={:.2}, candidates={:?}",
                        resp.known_answer, resp.confidence, resp.candidate_answers);
                    recon_briefing = resp.briefing.clone();
                    // Only set suspected answer from reconnaissance if DB doesn't have one
                    if suspected_answer.is_none() {
                        if let Some(ref answer) = resp.known_answer {
                            if resp.confidence >= 0.5 {
                                suspected_answer = Some(step::SuspectedAnswer {
                                    value: answer.clone(),
                                    source: "reconnaissance".to_string(),
                                    confidence: resp.confidence,
                                    disproved: false,
                                    disproval_reason: None,
                                });
                                tracing::info!("Reconnaissance: suspected answer '{}' (confidence: {:.2}, source: external)",
                                    answer, resp.confidence);
                            }
                        }
                    }
                    if !resp.candidate_answers.is_empty() {
                        tracing::info!(
                            "Reconnaissance: candidate answers: {:?}",
                            resp.candidate_answers
                        );
                    }
                    emit_diagnostic(
                        &app_handle,
                        "info",
                        "info",
                        "reconnaissance",
                        None,
                        &format!(
                            "Reconnaissance: {} candidates, confidence {:.2}",
                            resp.candidate_answers.len(),
                            resp.confidence
                        ),
                        serde_json::json!({
                            "suspected_answer": resp.known_answer,
                            "candidates": &resp.candidate_answers,
                            "confidence": resp.confidence,
                            "strategies": &resp.proof_strategies,
                        }),
                        &self.attempt_id,
                    );
                    let _ = app_handle.emit(
                        "agent:reconnaissance_result",
                        serde_json::json!({
                            "suspected_answer": resp.known_answer,
                            "candidates": &resp.candidate_answers,
                            "confidence": resp.confidence,
                            "briefing": &resp.briefing,
                        }),
                    );
                }
                Err(e) => {
                    // Fail-open: reconnaissance error never blocks the proof run
                    tracing::warn!("Reconnaissance failed: {} — proceeding without", e);
                    emit_diagnostic(
                        &app_handle,
                        "mechanical",
                        "warn",
                        "reconnaissance",
                        None,
                        &format!("Reconnaissance failed: {}", e),
                        serde_json::json!({"error": e.to_string()}),
                        &self.attempt_id,
                    );
                }
            }
        }

        // === Strategic Decomposition ===
        // Run the decomposer to create a typed obligation graph.
        // Reconnaissance results are injected into the decomposer prompt so it
        // builds obligations around the correct target answer (when known).
        let mut current_decomp_id: Option<String> = None;
        if let Some(ref dec_llm) = decomposer_llm {
            // Pass suspected answer to decomposer (only if not disproved and confidence >= 0.7)
            let decomposer_answer = suspected_answer
                .as_ref()
                .filter(|sa| !sa.disproved && sa.confidence >= 0.7)
                .map(|sa| sa.value.as_str());
            tracing::info!(
                "Decomposer: suspected_answer={:?}, recon_briefing_len={}",
                decomposer_answer,
                recon_briefing.len()
            );
            match decomposer::run_initial_decomposition(
                &self.state.db,
                dec_llm,
                &self.attempt_id,
                current_branch_id,
                &problem.statement,
                &problem.domain,
                decomposer_answer,
                &recon_briefing,
                &app_handle,
            )
            .await
            {
                Ok((decomp_id, result)) => {
                    emit_diagnostic(
                        &app_handle,
                        "info",
                        "info",
                        "decomposer",
                        None,
                        &format!(
                            "Decomposed into {} obligations (type: {})",
                            result.obligations.len(),
                            result.problem_profile.problem_type
                        ),
                        serde_json::json!({
                            "decomposition_id": &decomp_id,
                            "obligations": result.obligations.len(),
                            "problem_type": &result.problem_profile.problem_type,
                        }),
                        &self.attempt_id,
                    );
                    let _ = app_handle.emit(
                        "agent:orchestrator_decision",
                        AgentOrchestratorEventPayload {
                            type_: "decomposition".to_string(),
                            obligations_created: Some(result.obligations.len() as u32),
                            problem_type: Some(result.problem_profile.problem_type.clone()),
                            obligation_id: None,
                            worker_count: None,
                            worker_models: None,
                            results_processed: None,
                            extra: std::collections::BTreeMap::new(),
                        },
                    );
                    current_decomp_id = Some(decomp_id);
                }
                Err(e) => {
                    // Fail-open: decomposer error never blocks the proof run
                    tracing::warn!(
                        "Decomposer failed: {} — falling back to freeform solving",
                        e
                    );
                    emit_diagnostic(
                        &app_handle,
                        "mechanical",
                        "warn",
                        "decomposer",
                        None,
                        &format!("Decomposer failed: {} — falling back to freeform", e),
                        serde_json::json!({"error": &e}),
                        &self.attempt_id,
                    );
                }
            }
        }

        // === Pre-Solve Research Briefing ===
        // Run the scout agent to gather relevant research context for this problem.
        // Queries arXiv, Semantic Scholar, OEIS, Loogle etc. based on problem domain.
        // The briefing text is injected into every solver prompt for the rest of this attempt.
        let research_briefing: String = if !effective_scout_sources.is_empty() {
            let sidecar_scout = crate::api::sidecar::SidecarClient::new();
            match sidecar_scout
                .scout_briefing(
                    &problem.statement,
                    Some(&problem.domain),
                    &effective_scout_sources,
                )
                .await
            {
                Ok(resp) => {
                    let briefing = resp.briefing.clone();
                    if !briefing.is_empty() {
                        tracing::info!(
                            "Scout briefing: {} results from {:?}",
                            resp.results_count,
                            resp.sources_queried
                        );
                        emit_diagnostic(
                            &app_handle,
                            "info",
                            "info",
                            "scout",
                            None,
                            &format!(
                                "Research briefing: {} results from {} sources",
                                resp.results_count,
                                resp.sources_queried.len()
                            ),
                            serde_json::json!({
                                "results_count": resp.results_count,
                                "sources": resp.sources_queried,
                                "briefing_len": briefing.len(),
                            }),
                            &self.attempt_id,
                        );
                        let _ = app_handle.emit(
                            "agent:scout_result",
                            AgentScoutResultPayload {
                                trigger: ScoutTrigger::PreSolve,
                                results_count: resp.results_count,
                                sources: resp.sources_queried.clone(),
                                briefing: briefing.clone(),
                                obligation_id: None,
                                obligation_desc: None,
                                blacklisted_techniques: None,
                            },
                        );
                    } else {
                        tracing::info!("Scout briefing: no relevant results found");
                    }
                    briefing
                }
                Err(e) => {
                    // Fail-open: scout error never blocks the proof run
                    tracing::warn!(
                        "Scout briefing failed: {} — proceeding without research context",
                        e
                    );
                    emit_diagnostic(
                        &app_handle,
                        "mechanical",
                        "warn",
                        "scout",
                        None,
                        &format!("Scout failed: {} — no research context", e),
                        serde_json::json!({"error": e.to_string()}),
                        &self.attempt_id,
                    );
                    String::new()
                }
            }
        } else {
            String::new()
        };

        // === Context Enrichment ===
        // Build rich context from DAG history, prior attempts, patterns, and research.
        // This context is injected into all agent prompts for the rest of this attempt.
        let proof_context = context_enricher::ProofContext::build(
            &self.state.db,
            &self.problem_id,
            &problem.domain,
            &self.attempt_id,
            research_briefing,
        );
        // The enriched context replaces the raw research_briefing for solver prompts
        let enriched_solver_context = proof_context.format_for_solver();
        let enriched_analyst_context = proof_context.format_for_analyst();
        if !enriched_solver_context.is_empty() {
            tracing::info!(
                "Context enricher: {} chars of solver context, {} chars of analyst context",
                enriched_solver_context.len(),
                enriched_analyst_context.len()
            );
        }

        // === Build step config and state ===
        let step_cfg = step::StepConfig {
            state: self.state.clone(),
            attempt_id: self.attempt_id.clone(),
            problem_id: self.problem_id.clone(),
            problem: problem.clone(),
            attempt_constraints: self.attempt_constraints.clone(),
            max_steps,
            use_patterns: self.config.use_patterns,
            failure_threshold: self.config.failure_threshold,
            enriched_solver_context,
            enriched_analyst_context,
            techniques,
            prior_findings,
            pipeline,
            llm: llm.clone(),
            reviewer_llm: reviewer_llm.clone(),
            adversary_llm: adversary_llm.clone(),
            critic_llm: critic_llm.clone(),
            discerner_llm: discerner_llm.clone(),
            decomposer_llm: decomposer_llm.clone(),
            model_name: model_name.clone(),
            adversary_model_name: adversary_model_name.clone(),
            solver_workers,
            same_obligation_fanin_enabled: self.config.same_obligation_fanin_enabled,
            max_fanin_workers: self.config.max_fanin_workers,
            scout_sources: effective_scout_sources.clone(),
        };

        let mut step_state = step::StepState {
            step_number,
            failures,
            failure_buffer,
            proof_complete,
            stopped_by_user,
            verified_since_audit,
            last_audit,
            verified_count,
            consecutive_failures,
            pivot_tracker,
            selected_obligation,
            claim_monitor,
            all_injected_pattern_ids,
            current_branch_id,
            current_decomp_id,
            orchestrator,
            main_branch_id: main_branch_id as i32,
            obligation_scouted: std::collections::HashSet::new(),
            obligation_scout_bl_at: std::collections::HashMap::new(),
            obligation_scout_results: std::collections::HashMap::new(),
            suspected_answer,
            sticky_obligations: Vec::new(),
            fanin_focus_obligation_id: None,
            pending_proposals: Vec::new(),
            steps_since_all_closed: 0,
        };

        while step_state.step_number < step_cfg.max_steps {
            // Check if still running
            let running = self.state.loop_running.lock().await;
            if !*running {
                tracing::info!("Loop paused/stopped at step {}", step_state.step_number);
                step_state.stopped_by_user = true;
                break;
            }
            drop(running);

            match step::run_step(&step_cfg, &mut step_state, &app_handle).await? {
                step::StepOutcome::Continue => continue,
                step::StepOutcome::ProofComplete => break,
                step::StepOutcome::Break(_reason) => break,
            }
        }

        // === Rebind locals from step_state for post-loop code ===
        let step_number = step_state.step_number;
        let failures = step_state.failures;
        let failure_buffer = step_state.failure_buffer;
        let proof_complete = step_state.proof_complete;
        let stopped_by_user = step_state.stopped_by_user;
        let verified_count = step_state.verified_count;
        let all_injected_pattern_ids = step_state.all_injected_pattern_ids;

        // === Post-Attempt Review ===
        // Run a review of the full proof trace to identify gaps in exploration.
        // Findings get recorded to council_sessions + council_findings for injection
        // into future attempts on this problem.
        let steps_processed = step_number - self.starting_step;
        let mut review_result: Option<review::ReviewResult> = None;

        if steps_processed > 0 {
            tracing::info!("Running post-attempt review...");
            let _ = self.state.db.append_dag_event(
                &self.attempt_id,
                "review_started",
                &serde_json::json!({"steps_processed": steps_processed}).to_string(),
                "reviewer",
            );
            let _ = app_handle.emit(
                "loop:review_start",
                LoopReviewStartPayload {
                    attempt_id: Some(self.attempt_id.clone()),
                    manual: None,
                    problem_id: None,
                },
            );

            let all_steps = self
                .state
                .db
                .get_problem_steps(&self.problem_id)
                .unwrap_or_default();
            let review_prompt = review::build_review_prompt(
                &problem.statement,
                &all_steps,
                problem.known_answer.as_deref(),
                &step_cfg.enriched_analyst_context,
            );

            let _ = app_handle.emit(
                "loop:thinking_start",
                LoopThinkingStartPayload {
                    step_number: Some(step_number),
                    model: model_name.clone(),
                    agent_role: Some("reviewer".to_string()),
                    obligation_id: None,
                    review: Some(true),
                    manual: None,
                },
            );
            let review_handle = app_handle.clone();
            let review_resp_result = reviewer_llm
                .complete_streaming(&review_prompt, move |chunk| {
                    let _ = review_handle.emit(
                        "loop:token",
                        LoopTokenPayload {
                            text: chunk.to_string(),
                            agent_role: Some("reviewer".to_string()),
                            obligation_id: None,
                        },
                    );
                })
                .await;
            let _ = app_handle.emit(
                "loop:thinking_end",
                LoopThinkingEndPayload {
                    obligation_id: None,
                },
            );

            if let Ok(review_resp) = review_resp_result {
                if let Some(parsed) = review::parse_review(&review_resp.text) {
                    tracing::info!(
                        findings = parsed.findings.len(),
                        coverage = parsed.exploration_coverage,
                        conclusion_sound = parsed.conclusion_sound,
                        label = %parsed.training_label,
                        "Post-attempt review complete"
                    );
                    let _ = self.state.db.append_dag_event(
                        &self.attempt_id,
                        "review_completed",
                        &serde_json::json!({
                            "findings": parsed.findings.len(),
                            "coverage": parsed.exploration_coverage,
                            "label": &parsed.training_label,
                            "conclusion_sound": parsed.conclusion_sound,
                        })
                        .to_string(),
                        "reviewer",
                    );

                    // Record council session + findings
                    if let Ok(session_id) = self.state.db.record_council_session(
                        "post_attempt_review",
                        &self.problem_id,
                        Some(&self.attempt_id),
                        &model_name,
                        &review_resp.text,
                        parsed.findings.len() as u32,
                    ) {
                        for finding in &parsed.findings {
                            let _ = self.state.db.record_council_finding(
                                &session_id,
                                &finding.finding_type,
                                &finding.summary,
                                &finding.detail,
                                finding.target_agent.as_deref(),
                            );
                        }
                    }

                    // Emit review event to frontend
                    let _ = app_handle.emit(
                        "loop:review_complete",
                        ContractReviewResult {
                            attempt_id: self.attempt_id.clone(),
                            findings_count: parsed.findings.len() as u32,
                            exploration_coverage: parsed.exploration_coverage,
                            conclusion_sound: parsed.conclusion_sound,
                            conclusion_confidence: parsed.conclusion_confidence,
                            training_label: parsed.training_label.clone(),
                            missing_constructions: parsed.missing_constructions.clone(),
                            findings: parsed
                                .findings
                                .iter()
                                .map(|f| ReviewFinding {
                                    type_: f.finding_type.clone(),
                                    summary: f.summary.clone(),
                                    detail: f.detail.clone(),
                                    priority: f.priority.clone(),
                                })
                                .collect(),
                            manual: None,
                            problem_id: None,
                        },
                    );

                    review_result = Some(parsed);
                } else {
                    tracing::warn!("Failed to parse review response");
                }
            }

            let _ = app_handle.emit("loop:review_end", EmptyPayload {});
        }

        // === Pattern Extraction (M2) ===
        // After a successful proof, extract reusable technique patterns for future problems.
        if proof_complete && steps_processed >= 3 {
            tracing::info!("Extracting patterns from verified proof...");
            let all_steps = self
                .state
                .db
                .get_problem_steps(&self.problem_id)
                .unwrap_or_default();
            let verified_steps: Vec<_> = all_steps.iter().filter(|s| s.verified).cloned().collect();

            if verified_steps.len() >= 3 {
                let extraction_prompt = patterns::build_extraction_prompt(
                    &problem.statement,
                    &problem.domain,
                    &verified_steps,
                );

                let extraction_messages =
                    [serde_json::json!({"role": "user", "content": &extraction_prompt})];
                let extraction_turn = llm
                    .complete_with_tools(&extraction_messages, &[], |_| {})
                    .await;
                let extraction_text = extraction_turn
                    .ok()
                    .and_then(|t| match t {
                        crate::api::llm_client::LlmTurn::Text(r) => Some(r.text),
                        _ => None,
                    })
                    .unwrap_or_default();
                if let Some(extracted) = patterns::parse_extraction(&extraction_text) {
                    tracing::info!("Extracted {} patterns from proof", extracted.len());
                    let mut pattern_names = Vec::new();

                    for pat in &extracted {
                        let source_json = serde_json::to_string(&pat.source_step_numbers)
                            .unwrap_or_else(|_| "[]".to_string());

                        // Deduplicate: check if similar trigger exists for this domain
                        let existing = self
                            .state
                            .db
                            .find_pattern_by_trigger(&problem.domain, &pat.trigger_text);

                        if let Ok(Some(existing_pat)) = existing {
                            tracing::info!(
                                "Pattern '{}' already exists — incrementing success",
                                existing_pat.name
                            );
                            let _ = self.state.db.increment_pattern_success(&existing_pat.id);
                            pattern_names.push(existing_pat.name);
                        } else {
                            let tc = if pat.technique_class.is_empty() {
                                None
                            } else {
                                Some(pat.technique_class.as_str())
                            };
                            match self.state.db.insert_pattern(
                                &pat.name,
                                &pat.description,
                                Some(&problem.domain),
                                &pat.trigger_text,
                                &pat.strategy,
                                &source_json,
                                tc,
                            ) {
                                Ok(id) => {
                                    tracing::info!(
                                        "Stored new pattern '{}' (id: {})",
                                        pat.name,
                                        id
                                    );
                                    pattern_names.push(pat.name.clone());
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to store pattern '{}': {}", pat.name, e)
                                }
                            }
                        }
                    }

                    // Record pattern extraction in dag_events
                    let _ = self.state.db.append_dag_event(
                        &self.attempt_id,
                        "patterns_extracted",
                        &serde_json::json!({
                            "count": extracted.len(),
                            "names": &pattern_names,
                        })
                        .to_string(),
                        "loop_engine",
                    );

                    // Emit event for frontend
                    let _ = app_handle.emit(
                        "loop:patterns_extracted",
                        LoopPatternsExtractedPayload {
                            attempt_id: self.attempt_id.clone(),
                            count: extracted.len() as u32,
                            patterns: extracted
                                .iter()
                                .map(|p| ExtractedPattern {
                                    name: p.name.clone(),
                                    description: p.description.clone(),
                                    trigger_text: p.trigger_text.clone(),
                                    strategy: p.strategy.clone(),
                                    technique_class: p.technique_class.clone(),
                                })
                                .collect(),
                        },
                    );
                } else {
                    tracing::warn!("Failed to parse pattern extraction response");
                }
            }
        }

        // === Pattern Success/Failure Feedback ===
        // Update injected pattern stats based on attempt outcome.
        if !all_injected_pattern_ids.is_empty() {
            if proof_complete {
                tracing::info!(
                    "Proof complete — incrementing success for {} injected patterns",
                    all_injected_pattern_ids.len()
                );
                for pid in &all_injected_pattern_ids {
                    let _ = self.state.db.increment_pattern_success(pid);
                    let _ = self.state.db.update_pattern_last_used(pid);
                }
            } else if !stopped_by_user && steps_processed > 0 {
                tracing::info!(
                    "Attempt exhausted — incrementing failure for {} injected patterns",
                    all_injected_pattern_ids.len()
                );
                for pid in &all_injected_pattern_ids {
                    let _ = self.state.db.increment_pattern_failure(pid);
                }
            }
        }

        // === RED-015: Technique Registry Feedback ===
        // Update technique_registry success/failure counts based on attempt outcome.
        if !step_cfg.techniques.is_empty() && steps_processed > 0 {
            let success = proof_complete;
            tracing::info!(
                "Recording technique use ({}) for {} techniques",
                if success { "success" } else { "failure" },
                step_cfg.techniques.len()
            );
            for t in &step_cfg.techniques {
                let _ = self.state.db.record_technique_use(t.id, success);
            }
        }

        tracing::info!("Loop engine finished. {} steps processed.", steps_processed);
        emit_diagnostic(
            &app_handle,
            "info",
            if proof_complete { "info" } else { "warn" },
            "engine",
            None,
            &format!(
                "Attempt finished: {} steps, proof_complete={}, stopped={}",
                steps_processed, proof_complete, stopped_by_user
            ),
            serde_json::json!({"steps": steps_processed, "proof_complete": proof_complete, "stopped": stopped_by_user}),
            &self.attempt_id,
        );

        // Record attempt completion as dag_event
        let _ = self.state.db.append_dag_event(
            &self.attempt_id,
            "attempt_finished",
            &serde_json::json!({
                "steps_processed": steps_processed,
                "proof_complete": proof_complete,
                "stopped_by_user": stopped_by_user,
            })
            .to_string(),
            "loop_engine",
        );

        // === After-Action Report persistence ===
        // Aggregate stats and persist a structured AAR to the after_action_reports table.
        if steps_processed > 0 {
            let open_obs_count = self
                .state
                .db
                .get_open_obligations(&self.attempt_id)
                .map(|v| v.len())
                .unwrap_or(0) as i32;
            let all_obs_count: i32 = {
                let conn = self.state.db.conn();
                conn.query_row(
                    "SELECT COUNT(*) FROM obligations WHERE attempt_id = ?1",
                    rusqlite::params![self.attempt_id],
                    |r| r.get(0),
                )
                .unwrap_or(0)
            };
            let closed_obs = all_obs_count - open_obs_count;
            let contradictions_count: i32 = self
                .state
                .db
                .get_conflicts_for_attempt(&self.attempt_id)
                .map(|v| v.len())
                .unwrap_or(0) as i32;

            let coverage = review_result.as_ref().map(|r| r.exploration_coverage);
            let soundness = review_result.as_ref().map(|r| r.conclusion_confidence);
            let training_label = review_result.as_ref().map(|r| r.training_label.as_str());
            let findings_json = review_result
                .as_ref()
                .map(|r| serde_json::to_string(&r.findings).unwrap_or_else(|_| "[]".to_string()));
            let recommendations = review_result
                .as_ref()
                .map(|r| r.missing_constructions.join("; "));
            let efficiency = if steps_processed > 0 {
                Some(verified_count as f64 / steps_processed as f64)
            } else {
                None
            };
            let death_spirals = if failure_buffer.streak() >= step::DISCERNER_TRIGGER_STREAK {
                1
            } else {
                0
            };

            let _ = self.state.db.create_after_action_report(
                &self.attempt_id,
                &self.problem_id,
                coverage,
                soundness,
                efficiency,
                death_spirals,
                contradictions_count,
                all_obs_count,
                closed_obs,
                findings_json.as_deref(),
                recommendations.as_deref(),
                training_label,
            );
            tracing::info!(
                "After-action report persisted for attempt {}",
                self.attempt_id
            );
        }

        // === DISCERNER ===
        // If a Discerner is configured and the attempt failed (not proof_complete, not stopped by user),
        // classify the failure as mechanical vs. logical to inform the retry strategy.
        let discerner_verdict: Option<discerner::FailureClassification> =
            if !proof_complete && !stopped_by_user && steps_processed > 0 {
                if let Some(ref dis_llm) = discerner_llm {
                    let step_summary = format!(
                        "Problem: {}. {} steps attempted, {} failures recorded.",
                        problem.statement,
                        steps_processed,
                        failures.len()
                    );
                    match discerner::classify_failure(
                        &failures,
                        &step_summary,
                        dis_llm,
                        &app_handle,
                        &self.attempt_id,
                    )
                    .await
                    {
                        Ok(verdict) => {
                            emit_diagnostic(
                                &app_handle,
                                "gate",
                                "info",
                                "discerner",
                                None,
                                &format!(
                                "Failure classified as '{}' (confidence={:.0}%): {} → retry: {}",
                                verdict.kind,
                                verdict.confidence * 100.0,
                                verdict.reasoning,
                                verdict.retry_rec
                            ),
                                serde_json::json!({
                                    "kind": &verdict.kind,
                                    "confidence": verdict.confidence,
                                    "mechanical_score": verdict.mechanical_score,
                                    "logical_score": verdict.logical_score,
                                    "retry_rec": &verdict.retry_rec,
                                }),
                                &self.attempt_id,
                            );
                            // Persist the decision
                            let decision = crate::db::discerner::DiscernerDecision {
                                id: uuid::Uuid::new_v4().to_string(),
                                attempt_id: self.attempt_id.clone(),
                                kind: verdict.kind.clone(),
                                confidence: verdict.confidence,
                                mechanical_score: verdict.mechanical_score,
                                logical_score: verdict.logical_score,
                                reasoning: verdict.reasoning.clone(),
                                retry_rec: verdict.retry_rec.clone(),
                                model: dis_llm.model_name(),
                                failures_json: serde_json::to_string(&failures).unwrap_or_default(),
                                created_at: chrono::Utc::now()
                                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                            };
                            let _ = self.state.db.save_discerner_decision(&decision);
                            Some(verdict)
                        }
                        Err(e) => {
                            tracing::warn!("Discerner failed: {}", e);
                            emit_diagnostic(
                                &app_handle,
                                "mechanical",
                                "warn",
                                "discerner",
                                None,
                                &format!("Discerner error: {}", e),
                                serde_json::json!({"error": e}),
                                &self.attempt_id,
                            );
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            };

        Ok(AttemptOutcome {
            attempt_id: self.attempt_id.clone(),
            steps_processed,
            review: review_result,
            stopped_by_user,
            proof_complete,
            discerner_verdict,
        })
    }

    /// Run an iterative solve loop. Creates new attempts until the review
    /// indicates sufficient quality, or limits are reached.
    pub async fn run_outer_loop(
        state: Arc<AppState>,
        config: MultiAgentConfig,
        problem_id: String,
        initial_attempt_id: String,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        let max_attempts = config.max_attempts.max(1);
        let min_coverage = config.min_exploration_coverage;
        let min_confidence = config.min_conclusion_confidence;

        let mut attempt_id = initial_attempt_id;
        let mut attempt_number: u32 = 1;
        // Accumulated constraints from prior failed attempts — force diversity
        let mut attempt_constraints: Vec<String> = Vec::new();

        loop {
            *state.current_attempt_id.lock().await = Some(attempt_id.clone());

            let _ = app_handle.emit(
                "loop:attempt_start",
                LoopAttemptStartPayload {
                    attempt_id: attempt_id.clone(),
                    attempt_number,
                    max_attempts,
                    constraints: Some(attempt_constraints.clone()),
                },
            );

            let engine = LoopEngine::new(
                state.clone(),
                config.clone(),
                problem_id.clone(),
                attempt_id.clone(),
                0,
            )
            .with_constraints(attempt_constraints.clone());
            let outcome = engine.run(app_handle.clone()).await?;

            // Update attempt status in DB
            let attempt_status = if outcome.proof_complete {
                "completed"
            } else if outcome.stopped_by_user {
                "stopped"
            } else {
                "reviewed"
            };
            let _ = state.db.update_attempt_status(&attempt_id, attempt_status);

            // === Decision: should we re-attempt? ===

            if outcome.stopped_by_user {
                tracing::info!("User stopped — no re-attempt");
                break;
            }

            // Proof complete AND review says correct → done
            if outcome.proof_complete {
                if let Some(ref rev) = outcome.review {
                    if rev.training_label == "correct" {
                        tracing::info!("Proof complete and correct — no re-attempt");
                        break;
                    }
                    tracing::info!(
                        "Proof concluded but label='{}', coverage={:.2}, confidence={:.2}",
                        rev.training_label,
                        rev.exploration_coverage,
                        rev.conclusion_confidence
                    );
                } else {
                    tracing::info!("Proof complete, no review — accepting");
                    break;
                }
            }

            // Check thresholds from review
            let should_retry = if let Some(ref rev) = outcome.review {
                let low_coverage = rev.exploration_coverage < min_coverage;
                let low_confidence = rev.conclusion_confidence < min_confidence;
                let wrong = rev.training_label == "wrong_answer";
                let incomplete = rev.training_label == "locally_sound_globally_incomplete";
                low_coverage || low_confidence || wrong || incomplete
            } else {
                false
            };

            if !should_retry {
                tracing::info!("Review thresholds met — no re-attempt needed");
                break;
            }

            if attempt_number >= max_attempts {
                tracing::info!("Reached max attempts ({}) — stopping", max_attempts);
                let _ = state.db.update_attempt_status(&attempt_id, "exhausted");
                break;
            }

            // Check if still running (user might stop between attempts)
            if !*state.loop_running.lock().await {
                tracing::info!("Loop stopped between attempts — no re-attempt");
                break;
            }

            // === Discerner-informed retry strategy ===
            // If the Discerner classified the failure as mechanical (infrastructure),
            // clear accumulated constraints — the approach itself wasn't at fault.
            if let Some(ref verdict) = outcome.discerner_verdict {
                match verdict.retry_rec.as_str() {
                    "same" if verdict.confidence >= 0.7 => {
                        tracing::info!(
                            "Discerner: mechanical failure (confidence={:.0}%) — clearing constraints, retrying same approach",
                            verdict.confidence * 100.0
                        );
                        attempt_constraints.clear();
                    }
                    "restructure" => {
                        tracing::info!(
                            "Discerner: logical failure (confidence={:.0}%) — restructure recommended",
                            verdict.confidence * 100.0
                        );
                        attempt_constraints.push(format!(
                            "DISCERNER: {} Restructure your approach entirely.",
                            verdict.reasoning
                        ));
                    }
                    _ => {} // "new_approach" or low-confidence "same" — fall through to normal constraint building
                }
            }

            // === Build attempt diversity constraints from the failed attempt ===
            if let Some(ref rev) = outcome.review {
                if rev.training_label == "wrong_answer"
                    || rev.training_label == "locally_sound_globally_incomplete"
                {
                    // Extract the conclusion from the failed attempt
                    let all_steps = state.db.get_problem_steps(&problem_id).unwrap_or_default();
                    let conclusion = all_steps
                        .iter()
                        .rev()
                        .find(|s| s.verified && s.proposal_type == "conclusion")
                        .map(|s| s.proposal_natural.as_str());
                    if let Some(c) = conclusion {
                        attempt_constraints.push(format!(
                            "Previous attempt concluded '{}' which was labeled '{}'. Do NOT reach this same conclusion.",
                            c, rev.training_label
                        ));
                    }
                }
                for mc in &rev.missing_constructions {
                    attempt_constraints.push(format!("MUST explore: {}", mc));
                }
                for f in &rev.findings {
                    if f.priority == "high" && f.finding_type == "technique_gap" {
                        attempt_constraints.push(format!("REQUIRED: {}", f.summary));
                    }
                }
            }
            // Also note any unresolved obligations from the previous attempt
            let unresolved = state
                .db
                .get_open_obligations(&attempt_id)
                .unwrap_or_default();
            for ob in &unresolved {
                attempt_constraints
                    .push(format!("UNRESOLVED from prior attempt: {}", ob.description));
            }

            tracing::info!(
                "Attempt constraints for next attempt: {:?}",
                attempt_constraints
            );

            // === Start new attempt ===
            attempt_number += 1;
            let models: Vec<String> = config
                .models
                .iter()
                .map(|m| format!("{}/{}", m.provider, m.model))
                .collect();
            attempt_id = state
                .db
                .create_attempt(&problem_id, &models)
                .map_err(|e| e.to_string())?;

            tracing::info!(
                "Starting re-attempt {}/{} for problem {} (attempt_id: {})",
                attempt_number,
                max_attempts,
                problem_id,
                attempt_id
            );

            let _ = app_handle.emit(
                "loop:retry",
                LoopRetryPayload {
                    attempt_number,
                    max_attempts,
                    previous_attempt_id: Some(outcome.attempt_id.clone()),
                    new_attempt_id: attempt_id.clone(),
                    reason: Some(format!(
                        "coverage={:.2}, confidence={:.2}, label={}",
                        outcome
                            .review
                            .as_ref()
                            .map(|r| r.exploration_coverage)
                            .unwrap_or(0.0),
                        outcome
                            .review
                            .as_ref()
                            .map(|r| r.conclusion_confidence)
                            .unwrap_or(0.0),
                        outcome
                            .review
                            .as_ref()
                            .map(|r| r.training_label.as_str())
                            .unwrap_or("unknown"),
                    )),
                },
            );
        }

        // Clean up
        *state.loop_running.lock().await = false;
        *state.current_attempt_id.lock().await = None;

        let _ = app_handle.emit(
            "loop:outer_complete",
            LoopOuterCompletePayload {
                attempts_used: attempt_number,
                final_attempt_id: attempt_id.clone(),
            },
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::step::{
        extract_corrected_formal, obligation_needs_llm_review, parse_proposal,
        parse_resolved_obligations, parse_reviewer_verdicts, pick_selected_obligations,
        resolve_step_cursor, tally_has_closing_majority,
    };
    use crate::loop_engine::obligation_queue::SelectedObligation;
    use crate::models::dag::{Obligation, ProofNode};

    // === parse_proposal tests ===

    #[test]
    fn parse_proposal_raw_json() {
        let input = r#"{"natural": "We have x = 2", "formal": "x = 2"}"#;
        let p = parse_proposal(input).expect("should parse raw JSON");
        assert_eq!(p.natural, "We have x = 2");
        assert_eq!(p.formal.as_deref(), Some("x = 2"));
        assert!(p.proposal_type.is_none());
    }

    #[test]
    fn parse_proposal_fenced_json() {
        let input = "Here is my step:\n```json\n{\"natural\": \"Step 1\", \"formal\": \"a = b\"}\n```\nDone.";
        let p = parse_proposal(input).expect("should parse fenced JSON");
        assert_eq!(p.natural, "Step 1");
        assert_eq!(p.formal.as_deref(), Some("a = b"));
    }

    #[test]
    fn parse_proposal_prose_wrapped() {
        let input = "Let me think about this...\n\n{\"natural\": \"By induction\", \"formal\": \"P(n) => P(n+1)\", \"reasoning\": \"base case holds\"}\n\nI hope that's right.";
        let p = parse_proposal(input).expect("should parse prose-wrapped JSON");
        assert_eq!(p.natural, "By induction");
        assert_eq!(p.reasoning.as_deref(), Some("base case holds"));
    }

    #[test]
    fn parse_proposal_with_all_fields() {
        let input = r#"{
            "proposal_type": "conclusion",
            "natural": "Therefore the answer is 42",
            "formal": "answer = 42",
            "formal_lean": "theorem answer : answer = 42 := rfl",
            "reasoning": "From steps 1-5",
            "targets_obligation": "BOUND",
            "closes_obligation": true,
            "closure_reason": "Established the upper bound"
        }"#;
        let p = parse_proposal(input).expect("should parse all fields");
        assert_eq!(p.proposal_type.as_deref(), Some("conclusion"));
        assert_eq!(p.natural, "Therefore the answer is 42");
        assert_eq!(p.formal.as_deref(), Some("answer = 42"));
        assert_eq!(
            p.formal_lean.as_deref(),
            Some("theorem answer : answer = 42 := rfl")
        );
        assert_eq!(p.reasoning.as_deref(), Some("From steps 1-5"));
        assert_eq!(p.targets_obligation.as_deref(), Some("BOUND"));
        assert_eq!(p.closes_obligation, Some(true));
        assert_eq!(
            p.closure_reason.as_deref(),
            Some("Established the upper bound")
        );
    }

    #[test]
    fn parse_proposal_nested_braces_in_strings() {
        // formal contains braces inside the string value
        let input = r#"{"natural": "Set S = {1, 2, 3}", "formal": "S = \\{1, 2, 3\\}"}"#;
        let p = parse_proposal(input).expect("should handle braces in strings");
        assert_eq!(p.natural, "Set S = {1, 2, 3}");
    }

    #[test]
    fn parse_proposal_unparseable() {
        assert!(parse_proposal("This is just prose with no JSON at all").is_none());
        assert!(parse_proposal("").is_none());
        assert!(parse_proposal("{ malformed json }").is_none());
    }

    #[test]
    fn parse_proposal_missing_natural_fails() {
        // natural is required (not Option)
        let input = r#"{"formal": "x = 2"}"#;
        assert!(parse_proposal(input).is_none());
    }

    // === parse_resolved_obligations tests ===

    #[test]
    fn parse_resolved_new_format() {
        let input = r#"[{"note": "Steps 3-7 prove it", "id": "abc-123"}, {"note": "Follows from step 2", "id": "def-456"}]"#;
        let result = parse_resolved_obligations(input);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "abc-123");
        assert_eq!(result[0].1, "Steps 3-7 prove it");
        assert_eq!(result[1].0, "def-456");
    }

    #[test]
    fn parse_resolved_legacy_string_array() {
        let input = r#"["id-1", "id-2", "id-3"]"#;
        let result = parse_resolved_obligations(input);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, "id-1");
        assert!(result[0].1.is_empty()); // legacy has no note
    }

    #[test]
    fn parse_resolved_empty_array() {
        let result = parse_resolved_obligations("[]");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_resolved_prose_wrapped() {
        let input = "Based on my analysis:\n[{\"id\": \"xyz\", \"note\": \"done\"}]\nThat's all.";
        let result = parse_resolved_obligations(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "xyz");
    }

    #[test]
    fn parse_resolved_no_array() {
        assert!(parse_resolved_obligations("No obligations resolved.").is_empty());
        assert!(parse_resolved_obligations("").is_empty());
    }

    #[test]
    fn parse_resolved_note_before_id() {
        // The prompt asks for note before id — make sure order doesn't matter
        let input = r#"[{"note": "reason first", "id": "abc"}]"#;
        let result = parse_resolved_obligations(input);
        assert_eq!(result[0].0, "abc");
        assert_eq!(result[0].1, "reason first");
    }

    #[test]
    fn tally_requires_three_votes_before_closure() {
        assert!(
            !tally_has_closing_majority(1, 1),
            "mechanical 1/1 should not close an obligation"
        );
        assert!(
            !tally_has_closing_majority(2, 2),
            "2/2 is still below the 3-vote floor"
        );
        assert!(
            tally_has_closing_majority(2, 3),
            "2/3 should satisfy the closure majority rule"
        );
    }

    #[test]
    fn reserved_parallel_step_number_is_recorded_without_off_by_one_shift() {
        let (recorded_step, next_cursor) = resolve_step_cursor(24, 24);
        assert_eq!(recorded_step, 24);
        assert_eq!(next_cursor, 25);

        let (recorded_step, next_cursor) = resolve_step_cursor(30, 28);
        assert_eq!(recorded_step, 28);
        assert_eq!(next_cursor, 30);
    }

    // === parse_reviewer_verdicts tests ===

    #[test]
    fn parse_reviewer_verdicts_all_obligations() {
        let input = r#"[{"note": "Steps prove n>0 case but n=0 missing", "id": "abc-123", "satisfied": false}, {"note": "All cases covered", "id": "def-456", "satisfied": true}]"#;
        let result = parse_reviewer_verdicts(input);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "abc-123");
        assert!(!result[0].1); // not satisfied
        assert_eq!(result[0].2, "Steps prove n>0 case but n=0 missing");
        assert_eq!(result[1].0, "def-456");
        assert!(result[1].1); // satisfied
    }

    #[test]
    fn parse_reviewer_verdicts_empty() {
        assert!(parse_reviewer_verdicts("[]").is_empty());
    }

    #[test]
    fn parse_reviewer_verdicts_prose_wrapped() {
        let input =
            "After analysis:\n[{\"note\": \"not done\", \"id\": \"xyz\", \"satisfied\": false}]\n";
        let result = parse_reviewer_verdicts(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "xyz");
        assert!(!result[0].1);
        assert_eq!(result[0].2, "not done");
    }

    #[test]
    fn reviewer_skips_obligations_without_any_attached_evidence() {
        let should_review = obligation_needs_llm_review("ob-empty", None, false, &[]);
        assert!(
            !should_review,
            "reviewer/adversary should skip obligations with no targeted proof nodes"
        );
    }

    #[test]
    fn reviewer_keeps_targeted_or_mechanically_satisfied_obligations() {
        let node = ProofNode {
            id: "node-1".into(),
            attempt_id: "attempt-1".into(),
            branch_id: 0,
            node_type: "step".into(),
            parent_ids: None,
            content: "Verified progress".into(),
            formal_content: Some("x = x".into()),
            technique_class: None,
            construction_family: None,
            status: "verified".into(),
            validator_used: None,
            validator_result: None,
            model_id: None,
            obligation_ref: Some("ob-has-proof".into()),
            opens_obligations: None,
            step_id: Some("step-1".into()),
            token_cost: None,
            sequence_number: 1,
            created_at: "2026-03-06T00:00:00Z".into(),
            verified_at: Some("2026-03-06T00:00:00Z".into()),
        };

        assert!(obligation_needs_llm_review(
            "ob-has-proof",
            None,
            false,
            std::slice::from_ref(&node),
        ));
        assert!(obligation_needs_llm_review(
            "ob-targeted",
            Some("ob-targeted"),
            false,
            &[],
        ));
        assert!(obligation_needs_llm_review(
            "ob-mechanical",
            None,
            true,
            &[],
        ));
    }

    #[test]
    fn fanin_focus_prevents_new_obligations_from_stealing_workers() {
        let mk_selected = |id: &str, desc: &str| SelectedObligation {
            obligation: Obligation {
                id: id.into(),
                attempt_id: "attempt-1".into(),
                branch_id: 0,
                parent_node_id: "parent".into(),
                description: desc.into(),
                obligation_type: "CASE_CHECK".into(),
                priority: 0.9,
                confidence: 0.9,
                source_layer: None,
                status: "open".into(),
                assigned_model: None,
                closure_node_id: None,
                closure_type: None,
                escalation_level: 0,
                steps_spent: 0,
                max_steps: 20,
                search_space: None,
                superseded_by: None,
                retraction_reason: None,
                depends_on: None,
                decomposition_id: None,
                satisfaction_criteria: None,
                signature_json: None,
                embedding_json: None,
                scout_status: None,
                last_scout_session_id: None,
                last_scout_confidence: None,
                resolved_externally: false,
                resolved_by_corpus_id: None,
                external_reference: None,
                scout_last_checked_at: None,
                assigned_models_json: None,
                active_solver_round_id: None,
                created_at: "2026-03-06T00:00:00Z".into(),
                closed_at: None,
            },
            blacklisted_approaches: vec![],
        };

        let sticky = vec![
            mk_selected("ob-case", "existing focus"),
            mk_selected("ob-classify", "new branch"),
            mk_selected("ob-bound", "another branch"),
        ];

        let selected = pick_selected_obligations(&sticky, Some("ob-case"), true, 3, 3);
        assert_eq!(selected.len(), 5);
        assert_eq!(selected[0].obligation.id, "ob-case");
        assert_eq!(selected[1].obligation.id, "ob-case");
        assert_eq!(selected[2].obligation.id, "ob-case");
        assert_eq!(selected[3].obligation.id, "ob-classify");
        assert_eq!(selected[4].obligation.id, "ob-bound");
    }

    #[test]
    fn focused_obligation_stays_first_even_without_parallel_workers() {
        let sticky = vec![
            SelectedObligation {
                obligation: Obligation {
                    id: "ob-case".into(),
                    attempt_id: "attempt-1".into(),
                    branch_id: 0,
                    parent_node_id: "parent".into(),
                    description: "existing focus".into(),
                    obligation_type: "CASE_CHECK".into(),
                    priority: 0.9,
                    confidence: 0.9,
                    source_layer: None,
                    status: "open".into(),
                    assigned_model: None,
                    closure_node_id: None,
                    closure_type: None,
                    escalation_level: 0,
                    steps_spent: 0,
                    max_steps: 20,
                    search_space: None,
                    superseded_by: None,
                    retraction_reason: None,
                    depends_on: None,
                    decomposition_id: None,
                    satisfaction_criteria: None,
                    signature_json: None,
                    embedding_json: None,
                    scout_status: None,
                    last_scout_session_id: None,
                    last_scout_confidence: None,
                    resolved_externally: false,
                    resolved_by_corpus_id: None,
                    external_reference: None,
                    scout_last_checked_at: None,
                    assigned_models_json: None,
                    active_solver_round_id: None,
                    created_at: "2026-03-06T00:00:00Z".into(),
                    closed_at: None,
                },
                blacklisted_approaches: vec![],
            },
            SelectedObligation {
                obligation: Obligation {
                    id: "ob-next".into(),
                    attempt_id: "attempt-1".into(),
                    branch_id: 0,
                    parent_node_id: "parent".into(),
                    description: "next branch".into(),
                    obligation_type: "BOUND".into(),
                    priority: 0.8,
                    confidence: 0.8,
                    source_layer: None,
                    status: "open".into(),
                    assigned_model: None,
                    closure_node_id: None,
                    closure_type: None,
                    escalation_level: 0,
                    steps_spent: 0,
                    max_steps: 20,
                    search_space: None,
                    superseded_by: None,
                    retraction_reason: None,
                    depends_on: None,
                    decomposition_id: None,
                    satisfaction_criteria: None,
                    signature_json: None,
                    embedding_json: None,
                    scout_status: None,
                    last_scout_session_id: None,
                    last_scout_confidence: None,
                    resolved_externally: false,
                    resolved_by_corpus_id: None,
                    external_reference: None,
                    scout_last_checked_at: None,
                    assigned_models_json: None,
                    active_solver_round_id: None,
                    created_at: "2026-03-06T00:00:00Z".into(),
                    closed_at: None,
                },
                blacklisted_approaches: vec![],
            },
        ];

        let selected = pick_selected_obligations(&sticky, Some("ob-case"), true, 1, 3);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].obligation.id, "ob-case");
        assert_eq!(selected[1].obligation.id, "ob-next");
    }

    // === extract_corrected_formal tests ===

    #[test]
    fn extract_corrected_formal_simple() {
        let input = r#"{"formal": "x^2 + 2*x + 1 = (x+1)^2"}"#;
        let result = extract_corrected_formal(input);
        assert_eq!(result.as_deref(), Some("x^2 + 2*x + 1 = (x+1)^2"));
    }

    #[test]
    fn extract_corrected_formal_full_proposal() {
        // Model returns full proposal instead of just {formal: ...}
        let input = r#"{"natural": "corrected", "formal": "a = b + c", "reasoning": "fixed sign"}"#;
        let result = extract_corrected_formal(input);
        assert_eq!(result.as_deref(), Some("a = b + c"));
    }

    #[test]
    fn extract_corrected_formal_no_formal_field() {
        let input = r#"{"natural": "something else"}"#;
        assert!(extract_corrected_formal(input).is_none());
    }

    #[test]
    fn extract_corrected_formal_not_json() {
        assert!(extract_corrected_formal("just plain text").is_none());
    }

    #[test]
    fn extract_corrected_formal_fenced() {
        let input = "Here's the correction:\n```json\n{\"formal\": \"fixed = expr\"}\n```";
        let result = extract_corrected_formal(input);
        assert_eq!(result.as_deref(), Some("fixed = expr"));
    }
}
