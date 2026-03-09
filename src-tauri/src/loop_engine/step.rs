//! Single-iteration step execution — extracted from the main loop body.
//!
//! `run_step` owns everything that happens inside one `while step_number < max_steps`
//! iteration. The outer loop in `LoopEngine::run()` owns setup, the stop-check,
//! and post-loop review/persistence.

use crate::api::llm_client::LlmClient;
use crate::contracts::loop_events::{
    AgentCouncilFindingPayload, AgentCriticEvaluationPayload, AgentScoutResultPayload,
    AuditResult as LoopAuditResult, ClaimEventRecord, CriticCheckEvent, DiscernerClassification,
    DiscernerFinding as LoopDiscernerFinding, DiscernerSuggestedAction, LoopAnswerMismatchPayload,
    LoopBranchClosedPayload, LoopBranchCreatedPayload, LoopBranchSwitchedPayload,
    LoopCheckpointStartPayload, LoopClaimCheckFailedPayload, LoopClaimConflictPayload,
    LoopClaimsExtractedPayload, LoopConclusionReviewPayload, LoopEvidenceCollapsePayload,
    LoopFaninRoundCompletePayload, LoopFaninRoundStartPayload, LoopFaninRoundUpdatePayload,
    LoopFinishedPayload, LoopNodeChallengedPayload, LoopObligationAssignedPayload,
    LoopObligationBlockedPayload, LoopObligationClosedPayload, LoopObligationGatePayload,
    LoopObligationOpenedPayload, LoopPivotForcedPayload, LoopSolverSelfAssessmentPayload,
    LoopStepErrorPayload, LoopSuspectedAnswerDisprovedPayload, LoopSympyCorrectionAppliedPayload,
    LoopSympyCorrectionAttemptPayload, LoopThinkingEndPayload, LoopThinkingStartPayload,
    LoopTokenPayload, LoopToolCallPayload, ObligationStatus, SatisfactionSignalEvent,
    SatisfactionSource, ScoutTrigger,
};
use crate::verification::{StepContext, VerificationPipeline};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::Emitter;

/// A suspected answer from reconnaissance or DB.  Can be disproved during the run.
#[derive(Debug, Clone, Serialize)]
pub(super) struct SuspectedAnswer {
    /// The suspected answer value (e.g., "c = 4").
    pub value: String,
    /// Where this came from: "db", "reconnaissance", "user".
    pub source: String,
    /// Confidence 0.0-1.0 from reconnaissance (1.0 for DB/user-provided).
    pub confidence: f64,
    /// Set to true when a verified step contradicts this answer.
    pub disproved: bool,
    /// Reason for disproval, if any.
    pub disproval_reason: Option<String>,
}

use super::{
    audit, claim_extractor, critic, discerner, emit_diagnostic, evidence, json_parse,
    obligation_queue, orchestrator, research, response_guard, satisfaction, solver, truncate_str,
    StepEvent,
};

/// Helper: unwrap a DB write result, logging an error and emitting a diagnostic on failure.
/// Returns empty string on error so callers can check `id.is_empty()` before using it.
fn db_write_or_log<E: std::fmt::Display>(
    result: Result<String, E>,
    op: &str,
    app_handle: &tauri::AppHandle,
    attempt_id: &str,
) -> String {
    match result {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(error = %e, op, "DB write failed — DAG may be incomplete");
            emit_diagnostic(
                app_handle,
                "mechanical",
                "warn",
                "db",
                None,
                &format!("{op} failed: {e}"),
                serde_json::Value::Null,
                attempt_id,
            );
            String::new()
        }
    }
}

fn emit_loop_thinking_start(
    app: &tauri::AppHandle,
    step_number: Option<u32>,
    model: &str,
    agent_role: Option<&str>,
    obligation_id: Option<&str>,
    review: Option<bool>,
    manual: Option<bool>,
) {
    let _ = app.emit(
        "loop:thinking_start",
        LoopThinkingStartPayload {
            step_number,
            model: model.to_string(),
            agent_role: agent_role.map(str::to_string),
            obligation_id: obligation_id.map(str::to_string),
            review,
            manual,
        },
    );
}

fn emit_loop_token(
    app: &tauri::AppHandle,
    text: impl Into<String>,
    agent_role: Option<&str>,
    obligation_id: Option<&str>,
) {
    let _ = app.emit(
        "loop:token",
        LoopTokenPayload {
            text: text.into(),
            agent_role: agent_role.map(str::to_string),
            obligation_id: obligation_id.map(str::to_string),
        },
    );
}

fn emit_loop_thinking_end(app: &tauri::AppHandle, obligation_id: Option<&str>) {
    let _ = app.emit(
        "loop:thinking_end",
        LoopThinkingEndPayload {
            obligation_id: obligation_id.map(str::to_string),
        },
    );
}

fn emit_obligation_closed(
    app: &tauri::AppHandle,
    id: &str,
    status: ObligationStatus,
    closure_node_id: Option<&str>,
    closed_by_step: Option<u32>,
    closure_note: Option<&str>,
    tally_yes: Option<u32>,
    tally_total: Option<u32>,
) {
    let _ = app.emit(
        "loop:obligation_closed",
        LoopObligationClosedPayload {
            id: id.to_string(),
            status,
            closure_node_id: closure_node_id.map(str::to_string),
            closed_by_step,
            closure_note: closure_note.map(str::to_string),
            tally_yes,
            tally_total,
        },
    );
}

fn emit_satisfaction_signal(
    app: &tauri::AppHandle,
    obligation_id: &str,
    source: SatisfactionSource,
    satisfies: bool,
    tally_yes: u32,
    tally_total: u32,
    note: Option<&str>,
) {
    let _ = app.emit(
        "loop:satisfaction_signal",
        SatisfactionSignalEvent {
            obligation_id: obligation_id.to_string(),
            source,
            satisfies,
            tally_yes,
            tally_total,
            note: note.map(str::to_string),
        },
    );
}

fn map_discerner_classification(value: &str) -> DiscernerClassification {
    match value {
        "mechanical" => DiscernerClassification::Mechanical,
        "gate" => DiscernerClassification::Gate,
        "validator" => DiscernerClassification::Validator,
        _ => DiscernerClassification::Model,
    }
}

fn map_discerner_suggested_action(value: &str) -> DiscernerSuggestedAction {
    match value {
        "switch_model" => DiscernerSuggestedAction::SwitchModel,
        "add_backoff" => DiscernerSuggestedAction::AddBackoff,
        "retry" => DiscernerSuggestedAction::Retry,
        "rephrase_prompt" => DiscernerSuggestedAction::RephrasePrompt,
        _ => DiscernerSuggestedAction::Continue,
    }
}

fn claim_event_record(
    raw: &str,
    formal: &str,
    source: &str,
    offset: Option<usize>,
) -> ClaimEventRecord {
    ClaimEventRecord {
        raw: raw.to_string(),
        formal: formal.to_string(),
        source: source.to_string(),
        offset: offset.and_then(|value| u32::try_from(value).ok()),
    }
}

// ── Step-only constants ──────────────────────────────────────────────

pub(super) const DISCERNER_TRIGGER_STREAK: u32 = 2;
pub(super) const AUDIT_INTERVAL: u32 = 3;
pub(super) const MAX_OPEN_OBLIGATIONS: usize = 3;
pub(super) const MAX_CONSECUTIVE_FAILURES: u32 = 3;
pub(super) const LEAN_COUNCIL_INTERVAL: u32 = 5;
pub(super) const MAX_TOOL_CALLS_PER_STEP: u32 = 15;

// ── Step-only types ──────────────────────────────────────────────────

pub(super) fn tally_has_closing_majority(yes: u32, total: u32) -> bool {
    total >= 3 && yes * 2 > total
}

pub(super) fn resolve_step_cursor(current_cursor: u32, reserved_step_number: u32) -> (u32, u32) {
    (
        reserved_step_number,
        current_cursor.max(reserved_step_number.saturating_add(1)),
    )
}

pub(super) fn obligation_needs_llm_review(
    obligation_id: &str,
    targeted_obligation_id: Option<&str>,
    mechanical_satisfied: bool,
    obligation_nodes: &[crate::models::dag::ProofNode],
) -> bool {
    mechanical_satisfied
        || targeted_obligation_id == Some(obligation_id)
        || obligation_nodes
            .iter()
            .any(|node| node.status == "verified")
}

pub(super) fn pick_selected_obligations(
    sticky_obligations: &[obligation_queue::SelectedObligation],
    fanin_focus_obligation_id: Option<&str>,
    same_obligation_fanin_enabled: bool,
    solver_worker_count: usize,
    max_fanin_workers: u32,
) -> Vec<obligation_queue::SelectedObligation> {
    if let Some(focus_id) = fanin_focus_obligation_id {
        if let Some(focused) = sticky_obligations
            .iter()
            .find(|selected| selected.obligation.id == focus_id)
        {
            let extras: Vec<_> = sticky_obligations
                .iter()
                .filter(|selected| selected.obligation.id != focus_id)
                .cloned()
                .collect();

            if same_obligation_fanin_enabled && solver_worker_count >= 2 {
                if extras.is_empty() {
                    return vec![focused.clone()];
                }

                let fanin_copies = (solver_worker_count as u32).min(max_fanin_workers).max(1);
                let mut selected = Vec::with_capacity(fanin_copies as usize + extras.len());
                for _ in 0..fanin_copies {
                    selected.push(focused.clone());
                }
                selected.extend(extras);
                return selected;
            }

            let mut selected = Vec::with_capacity(1 + extras.len());
            selected.push(focused.clone());
            selected.extend(extras);
            return selected;
        }
    }

    sticky_obligations.to_vec()
}

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub(super) struct TypedClaim {
    #[serde(rename = "type")]
    pub claim_type: String,
    pub lhs: Option<String>,
    pub rhs: Option<String>,
    pub dividend: Option<String>,
    pub divisor: Option<String>,
    pub relation: Option<String>,
    pub a: Option<String>,
    pub b: Option<String>,
    pub value: Option<String>,
    pub expr: Option<String>,
    pub remainder: Option<String>,
    pub modulus: Option<String>,
    pub variable: Option<String>,
    pub domain: Option<String>,
    pub predicate: Option<String>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub(super) struct LlmProposal {
    pub proposal_type: Option<String>,
    pub natural: String,
    pub formal: Option<String>,
    pub formal_lean: Option<String>,
    pub reasoning: Option<String>,
    #[serde(default)]
    pub targets_obligation: Option<String>,
    #[serde(default)]
    pub closes_obligation: Option<bool>,
    #[serde(default)]
    pub closure_reason: Option<String>,
    #[serde(default)]
    pub claim: Option<TypedClaim>,
}

// ── Control flow ─────────────────────────────────────────────────────

#[derive(Debug)]
pub(super) enum StepOutcome {
    /// Continue to next iteration.
    Continue,
    /// Proof is complete — break out of the loop.
    ProofComplete,
    /// Break for a terminal reason.
    Break(BreakReason),
}

#[derive(Debug)]
#[allow(dead_code)] // variants used as control flow, payload reserved for future diagnostics
pub(super) enum BreakReason {
    FatalApiError(String),
    MaxConsecutiveFailures,
}

// ── Parallel solver types ───────────────────────────────────────────

/// Error from a solver LLM call (parallel-safe — no StepState references).
enum SolverError {
    /// 401/403/invalid_api_key — abort everything.
    Fatal(String),
    /// Transient error — count as failure.
    Retryable(String),
    /// Repetition loop detected mid-stream.
    StreamAborted(String),
}

/// Packaged result from one parallel solver call.
struct SolverCallResult {
    obligation: Option<obligation_queue::SelectedObligation>,
    step_number: u32,
    prompt: String,
    goal_state: String,
    context_refs_json: Option<String>,
    response: Result<crate::api::llm_client::LlmResponse, SolverError>,
    /// Worker identity for fan-in rounds (empty string for non-fan-in).
    worker_id: String,
    /// Model name of the worker that produced this result.
    worker_model_name: String,
    /// Round ID grouping parallel fan-in siblings (None for non-fan-in).
    solver_round_id: Option<String>,
    /// Dispatch mode: "freeform", "parallel_distinct", or "parallel_fanin".
    dispatch_mode: String,
    /// IDs of tool_runs created during this solver call (for step_id backfill).
    tool_run_ids: Vec<String>,
}

/// A solver worker — one LLM client paired with an identity.
#[derive(Clone)]
pub(super) struct SolverWorker {
    pub worker_id: String,
    pub model_name: String,
    pub llm: LlmClient,
}

// ── Immutable per-run config ─────────────────────────────────────────

pub(super) struct StepConfig {
    pub state: Arc<AppState>,
    pub attempt_id: String,
    pub problem_id: String,
    pub problem: crate::models::proof::Problem,
    pub attempt_constraints: Vec<String>,
    pub max_steps: u32,
    pub use_patterns: bool,
    pub failure_threshold: u32,
    pub enriched_solver_context: String,
    pub enriched_analyst_context: String,
    pub techniques: Vec<crate::models::dag::TechniqueEntry>,
    pub prior_findings: Vec<crate::models::council::CouncilFinding>,
    pub pipeline: VerificationPipeline,
    // Cloned LLM clients
    pub llm: LlmClient,
    pub reviewer_llm: LlmClient,
    pub adversary_llm: LlmClient,
    pub critic_llm: LlmClient,
    pub discerner_llm: Option<LlmClient>,
    #[allow(dead_code)] // reserved for Sprint 4 branching
    pub decomposer_llm: Option<LlmClient>,
    // Stable model name strings
    pub model_name: String,
    pub adversary_model_name: String,
    // Solver worker pool (all configured solver models)
    pub solver_workers: Vec<SolverWorker>,
    // Fan-in config
    pub same_obligation_fanin_enabled: bool,
    pub max_fanin_workers: u32,
    /// Scout sources for mid-solve obligation-level research queries.
    pub scout_sources: Vec<String>,
}

// ── Mutable loop state ───────────────────────────────────────────────

pub(super) struct StepState {
    pub step_number: u32,
    pub failures: Vec<(String, String)>,
    pub failure_buffer: discerner::FailureBuffer,
    pub proof_complete: bool,
    pub stopped_by_user: bool,
    pub verified_since_audit: u32,
    pub last_audit: Option<audit::AuditResult>,
    pub verified_count: u32,
    pub consecutive_failures: u32,
    pub pivot_tracker: obligation_queue::PivotTracker,
    pub selected_obligation: Option<obligation_queue::SelectedObligation>,
    pub claim_monitor: Arc<std::sync::Mutex<claim_extractor::StreamMonitor>>,
    pub all_injected_pattern_ids: std::collections::HashSet<String>,
    pub current_branch_id: i32,
    #[allow(dead_code)] // reserved for Sprint 4 branching
    pub current_decomp_id: Option<String>,
    pub orchestrator: orchestrator::Orchestrator,
    pub main_branch_id: i32,
    /// Obligations that have already been scouted (no re-fire).
    pub obligation_scouted: std::collections::HashSet<String>,
    /// Blacklist count at the time of last scout per obligation (for re-scout on new failures).
    pub obligation_scout_bl_at: std::collections::HashMap<String, usize>,
    /// Cached scout briefings per obligation ID.
    pub obligation_scout_results: std::collections::HashMap<String, String>,
    /// Suspected answer from reconnaissance.  Mutable — can be disproved during the run.
    pub suspected_answer: Option<SuspectedAnswer>,
    /// Sticky obligation assignments — once picked, solver stays on these until closed/demoted.
    /// Cleared only when an obligation is closed, demoted, or all are satisfied.
    pub sticky_obligations: Vec<obligation_queue::SelectedObligation>,
    /// If set, keep same-obligation fan-in locked on this obligation until it closes/demotes.
    pub fanin_focus_obligation_id: Option<String>,
    /// Pre-parsed proposals from a batch LLM response. When the solver returns
    /// a JSON array of steps, we process the first immediately and queue the
    /// rest here. Subsequent `run_step` calls dequeue from this buffer instead
    /// of making a new LLM call.
    pub pending_proposals: Vec<LlmProposal>,
}

// ── Step-only helpers ────────────────────────────────────────────────

/// Check if a verified step contradicts the suspected answer.
/// Returns Some(reason) if the step disproves the hypothesis, None otherwise.
///
/// Detection heuristics:
/// 1. The step explicitly states a different answer value (e.g., "c = 4" when suspected is "c = 3/2")
/// 2. The step's formal expression asserts an equality with a different value for the same variable
/// 3. The step contains phrases like "not equal to", "cannot be", "contradicts" with the suspected value
fn check_disproval(natural: &str, formal: Option<&str>, suspected_value: &str) -> Option<String> {
    let natural_lower = natural.to_lowercase();

    // Extract the numeric/symbolic part from suspected value (e.g., "c = 4" → "4", "3/2" → "3/2")
    let suspected_num = suspected_value
        .split('=')
        .next_back()
        .unwrap_or(suspected_value)
        .trim();

    // Look for explicit contradiction phrases
    let contradiction_phrases = [
        "not equal to",
        "cannot be",
        "is not",
        "≠",
        "!=",
        "disproves",
        "contradicts",
        "incorrect",
        "wrong",
    ];
    for phrase in &contradiction_phrases {
        if natural_lower.contains(phrase) && natural_lower.contains(suspected_num) {
            return Some(format!(
                "Verified step states '{}' regarding suspected value '{}'",
                truncate_str(natural, 120),
                suspected_value
            ));
        }
    }

    // Look for conclusion-like statements asserting a DIFFERENT value for the same variable
    // Extract the variable name from suspected value (e.g., "c = 4" → "c")
    let suspected_var = suspected_value.split('=').next().map(|s| s.trim());
    if let Some(var) = suspected_var {
        if !var.is_empty() && var.len() <= 10 {
            // Check if natural text asserts var = <something different>
            let var_lower = var.to_lowercase();
            // Pattern: "the answer is X" or "c = X" where X ≠ suspected_num
            for pattern in &[
                format!("{} = ", var_lower),
                format!("{} is ", var_lower),
                "answer is ".to_string(),
                "equals ".to_string(),
            ] {
                if let Some(pos) = natural_lower.find(pattern.as_str()) {
                    let after = &natural[pos + pattern.len()..];
                    // Extract the claimed value (first token)
                    let claimed: String = after
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '/' || *c == '.' || *c == '-')
                        .collect();
                    if !claimed.is_empty() && claimed != suspected_num && claimed.len() <= 20 {
                        return Some(format!(
                            "Verified step asserts {} = {} (suspected: {})",
                            var, claimed, suspected_num
                        ));
                    }
                }
            }
        }
    }

    // Check formal expression for direct contradiction
    if let Some(formal_str) = formal {
        if let Some(var) = suspected_var {
            if !var.is_empty() && var.len() <= 10 {
                // formal might be "c = 4" when suspected is "c = 3/2"
                let formal_lower = formal_str.to_lowercase();
                let var_lower = var.to_lowercase();
                let eq_pattern = format!("{} = ", var_lower);
                if let Some(pos) = formal_lower.find(&eq_pattern) {
                    let after = &formal_str[pos + eq_pattern.len()..];
                    let claimed: String = after
                        .chars()
                        .take_while(|c| {
                            c.is_alphanumeric() || *c == '/' || *c == '.' || *c == '-' || *c == '*'
                        })
                        .collect();
                    if !claimed.is_empty() && claimed.trim() != suspected_num && claimed.len() <= 20
                    {
                        return Some(format!(
                            "Formal expression asserts {} = {} (suspected: {})",
                            var, claimed, suspected_num
                        ));
                    }
                }
            }
        }
    }

    None
}

/// Parse LLM response into a proposal struct.
/// Handles raw JSON, JSON in markdown fences, and JSON embedded in prose text.
pub(super) fn parse_proposal(response: &str) -> Option<LlmProposal> {
    let trimmed = response.trim();

    // 1. Try centralized extraction (direct parse + markdown fence + brace carve)
    if let Some(p) = json_parse::extract_json::<LlmProposal>(trimmed) {
        tracing::debug!("parse_proposal: direct parse succeeded");
        return Some(p);
    }

    // 2. Fall back to full brace-depth matching with string escape handling.
    let chars: Vec<char> = trimmed.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            let start = i;
            let mut depth = 1;
            let mut in_string = false;
            let mut escape_next = false;
            i += 1;
            while i < chars.len() && depth > 0 {
                let c = chars[i];
                if escape_next {
                    escape_next = false;
                } else if c == '\\' && in_string {
                    escape_next = true;
                } else if c == '"' {
                    in_string = !in_string;
                } else if !in_string {
                    if c == '{' {
                        depth += 1;
                    } else if c == '}' {
                        depth -= 1;
                    }
                }
                i += 1;
            }
            if depth == 0 {
                let json_str: String = chars[start..i].iter().collect();
                if let Ok(p) = serde_json::from_str::<LlmProposal>(&json_str) {
                    tracing::debug!(
                        "parse_proposal: brace-matched extraction succeeded at char {}",
                        start
                    );
                    return Some(p);
                }
            }
        } else {
            i += 1;
        }
    }

    tracing::warn!(
        "parse_proposal: all extraction methods failed. Response length: {}, first 200 chars: {}",
        trimmed.len(),
        trimmed.chars().take(200).collect::<String>()
    );
    None
}

/// Parse one or more proposals from an LLM response.
///
/// Supports two formats:
/// 1. **JSON array** — the solver returned multiple steps in one shot:
///    `[{"natural": "step 1", "formal": "..."}, {"natural": "step 2", ...}]`
/// 2. **Single JSON object** — the traditional one-step format.
///
/// Returns `None` only if no valid proposals could be extracted.
pub(super) fn parse_proposals(response: &str) -> Option<Vec<LlmProposal>> {
    let trimmed = response.trim();

    // Try JSON array first (batch mode)
    if let Some(arr) = json_parse::extract_json::<Vec<LlmProposal>>(trimmed) {
        if !arr.is_empty() {
            tracing::info!(
                "parse_proposals: batch mode — {} proposals extracted",
                arr.len()
            );
            return Some(arr);
        }
    }

    // Fall back to single proposal
    parse_proposal(trimmed).map(|p| vec![p])
}

/// Parse batched obligation resolution response — returns list of (id, note) pairs.
/// Supports both new format [{"id": "...", "note": "..."}] and legacy ["id1", "id2"].
pub(super) fn parse_resolved_obligations(response: &str) -> Vec<(String, String)> {
    let trimmed = response.trim();

    let json_str = if trimmed.starts_with('[') {
        trimmed.to_string()
    } else if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            trimmed[start..=end].to_string()
        } else {
            return Vec::new();
        }
    } else {
        return Vec::new();
    };

    #[derive(Deserialize)]
    struct Resolution {
        id: String,
        #[serde(default)]
        note: String,
    }
    if let Ok(resolutions) = serde_json::from_str::<Vec<Resolution>>(&json_str) {
        return resolutions.into_iter().map(|r| (r.id, r.note)).collect();
    }

    if let Ok(ids) = serde_json::from_str::<Vec<String>>(&json_str) {
        return ids.into_iter().map(|id| (id, String::new())).collect();
    }

    Vec::new()
}

/// Parse reviewer verdicts from the new all-obligations format.
/// Returns Vec<(obligation_id, satisfied, note)>.
pub(super) fn parse_reviewer_verdicts(response: &str) -> Vec<(String, bool, String)> {
    let trimmed = response.trim();
    let json_str = if trimmed.starts_with('[') {
        trimmed.to_string()
    } else if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            trimmed[start..=end].to_string()
        } else {
            return Vec::new();
        }
    } else {
        return Vec::new();
    };

    #[derive(Deserialize)]
    struct ReviewerVerdict {
        id: String,
        #[serde(default)]
        satisfied: bool,
        #[serde(default)]
        note: String,
    }
    if let Ok(verdicts) = serde_json::from_str::<Vec<ReviewerVerdict>>(&json_str) {
        return verdicts
            .into_iter()
            .map(|v| (v.id, v.satisfied, v.note))
            .collect();
    }
    Vec::new()
}

/// Extract the `formal` field from a SymPy correction response.
pub(super) fn extract_corrected_formal(text: &str) -> Option<String> {
    let val: serde_json::Value = json_parse::extract_json(text)?;
    val.get("formal")?.as_str().map(|s| s.to_string())
}

// ── Parallel-safe solver call ────────────────────────────────────────

/// Execute one solver LLM call with tool-use loop.
///
/// This function is `Send + 'static` compatible: it takes only owned/Arc'd data
/// and never touches `StepState`. Error classification and state mutation are
/// deferred to `process_solver_result()`.
async fn call_solver(
    llm: LlmClient,
    prompt: String,
    step_number: u32,
    app: tauri::AppHandle,
    model_name: String,
    attempt_id: String,
    obligation_id: Option<String>,
    db: Arc<AppState>,
) -> (
    Result<crate::api::llm_client::LlmResponse, SolverError>,
    Vec<String>,
) {
    tracing::info!("Step {}: calling {}", step_number, model_name);
    let ob_tag = obligation_id.clone();
    emit_loop_thinking_start(
        &app,
        Some(step_number),
        &model_name,
        Some("solver"),
        ob_tag.as_deref(),
        None,
        None,
    );

    let research_tools = research::tool_definitions();
    let research_sidecar = crate::api::sidecar::SidecarClient::new();
    let mut tool_messages: Vec<serde_json::Value> =
        vec![serde_json::json!({"role": "user", "content": &prompt})];
    let mut tool_calls_made = 0u32;
    let mut accumulated_text = String::new();
    let mut total_tokens_in: u32 = 0;
    let mut total_tokens_out: u32 = 0;
    let provider = llm.provider().to_string();
    let mut tool_run_ids: Vec<String> = Vec::new();
    let tool_session_id = uuid::Uuid::new_v4().to_string();

    let rep_detector = std::sync::Arc::new(std::sync::Mutex::new(
        response_guard::RepetitionDetector::new(),
    ));
    let rep_abort = rep_detector
        .lock()
        .map(|d| d.abort_flag())
        .unwrap_or_else(|_| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));

    let llm_result: Result<crate::api::llm_client::LlmResponse, String> = 'tool_loop: {
        loop {
            let token_handle = app.clone();
            let rep_det_ref = std::sync::Arc::clone(&rep_detector);
            let ob_id_for_token = ob_tag.clone();
            let on_token = move |chunk: &str| {
                emit_loop_token(
                    &token_handle,
                    chunk,
                    Some("solver"),
                    ob_id_for_token.as_deref(),
                );
                if let Ok(mut det) = rep_det_ref.lock() {
                    det.feed(chunk);
                }
            };

            let turn = match llm
                .complete_with_tools_abortable(
                    &tool_messages,
                    &research_tools,
                    on_token,
                    std::sync::Arc::clone(&rep_abort),
                )
                .await
            {
                Ok(t) => t,
                Err(e) => {
                    emit_loop_thinking_end(&app, ob_tag.as_deref());

                    if e.starts_with("stream_aborted") {
                        let pattern = rep_detector
                            .lock()
                            .ok()
                            .and_then(|d| d.pattern().map(|p| p.to_string()))
                            .unwrap_or_else(|| "unknown".to_string());
                        tracing::warn!(
                            "Step {}: solver stream aborted (repetition loop, pattern={:?})",
                            step_number,
                            pattern
                        );
                        emit_diagnostic(
                            &app,
                            "model",
                            "warn",
                            "response_guard",
                            Some(step_number),
                            &format!(
                                "Repetition loop detected — stream aborted (pattern: {:?})",
                                pattern
                            ),
                            serde_json::json!({"pattern": pattern, "model": &model_name, "detail": &e}),
                            &attempt_id,
                        );
                        let _ = app.emit(
                            "loop:step_error",
                            LoopStepErrorPayload {
                                step_number,
                                error_type: "repetition_loop".to_string(),
                                model: model_name.clone(),
                                pattern: Some(pattern),
                                attempt_id: Some(attempt_id.clone()),
                            },
                        );
                        break 'tool_loop Err(e);
                    }

                    tracing::error!("LLM error: {}", e);
                    emit_diagnostic(
                        &app,
                        "mechanical",
                        "error",
                        "llm_client",
                        Some(step_number),
                        &format!("LLM error: {}", e),
                        serde_json::json!({"error": &e, "model": &model_name}),
                        &attempt_id,
                    );

                    let is_fatal = e.contains("401")
                        || e.contains("403")
                        || e.contains("Unauthorized")
                        || e.contains("Forbidden")
                        || e.contains("invalid_api_key")
                        || e.contains("Unsupported provider");
                    if is_fatal {
                        tracing::error!("Fatal API error — aborting: {}", e);
                        emit_diagnostic(
                            &app,
                            "mechanical",
                            "fatal",
                            "llm_client",
                            Some(step_number),
                            &format!("Fatal API error: {}", e),
                            serde_json::json!({"error": &e, "model": &model_name}),
                            &attempt_id,
                        );
                        let _ = app.emit(
                            "loop:finished",
                            LoopFinishedPayload {
                                reason: Some(format!("Fatal API error: {}", e)),
                                attempt_id: Some(attempt_id.clone()),
                            },
                        );
                    }
                    break 'tool_loop Err(e);
                }
            };

            use crate::api::llm_client::LlmTurn;
            match turn {
                LlmTurn::Text(mut response) => {
                    // Prepend accumulated reasoning from prior tool-use turns
                    if !accumulated_text.is_empty() {
                        accumulated_text.push_str(&response.text);
                        response.text = accumulated_text;
                    }
                    // Add accumulated token usage from tool turns
                    response.tokens_in = Some(response.tokens_in.unwrap_or(0) + total_tokens_in);
                    response.tokens_out = Some(response.tokens_out.unwrap_or(0) + total_tokens_out);
                    break 'tool_loop Ok(response);
                }
                LlmTurn::ToolUse {
                    calls,
                    partial_text,
                    tokens_in,
                    tokens_out,
                } => {
                    // Accumulate token usage from this tool turn
                    total_tokens_in += tokens_in.unwrap_or(0);
                    total_tokens_out += tokens_out.unwrap_or(0);

                    // Preserve reasoning text from this turn
                    if !partial_text.is_empty() {
                        accumulated_text.push_str(&partial_text);
                        accumulated_text.push('\n');
                    }

                    if calls.is_empty() {
                        tracing::warn!("Empty tool calls returned, forcing text continuation");
                        continue;
                    }

                    // Build provider-correct assistant message with tool calls + any partial text
                    if provider == "anthropic" {
                        // Anthropic: content array with optional text block + tool_use blocks
                        let mut assistant_content: Vec<serde_json::Value> = Vec::new();
                        if !partial_text.is_empty() {
                            assistant_content.push(serde_json::json!({
                                "type": "text",
                                "text": &partial_text,
                            }));
                        }
                        for call in &calls {
                            assistant_content.push(serde_json::json!({
                                "type": "tool_use",
                                "id": &call.id,
                                "name": &call.name,
                                "input": &call.input,
                            }));
                        }
                        tool_messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": assistant_content,
                        }));
                    } else {
                        // OpenAI / OpenRouter / Gemini: tool_calls array on assistant message
                        let tool_calls_json: Vec<serde_json::Value> = calls.iter().map(|call| {
                            serde_json::json!({
                                "id": &call.id,
                                "type": "function",
                                "function": {
                                    "name": &call.name,
                                    "arguments": serde_json::to_string(&call.input).unwrap_or_default(),
                                }
                            })
                        }).collect();
                        let mut assistant_msg = serde_json::json!({
                            "role": "assistant",
                            "tool_calls": tool_calls_json,
                        });
                        if !partial_text.is_empty() {
                            assistant_msg["content"] = serde_json::json!(&partial_text);
                        }
                        tool_messages.push(assistant_msg);
                    }

                    // Execute each tool call and collect results
                    let mut tool_result_strings: Vec<(String, String)> = Vec::new(); // (call_id, result)
                    for call in &calls {
                        let input_str = serde_json::to_string(&call.input).unwrap_or_default();
                        let input_preview: String = input_str.chars().take(200).collect();
                        tracing::info!("Tool call: {}({})", call.name, input_preview);

                        // Emit tool call into token stream so it's visible in the UI
                        emit_loop_token(
                            &app,
                            format!("\n🔧 {}({})\n", call.name, input_preview),
                            Some("tool_call"),
                            ob_tag.as_deref(),
                        );

                        // Create tool_run record before execution
                        let run_id = db
                            .db
                            .create_tool_run(
                                &attempt_id,
                                None, // branch_id
                                None, // step_id (backfilled after step is recorded)
                                Some(step_number),
                                obligation_id.as_deref(),
                                None, // parent_tool_run_id
                                Some(&tool_session_id),
                                "solver",
                                &call.name, // trigger_kind
                                &call.name,
                                "sidecar",
                                None, // tier
                                &input_str,
                                None, // input_hash
                                false,
                            )
                            .ok();

                        let start = std::time::Instant::now();
                        let result = research::execute_tool_call(&research_sidecar, call).await;
                        let latency_ms = start.elapsed().as_millis() as u32;
                        tool_calls_made += 1;

                        // Complete tool_run with result and latency
                        if let Some(ref rid) = run_id {
                            let is_error =
                                result.contains("unavailable") || result.contains("Error:");
                            let status = if is_error { "failed" } else { "completed" };
                            let summary: String = result.chars().take(500).collect();
                            let _ = db.db.complete_tool_run(
                                rid,
                                status,
                                None, // hit
                                Some(latency_ms),
                                if is_error { Some(&result) } else { None },
                                Some(&result),
                                Some(&summary),
                            );
                            tool_run_ids.push(rid.clone());
                        }

                        // Emit tool result summary into token stream
                        let result_preview: String = result.chars().take(300).collect();
                        emit_loop_token(
                            &app,
                            format!("→ {}\n", result_preview),
                            Some("tool_result"),
                            ob_tag.as_deref(),
                        );

                        let _ = app.emit(
                            "loop:tool_call",
                            LoopToolCallPayload {
                                step_number,
                                tool: call.name.clone(),
                                input: call.input.clone(),
                                result: result_preview.clone(),
                                result_len: result.len() as u32,
                                tool_calls_made,
                                latency_ms,
                            },
                        );

                        tool_result_strings.push((call.id.clone(), result));
                    }

                    // Append tool results in provider-correct format
                    if provider == "anthropic" {
                        let results_content: Vec<serde_json::Value> = tool_result_strings
                            .iter()
                            .map(|(id, result)| {
                                serde_json::json!({
                                    "type": "tool_result",
                                    "tool_use_id": id,
                                    "content": result,
                                })
                            })
                            .collect();
                        tool_messages.push(serde_json::json!({
                            "role": "user",
                            "content": results_content,
                        }));
                    } else {
                        // OpenAI: one message per tool result with role "tool"
                        for (id, result) in &tool_result_strings {
                            tool_messages.push(serde_json::json!({
                                "role": "tool",
                                "tool_call_id": id,
                                "content": result,
                            }));
                        }
                    }

                    tracing::info!("Tool calls executed: {} total", tool_calls_made);

                    // Tool call cap — force final answer when limit reached
                    if tool_calls_made >= MAX_TOOL_CALLS_PER_STEP {
                        tracing::warn!(
                            "Step {}: tool call cap reached ({}), forcing text response",
                            step_number,
                            MAX_TOOL_CALLS_PER_STEP
                        );
                        emit_diagnostic(
                            &app,
                            "mechanical",
                            "warn",
                            "tool_loop",
                            Some(step_number),
                            &format!(
                                "Tool call cap reached ({}) — forcing final answer",
                                MAX_TOOL_CALLS_PER_STEP
                            ),
                            serde_json::json!({"tool_calls_made": tool_calls_made}),
                            &attempt_id,
                        );

                        // Nudge the model to produce its final JSON
                        tool_messages.push(serde_json::json!({
                            "role": "user",
                            "content": "Tool call limit reached. You MUST now produce your final JSON response with proposal_type, natural, and formal fields. No more tool calls are available.",
                        }));

                        // One more call with empty tools to force text-only response
                        let cap_handle = app.clone();
                        let cap_ob = ob_tag.clone();
                        let cap_on_token = move |chunk: &str| {
                            emit_loop_token(&cap_handle, chunk, Some("solver"), cap_ob.as_deref());
                        };
                        let empty_tools: Vec<crate::api::llm_client::ToolDef> = vec![];
                        match llm
                            .complete_with_tools_abortable(
                                &tool_messages,
                                &empty_tools,
                                cap_on_token,
                                std::sync::Arc::clone(&rep_abort),
                            )
                            .await
                        {
                            Ok(LlmTurn::Text(mut response)) => {
                                if !accumulated_text.is_empty() {
                                    accumulated_text.push_str(&response.text);
                                    response.text = accumulated_text;
                                }
                                response.tokens_in =
                                    Some(response.tokens_in.unwrap_or(0) + total_tokens_in);
                                response.tokens_out =
                                    Some(response.tokens_out.unwrap_or(0) + total_tokens_out);
                                break 'tool_loop Ok(response);
                            }
                            Ok(LlmTurn::ToolUse {
                                partial_text: cap_text,
                                tokens_in: ti,
                                tokens_out: to,
                                ..
                            }) => {
                                // Model tried to use tools again despite empty list — use whatever text it gave
                                accumulated_text.push_str(&cap_text);
                                total_tokens_in += ti.unwrap_or(0);
                                total_tokens_out += to.unwrap_or(0);
                                break 'tool_loop Ok(crate::api::llm_client::LlmResponse {
                                    text: accumulated_text,
                                    tokens_in: Some(total_tokens_in),
                                    tokens_out: Some(total_tokens_out),
                                });
                            }
                            Err(e) => break 'tool_loop Err(e),
                        }
                    }
                }
            }
        }
    };

    emit_loop_thinking_end(&app, ob_tag.as_deref());

    // Classify the error
    let classified = match llm_result {
        Ok(resp) => Ok(resp),
        Err(e) => {
            if e.starts_with("stream_aborted") {
                Err(SolverError::StreamAborted(e))
            } else {
                let is_fatal = e.contains("401")
                    || e.contains("403")
                    || e.contains("Unauthorized")
                    || e.contains("Forbidden")
                    || e.contains("invalid_api_key")
                    || e.contains("Unsupported provider");
                if is_fatal {
                    Err(SolverError::Fatal(e))
                } else {
                    Err(SolverError::Retryable(e))
                }
            }
        }
    };
    (classified, tool_run_ids)
}

// ── Process one solver result (serial) ──────────────────────────────

/// Process one solver result through the full pipeline: parse, validate,
/// record, adversary challenge, satisfaction tally, audit, orchestrator.
///
/// Called once per obligation result, serially after parallel LLM calls complete.
#[allow(clippy::too_many_arguments)]
async fn process_solver_result(
    config: &StepConfig,
    state: &mut StepState,
    app_handle: &tauri::AppHandle,
    result: SolverCallResult,
    verified: &[(String, u32, String, String, String)],
    open_obligations: &[crate::models::dag::Obligation],
    all_obligations: &[crate::models::dag::Obligation],
    _patterns: &[(String, String, String)],
) -> Result<StepOutcome, String> {
    let SolverCallResult {
        obligation,
        step_number,
        prompt,
        goal_state,
        context_refs_json,
        response,
        worker_id: _worker_id,
        worker_model_name: _worker_model_name,
        solver_round_id: _solver_round_id,
        dispatch_mode: _dispatch_mode,
        tool_run_ids,
    } = result;
    state.selected_obligation = obligation;

    // Handle solver errors — deferred state mutation happens here
    let llm_result = match response {
        Ok(r) => r,
        Err(SolverError::Fatal(e)) => {
            return Ok(StepOutcome::Break(BreakReason::FatalApiError(e)));
        }
        Err(SolverError::StreamAborted(e)) => {
            state.consecutive_failures += 1;
            state.failures.push((
                "(stream aborted)".into(),
                format!("LLM stream aborted: {}", &e),
            ));
            state.failure_buffer.push(discerner::FailureEntry {
                step_number: Some(step_number),
                ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                failure_type: "llm_call".into(),
                category: "model".into(),
                reason: e,
                http_status: None,
                model: config.model_name.clone(),
                proposal_natural: None,
            });
            if state.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                return Ok(StepOutcome::Break(BreakReason::MaxConsecutiveFailures));
            }
            return Ok(StepOutcome::Continue);
        }
        Err(SolverError::Retryable(e)) => {
            state.consecutive_failures += 1;
            state
                .failures
                .push(("(LLM error)".into(), format!("Retryable LLM error: {}", &e)));
            state.failure_buffer.push(discerner::FailureEntry {
                step_number: Some(step_number),
                ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                failure_type: "llm_call".into(),
                category: "mechanical".into(),
                reason: e.clone(),
                http_status: discerner::extract_http_status(&e),
                model: config.model_name.clone(),
                proposal_natural: None,
            });
            emit_diagnostic(
                app_handle,
                "mechanical",
                "error",
                "llm_client",
                Some(step_number),
                &format!(
                    "LLM error ({}/{}): {}",
                    state.consecutive_failures, MAX_CONSECUTIVE_FAILURES, e
                ),
                serde_json::json!({"error": &e, "consecutive": state.consecutive_failures, "max": MAX_CONSECUTIVE_FAILURES}),
                &config.attempt_id,
            );
            if state.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                return Ok(StepOutcome::Break(BreakReason::MaxConsecutiveFailures));
            }
            return Ok(StepOutcome::Continue);
        }
    };

    tracing::debug!(
        step = step_number,
        tokens_in = ?llm_result.tokens_in,
        tokens_out = ?llm_result.tokens_out,
        response_len = llm_result.text.len(),
        "LLM response received"
    );

    // Parse LLM response — supports both single and batch (JSON array) formats.
    // If multiple proposals are returned, we process the first now and queue the rest
    // in state.pending_proposals for subsequent run_step calls (no extra LLM call).
    let mut proposal = match parse_proposals(&llm_result.text) {
        Some(mut proposals) => {
            let first = proposals.remove(0);
            if !proposals.is_empty() {
                tracing::info!(
                    "Batch mode: processing 1 of {} proposals, queueing {} for later",
                    proposals.len() + 1,
                    proposals.len()
                );
                state.pending_proposals.extend(proposals);
            }
            first
        }
        None => {
            let preview: String = llm_result.text.chars().take(300).collect();
            tracing::warn!(response = %preview, "Failed to parse LLM response as JSON");
            state.failures.push((
                llm_result.text.chars().take(200).collect(),
                "unparseable response".into(),
            ));
            state.consecutive_failures += 1;
            state.failure_buffer.push(discerner::FailureEntry {
                step_number: Some(step_number),
                ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                failure_type: "parse".into(),
                category: "model".into(),
                reason: format!(
                    "unparseable response: {}",
                    llm_result.text.chars().take(100).collect::<String>()
                ),
                http_status: None,
                model: config.model_name.clone(),
                proposal_natural: None,
            });
            emit_diagnostic(
                app_handle,
                "model",
                "warn",
                "parser",
                Some(step_number),
                &format!(
                    "Failed to parse LLM response ({}/{})",
                    state.consecutive_failures, MAX_CONSECUTIVE_FAILURES
                ),
                serde_json::json!({"preview": &preview, "consecutive": state.consecutive_failures}),
                &config.attempt_id,
            );
            if state.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                return Ok(StepOutcome::Break(BreakReason::MaxConsecutiveFailures));
            }
            return Ok(StepOutcome::Continue);
        }
    };

    // Parse succeeded — advance counter and reset consecutive failures.
    let (step_number, next_step_cursor) = resolve_step_cursor(state.step_number, step_number);
    state.step_number = next_step_cursor;
    state.consecutive_failures = 0;

    // Conclusion detection + hard block
    let mut is_conclusion =
        proposal.proposal_type.as_deref().unwrap_or("algebraic") == "conclusion";

    if is_conclusion && !open_obligations.is_empty() {
        tracing::info!(
            "Step {} BLOCKED: conclusion uncallable with {} open obligation(s) — discarding silently",
            step_number, open_obligations.len()
        );
        emit_diagnostic(
            app_handle,
            "gate",
            "info",
            "engine",
            Some(step_number),
            &format!(
                "Conclusion blocked: {} open obligation(s) — treating as non-conclusion",
                open_obligations.len()
            ),
            serde_json::json!({"open_count": open_obligations.len(), "action": "downgrade"}),
            &config.attempt_id,
        );
        is_conclusion = false;
        if let Some(ref mut pt) = proposal.proposal_type {
            *pt = "algebraic".to_string();
        }
        let gated_reason = format!(
            "BLOCKED: You used proposal_type \"conclusion\" but {} obligation(s) are still open. \
             \"conclusion\" is UNAVAILABLE until ALL obligations are closed. \
             Your step was processed as algebraic instead. Focus on resolving open obligations.",
            open_obligations.len()
        );
        state
            .failures
            .push((proposal.natural.clone(), gated_reason.clone()));
        state.failure_buffer.push(discerner::FailureEntry {
            step_number: Some(step_number),
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            failure_type: "conclusion_gated".into(),
            category: "model".into(),
            reason: gated_reason,
            http_status: None,
            model: config.model_name.clone(),
            proposal_natural: Some(proposal.natural.clone()),
        });
    }

    let proposal_type = proposal.proposal_type.as_deref().unwrap_or("algebraic");

    // === PRE-SUBMISSION CHECK ===
    // For typed claims (non-equality), use claim_check.
    // For equality claims, use the existing sympy_check path.
    if !is_conclusion {
        if let Some(ref typed_claim) = proposal.claim {
            if typed_claim.claim_type != "equality" {
                // Typed claim pre-check
                let pre_sidecar = crate::api::sidecar::SidecarClient::new();
                if let Ok(claim_val) = serde_json::to_value(typed_claim) {
                    if let Some(check) = pre_sidecar.claim_check(&claim_val).await {
                        if !check.verified {
                            tracing::info!(
                                "Step {} pre-check: typed claim ({}) failed: {}",
                                step_number,
                                typed_claim.claim_type,
                                check.reason
                            );
                            let _ = app_handle.emit(
                                "loop:claim_check_failed",
                                LoopClaimCheckFailedPayload {
                                    step_number,
                                    claim_type: typed_claim.claim_type.clone(),
                                    reason: check.reason.clone(),
                                },
                            );
                        }
                    }
                }
            }
        }

        // Equality pre-check (existing path)
        let formal_for_check = proposal.formal.clone();
        if proposal
            .claim
            .as_ref()
            .is_none_or(|c| c.claim_type == "equality")
        {
            if let Some(ref formal_str) = formal_for_check {
                let has_eq = formal_str.contains('=') && !formal_str.contains("==");
                let eq_count = formal_str.chars().filter(|&c| c == '=').count();
                if has_eq && eq_count == 1 {
                    if let Some((lhs, rhs)) = formal_str.split_once('=') {
                        let sympy_pre = crate::api::sidecar::SidecarClient::new();
                        if let Some(check) = sympy_pre.sympy_check(lhs.trim(), rhs.trim()).await {
                            if !check.is_equal {
                                if let Some(ref diff_str) = check.diff {
                                    tracing::info!(
                                        "Step {} pre-check: formal incorrect (diff={}), attempting correction",
                                        step_number, &diff_str[..diff_str.len().min(80)]
                                    );
                                    let _ = app_handle.emit(
                                        "loop:sympy_correction_attempt",
                                        LoopSympyCorrectionAttemptPayload {
                                            step_number,
                                            original_formal: formal_str.clone(),
                                            diff: diff_str.clone(),
                                        },
                                    );
                                    let correction_prompt = solver::build_sympy_correction_prompt(
                                        &proposal.natural.clone(),
                                        formal_str,
                                        diff_str,
                                    );
                                    let corr_handle = app_handle.clone();
                                    let corr_result = config
                                        .llm
                                        .complete_streaming(&correction_prompt, move |chunk| {
                                            emit_loop_token(
                                                &corr_handle,
                                                chunk,
                                                Some("solver_correction"),
                                                None,
                                            );
                                        })
                                        .await;
                                    if let Ok(corr_text) = corr_result {
                                        if let Some(corrected) =
                                            extract_corrected_formal(&corr_text.text)
                                        {
                                            tracing::info!(
                                                "Step {} SymPy correction applied",
                                                step_number
                                            );
                                            let _ = app_handle.emit(
                                                "loop:sympy_correction_applied",
                                                LoopSympyCorrectionAppliedPayload {
                                                    step_number,
                                                    original: formal_str.clone(),
                                                    corrected: corrected.clone(),
                                                },
                                            );
                                            proposal.formal = Some(corrected);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let formal = proposal.formal.as_deref();

    // RED-012: Track which obligation this step targets.
    let targeted_obligation_id = if let Some(ref sel) = state.selected_obligation {
        Some(sel.obligation.id.clone())
    } else if !open_obligations.is_empty() && !is_conclusion {
        Some(open_obligations[0].id.clone())
    } else {
        None
    };

    // Build step context with full lineage
    let ctx = StepContext {
        attempt_id: &config.attempt_id,
        parent_step_id: verified.last().map(|(id, _, _, _, _)| id.as_str()),
        step_number,
        model: &config.model_name,
        goal_state: &goal_state,
        context_refs: context_refs_json.as_deref(),
        context_provided: Some(&prompt),
        proposal_reasoning: proposal.reasoning.as_deref(),
        tokens_in: llm_result.tokens_in,
        tokens_out: llm_result.tokens_out,
        obligation_ref: targeted_obligation_id.as_deref(),
        branch_id: state.current_branch_id,
        problem_domain: Some(&config.problem.domain),
    };

    let verified_ok;
    #[allow(unused_assignments)] // set in conclusion/normal paths below
    let mut current_node_id: Option<String> = None;
    let mut current_step_id: Option<String> = None;
    let mut sympy_passed: Option<bool> = None;
    let mut pint_passed: Option<bool> = None;
    let mut lean_passed: Option<bool> = None;
    let mut rejection_reason: Option<String> = None;
    let should_run_lean = state.verified_count < 2
        || state.verified_count.is_multiple_of(LEAN_COUNCIL_INTERVAL)
        || proposal_type == "conclusion";

    // === CONCLUSION PATH (>= 3 verified steps) ===
    if is_conclusion && verified.len() >= 3 {
        // Call the conclusion handling helper
        let outcome = handle_conclusion(
            config,
            state,
            app_handle,
            &proposal,
            verified,
            open_obligations,
            all_obligations,
            &goal_state,
            &context_refs_json,
            &llm_result,
            &targeted_obligation_id,
            step_number,
            &tool_run_ids,
        )
        .await;
        return outcome;
    } else if is_conclusion {
        // Premature conclusion
        let reason = format!(
            "Premature conclusion: need at least 3 verified steps, currently have {}. Prove more algebraic steps first.",
            verified.len()
        );
        tracing::info!("Step {} REJECTED: {}", step_number, reason);

        use crate::db::StepRecord;
        let premature_rec = StepRecord {
            attempt_id: &config.attempt_id,
            parent_step_id: verified.last().map(|(id, _, _, _, _)| id.as_str()),
            step_number,
            model: &config.model_name,
            context_refs: context_refs_json.as_deref(),
            goal_state: &goal_state,
            context_provided: Some(&prompt),
            proposal_type: "conclusion",
            proposal_natural: &proposal.natural,
            proposal_formal: formal,
            proposal_reasoning: proposal.reasoning.as_deref(),
            sympy_result: None,
            sympy_passed: None,
            pint_result: None,
            pint_passed: None,
            lean_result: None,
            lean_passed: None,
            verified: false,
            rejection_reason: Some(&reason),
            model_tokens_in: llm_result.tokens_in,
            model_tokens_out: llm_result.tokens_out,
            wall_time_ms: None,
            challenge_model: None,
            challenge_flaw_found: None,
            challenge_attack: None,
            challenge_confidence: None,
            challenge_fatal: None,
            obligation_id: targeted_obligation_id.as_deref(),
            solver_round_id: None,
            solver_worker_id: None,
            solver_dispatch_mode: None,
            stale_sibling: false,
        };
        let premature_step_id = db_write_or_log(
            config.state.db.record_step(&premature_rec),
            "record_step(premature)",
            app_handle,
            &config.attempt_id,
        );

        // Backfill step_id on tool_runs created during this solver call
        if !premature_step_id.is_empty() && !tool_run_ids.is_empty() {
            let _ = config
                .state
                .db
                .backfill_tool_runs_step_id(&tool_run_ids, &premature_step_id);
        }

        let parent_node_id = if let Some(parent_sid) = verified.last().map(|(id, ..)| id.as_str()) {
            config
                .state
                .db
                .get_node_by_step_id(parent_sid)
                .ok()
                .flatten()
                .map(|n| n.id)
        } else {
            None
        };
        let premature_node_id = db_write_or_log(
            config.state.db.create_node(
                &config.attempt_id,
                state.current_branch_id,
                "closure",
                parent_node_id.as_deref(),
                &proposal.natural,
                formal,
                None,
                None,
                "rejected",
                None,
                Some(&format!(
                    "{{\"premature\": true, \"verified_count\": {}}}",
                    verified.len()
                )),
                Some(&config.model_name),
                None,
                Some(&premature_step_id),
                llm_result.tokens_in,
                step_number,
            ),
            "create_node(premature)",
            app_handle,
            &config.attempt_id,
        );
        let _ = config.state.db.append_dag_event(
            &config.attempt_id,
            "conclusion_premature",
            &serde_json::json!({"node_id": &premature_node_id, "verified_count": verified.len()})
                .to_string(),
            "loop_engine",
        );
        current_node_id = if premature_node_id.is_empty() {
            None
        } else {
            Some(premature_node_id)
        };

        verified_ok = false;
        state
            .failures
            .push((proposal.natural.clone(), reason.clone()));
        state.failure_buffer.push(discerner::FailureEntry {
            step_number: Some(step_number),
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            failure_type: "premature_conclusion".into(),
            category: "model".into(),
            reason: reason.clone(),
            http_status: None,
            model: config.model_name.clone(),
            proposal_natural: Some(proposal.natural.clone()),
        });
        rejection_reason = Some(reason);
    } else {
        // Normal step: validate through sidecar pipeline.
        // Serialize typed claim for the sidecar (if present).
        let claim_json = proposal
            .claim
            .as_ref()
            .and_then(|c| serde_json::to_value(c).ok());
        let result = match config
            .pipeline
            .validate_and_record(
                &config.state.db,
                &ctx,
                proposal_type,
                &proposal.natural,
                formal,
                proposal.formal_lean.as_deref(),
                should_run_lean,
                claim_json.as_ref(),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Verification pipeline error at step {}: {}", step_number, e);
                emit_diagnostic(
                    app_handle,
                    "mechanical",
                    "error",
                    "validator",
                    Some(step_number),
                    &format!("Validation pipeline error: {}", e),
                    serde_json::json!({"error": &e}),
                    &config.attempt_id,
                );
                let _ = app_handle.emit(
                    "loop:step_complete",
                    StepEvent {
                        attempt_id: config.attempt_id.clone(),
                        step_number,
                        proposal_type: proposal_type.to_string(),
                        proposal_natural: proposal.natural.clone(),
                        proposal_formal: proposal.formal.clone(),
                        proposal_reasoning: proposal.reasoning.clone(),
                        verified: false,
                        rejection_reason: Some(format!("Validation pipeline error: {}", e)),
                        model: config.model_name.clone(),
                        sympy_passed: None,
                        pint_passed: None,
                        lean_passed: None,
                        challenge_model: None,
                        challenge_flaw_found: None,
                        challenge_attack: None,
                        challenge_confidence: None,
                        challenge_fatal: None,
                        obligation_id: targeted_obligation_id.clone(),
                        obligation_desc: state
                            .selected_obligation
                            .as_ref()
                            .map(|s| s.obligation.description.clone()),
                        obligation_type: state
                            .selected_obligation
                            .as_ref()
                            .map(|s| s.obligation.obligation_type.clone()),
                        solver_round_id: None,
                        solver_worker_id: None,
                        solver_dispatch_mode: None,
                        stale_sibling: None,
                    },
                );
                state.failures.push((
                    proposal.natural.clone(),
                    format!("Validation pipeline error: {}", e),
                ));
                state.failure_buffer.push(discerner::FailureEntry {
                    step_number: Some(step_number),
                    ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    failure_type: "validation_pipeline".into(),
                    category: "mechanical".into(),
                    reason: format!("Validation pipeline error: {}", e),
                    http_status: None,
                    model: config.model_name.clone(),
                    proposal_natural: Some(proposal.natural.clone()),
                });
                return Ok(StepOutcome::Continue);
            }
        };
        sympy_passed = result.sympy_passed;
        pint_passed = result.pint_passed;
        lean_passed = result.lean_passed;
        verified_ok = result.verified;
        current_step_id = if result.step_id.is_empty() {
            None
        } else {
            Some(result.step_id)
        };
        current_node_id = if result.node_id.is_empty() {
            None
        } else {
            Some(result.node_id)
        };

        // Backfill step_id on tool_runs created during this solver call
        if let Some(ref sid) = current_step_id {
            if !tool_run_ids.is_empty() {
                let _ = config
                    .state
                    .db
                    .backfill_tool_runs_step_id(&tool_run_ids, sid);
            }
        }

        if !verified_ok {
            if let Some(detailed) = result.rejection_reason {
                rejection_reason = Some(detailed);
            } else {
                let mut reasons = vec![];
                if let Some(false) = sympy_passed {
                    reasons.push("SymPy rejected");
                }
                if let Some(false) = pint_passed {
                    reasons.push("Pint rejected");
                }
                if let Some(false) = lean_passed {
                    reasons.push("Lean rejected");
                }
                if reasons.is_empty() && formal.is_none() {
                    reasons.push("No formal expression provided");
                } else if reasons.is_empty() {
                    reasons.push("No validators matched (formal must be a single equality with =)");
                }
                rejection_reason = Some(reasons.join("; "));
            }
        }
    }

    // Track outcome
    state
        .orchestrator
        .record_outcome(&config.model_name, verified_ok);

    // === Obligation Queue Feedback ===
    if let Some(ref sel) = state.selected_obligation {
        let fresh_steps = sel.obligation.steps_spent + 1;
        if fresh_steps >= sel.obligation.max_steps && !verified_ok {
            tracing::info!(
                "Obligation budget exhausted: '{}' ({}/{} steps, no resolution)",
                sel.obligation.description,
                fresh_steps,
                sel.obligation.max_steps
            );
            let _ = config.state.db.demote_obligation(&sel.obligation.id);
            let _ = config.state.db.append_dag_event(
                &config.attempt_id, "obligation_demoted",
                &serde_json::json!({
                    "obligation_id": &sel.obligation.id,
                    "reason": format!("budget exhausted: {}/{} steps", fresh_steps, sel.obligation.max_steps),
                }).to_string(),
                "loop_engine",
            );
            emit_obligation_closed(
                app_handle,
                &sel.obligation.id,
                ObligationStatus::Demoted,
                None,
                None,
                Some(&format!(
                    "Budget exhausted: {}/{} steps without resolution",
                    fresh_steps, sel.obligation.max_steps
                )),
                None,
                None,
            );
        }

        if verified_ok {
            state.pivot_tracker.record_success(&sel.obligation.id);
        } else {
            let technique_class = proposal_type.to_string();
            let failure_reason = rejection_reason
                .as_deref()
                .unwrap_or("rejected")
                .to_string();
            if let Some((blacklisted_tech, _reason)) = state.pivot_tracker.record_failure(
                &sel.obligation.id,
                &technique_class,
                &failure_reason,
            ) {
                tracing::warn!(
                    "Pivot forced: blacklisting '{}' for obligation '{}'",
                    blacklisted_tech,
                    sel.obligation.description
                );
                let _ = app_handle.emit(
                    "loop:pivot_forced",
                    LoopPivotForcedPayload {
                        attempt_id: Some(config.attempt_id.clone()),
                        obligation_id: Some(sel.obligation.id.clone()),
                        obligation_desc: sel.obligation.description.clone(),
                        blacklisted_technique: blacklisted_tech.clone(),
                    },
                );
                let _ = config.state.db.append_dag_event(
                    &config.attempt_id,
                    "pivot_forced",
                    &serde_json::json!({
                        "obligation_id": &sel.obligation.id,
                        "technique": &blacklisted_tech,
                        "reason": &failure_reason,
                    })
                    .to_string(),
                    "obligation_queue",
                );

                // === Obligation-Level Research Injection ===
                // Always research on first encounter with an obligation.
                // On subsequent encounters, research again when 2+ techniques are blacklisted.
                // Research results are injected into the solver prompt to reduce wasted steps.
                let bl_count = state.pivot_tracker.blacklisted_count(&sel.obligation.id);
                let last_scout_bl = state
                    .obligation_scout_bl_at
                    .get(&sel.obligation.id)
                    .copied()
                    .unwrap_or(0);
                // Research before every solver attempt: on first encounter, and again whenever
                // new techniques have been blacklisted since the last scout.
                let should_research = !config.scout_sources.is_empty()
                    && (last_scout_bl == 0 || bl_count > last_scout_bl);
                tracing::debug!(
                    "Obligation research check: bl_count={}, last_scout_bl={}, scout_sources={}, will_research={}",
                    bl_count,
                    last_scout_bl,
                    config.scout_sources.len(),
                    should_research,
                );
                if should_research {
                    let sidecar_scout = crate::api::sidecar::SidecarClient::new();
                    let failed_techniques = state
                        .pivot_tracker
                        .blacklisted_techniques(&sel.obligation.id);
                    let failed_techniques_text = failed_techniques.join(", ");
                    let scout_query = if failed_techniques.is_empty() {
                        // First encounter — general research for this obligation
                        format!(
                            "Techniques and theorems for: {}. Problem context: {}",
                            sel.obligation.description,
                            config
                                .problem
                                .statement
                                .chars()
                                .take(200)
                                .collect::<String>(),
                        )
                    } else {
                        // Re-research after failures
                        format!(
                            "How to prove: {}. Failed approaches: {}",
                            sel.obligation.description, failed_techniques_text,
                        )
                    };
                    tracing::info!(
                        "Obligation research triggered for '{}' (bl_count={}, last_scout_bl={}, {} techniques blacklisted)",
                        sel.obligation.description,
                        bl_count,
                        last_scout_bl,
                        failed_techniques.len(),
                    );
                    match sidecar_scout
                        .scout_briefing_mid_solve(
                            &scout_query,
                            Some(&config.problem.domain),
                            &config.scout_sources,
                        )
                        .await
                    {
                        Ok(resp) if !resp.briefing.is_empty() => {
                            tracing::info!(
                                "Obligation scout: {} results for '{}'",
                                resp.results_count,
                                sel.obligation.description,
                            );
                            emit_diagnostic(
                                app_handle,
                                "info",
                                "info",
                                "scout",
                                Some(step_number),
                                &format!(
                                    "Obligation scout: {} results for stuck obligation '{}'",
                                    resp.results_count, sel.obligation.description,
                                ),
                                serde_json::json!({
                                    "obligation_id": &sel.obligation.id,
                                    "results_count": resp.results_count,
                                    "blacklisted_techniques": &failed_techniques,
                                }),
                                &config.attempt_id,
                            );
                            let _ = app_handle.emit(
                                "agent:scout_result",
                                AgentScoutResultPayload {
                                    trigger: ScoutTrigger::MidSolve,
                                    results_count: resp.results_count,
                                    sources: resp.sources_queried,
                                    briefing: resp.briefing.clone(),
                                    obligation_id: Some(sel.obligation.id.clone()),
                                    obligation_desc: Some(sel.obligation.description.clone()),
                                    blacklisted_techniques: Some(failed_techniques.clone()),
                                },
                            );
                            state
                                .obligation_scout_results
                                .insert(sel.obligation.id.clone(), resp.briefing);
                            state.obligation_scouted.insert(sel.obligation.id.clone());
                            state
                                .obligation_scout_bl_at
                                .insert(sel.obligation.id.clone(), bl_count);
                        }
                        Ok(_) => {
                            tracing::info!(
                                "Obligation scout: no results for '{}'",
                                sel.obligation.description
                            );
                            state.obligation_scouted.insert(sel.obligation.id.clone());
                            state
                                .obligation_scout_bl_at
                                .insert(sel.obligation.id.clone(), bl_count);
                        }
                        Err(e) => {
                            tracing::warn!("Obligation scout failed: {} — continuing without", e);
                        }
                    }
                }
            }
        }
    }

    // Emit diagnostic for validation result
    let lean_label = match (should_run_lean, lean_passed) {
        (false, _) => "skipped",
        (true, Some(true)) => "agreed",
        (true, Some(false)) => "disagreed",
        (true, None) => "no_opinion",
    };
    if verified_ok {
        emit_diagnostic(
            app_handle,
            "info",
            "info",
            "validator",
            Some(step_number),
            &format!(
                "Step {} VERIFIED (SymPy={:?} Lean={} Pint={:?})",
                step_number, sympy_passed, lean_label, pint_passed
            ),
            serde_json::json!({"sympy": sympy_passed, "lean": lean_passed, "lean_council": lean_label, "pint": pint_passed}),
            &config.attempt_id,
        );
    } else if !is_conclusion {
        emit_diagnostic(
            app_handle,
            "model",
            "warn",
            "validator",
            Some(step_number),
            &format!(
                "Step {} REJECTED: {}",
                step_number,
                rejection_reason.as_deref().unwrap_or("unknown")
            ),
            serde_json::json!({"sympy": sympy_passed, "lean": lean_passed, "lean_council": lean_label, "pint": pint_passed, "reason": &rejection_reason}),
            &config.attempt_id,
        );
    }

    // Increment running counters on the attempt row
    if verified_ok {
        let _ = config
            .state
            .db
            .increment_attempt_counter(&config.attempt_id, "steps_verified");
    } else {
        let _ = config
            .state
            .db
            .increment_attempt_counter(&config.attempt_id, "steps_rejected");
    }

    if verified_ok {
        tracing::info!("Step {} VERIFIED: {}", step_number, proposal.natural);
        state.failures.clear();
        state.failure_buffer.reset();
        state.verified_since_audit += 1;
        state.verified_count += 1;

        // === SUSPECTED ANSWER DISPROVAL CHECK ===
        // If a verified step explicitly contradicts the suspected answer, disprove it.
        if let Some(ref mut sa) = state.suspected_answer {
            if !sa.disproved {
                if let Some(reason) =
                    check_disproval(&proposal.natural, proposal.formal.as_deref(), &sa.value)
                {
                    sa.disproved = true;
                    sa.disproval_reason = Some(reason.clone());
                    tracing::warn!(
                        "Step {} DISPROVED suspected answer '{}': {}",
                        step_number,
                        sa.value,
                        reason
                    );
                    let _ = app_handle.emit(
                        "loop:suspected_answer_disproved",
                        LoopSuspectedAnswerDisprovedPayload {
                            step_number,
                            suspected_value: sa.value.clone(),
                            source: sa.source.clone(),
                            reason: reason.clone(),
                        },
                    );
                    emit_diagnostic(
                        app_handle,
                        "info",
                        "warn",
                        "disproval",
                        Some(step_number),
                        &format!("Suspected answer '{}' disproved: {}", sa.value, reason),
                        serde_json::json!({
                            "suspected_value": &sa.value,
                            "source": &sa.source,
                            "reason": &reason,
                        }),
                        &config.attempt_id,
                    );
                }
            }
        }

        // === CLAIM EXTRACTION ===
        let stream_claims: Vec<claim_extractor::ExtractedClaim> = {
            let monitor = state.claim_monitor.lock().unwrap_or_else(|poisoned| {
                tracing::warn!("claim_monitor mutex was poisoned, recovering");
                poisoned.into_inner()
            });
            monitor
                .claims_for_step(step_number)
                .into_iter()
                .cloned()
                .collect()
        };
        let batch_claims =
            claim_extractor::extract_claims(&proposal.natural, proposal.reasoning.as_deref());
        let mut all_formals = std::collections::HashSet::new();
        let mut merged_claims: Vec<claim_extractor::ExtractedClaim> = Vec::new();
        for c in stream_claims.into_iter().chain(batch_claims.into_iter()) {
            if all_formals.insert(c.formal.clone()) {
                merged_claims.push(c);
            }
        }
        if !merged_claims.is_empty() {
            tracing::info!(
                "Step {} total claims (stream+batch): {:?}",
                step_number,
                merged_claims
                    .iter()
                    .map(|c| format!("{} ({})", c.formal, c.source))
                    .collect::<Vec<_>>()
            );
            let _ = app_handle.emit(
                "loop:claims_extracted",
                LoopClaimsExtractedPayload {
                    step_number,
                    claims: merged_claims
                        .iter()
                        .map(|claim| {
                            claim_event_record(
                                &claim.raw_text,
                                &claim.formal,
                                &claim.source,
                                Some(claim.offset),
                            )
                        })
                        .collect(),
                },
            );

            if let Some(ref sid) = current_step_id {
                // If the proposal has a typed claim, record it with the precise type
                if let Some(ref typed_claim) = proposal.claim {
                    let ct = typed_claim.claim_type.as_str();
                    let (object, value) = match ct {
                        "divisibility" => (
                            typed_claim.dividend.as_deref().unwrap_or(""),
                            typed_claim.divisor.as_deref(),
                        ),
                        "inequality" => (
                            typed_claim.lhs.as_deref().unwrap_or(""),
                            typed_claim.rhs.as_deref(),
                        ),
                        "gcd" => (
                            typed_claim.a.as_deref().unwrap_or(""),
                            typed_claim.value.as_deref(),
                        ),
                        "congruence" => (
                            typed_claim.expr.as_deref().unwrap_or(""),
                            typed_claim.remainder.as_deref(),
                        ),
                        "for_all" => (
                            typed_claim.predicate.as_deref().unwrap_or(""),
                            typed_claim.domain.as_deref(),
                        ),
                        _ => {
                            let f = proposal.formal.as_deref().unwrap_or("");
                            let obj = f.split('=').next().unwrap_or(f).trim();
                            let val = f.split_once('=').map(|x| x.1).map(|s| s.trim());
                            (obj, val)
                        }
                    };
                    let raw = proposal.formal.as_deref().unwrap_or(&proposal.natural);
                    let _ = config.state.db.create_claim(
                        sid,
                        &config.attempt_id,
                        ct,
                        object,
                        None,
                        None,
                        None,
                        None,
                        value,
                        raw,
                        1.0,
                    );
                }

                // Also record any regex-extracted claims
                for c in &merged_claims {
                    let claim_type = if c.formal.contains("=") {
                        "equality"
                    } else {
                        "assertion"
                    };
                    let object = c.formal.split('=').next().unwrap_or(&c.formal).trim();
                    let value: Option<&str> =
                        c.formal.split_once('=').map(|x| x.1).map(|s| s.trim());
                    let _ = config.state.db.create_claim(
                        sid,
                        &config.attempt_id,
                        claim_type,
                        object,
                        None,
                        None,
                        None,
                        None,
                        value,
                        &c.raw_text,
                        1.0,
                    );
                }
            }
        }

        // === ADVERSARIAL NODE CHALLENGE ===
        let mut challenge_info: Option<(String, bool, String, f64, bool)> = None;
        {
            // Build validation summary for the challenger
            let validation_summary = {
                let mut parts = vec![];
                if let Some(sp) = sympy_passed {
                    parts.push(format!("SymPy: {}", if sp { "PASS" } else { "FAIL" }));
                }
                if let Some(pp) = pint_passed {
                    parts.push(format!("Pint: {}", if pp { "PASS" } else { "FAIL" }));
                }
                if let Some(lp) = lean_passed {
                    parts.push(format!("Lean: {}", if lp { "PASS" } else { "FAIL" }));
                }
                parts.join(", ")
            };
            // Get target obligation if present
            let target_ob = state.selected_obligation.as_ref().map(|s| &s.obligation);
            let challenge_prompt = critic::build_node_challenge_prompt(
                &config.problem.statement,
                &proposal.natural,
                formal,
                proposal.reasoning.as_deref(),
                verified,
                &config.model_name,
                &config.enriched_analyst_context,
                target_ob,
                &validation_summary,
            );

            emit_loop_thinking_start(
                app_handle,
                Some(step_number),
                &config.adversary_model_name,
                Some("challenger"),
                targeted_obligation_id.as_deref(),
                None,
                None,
            );
            let challenge_handle = app_handle.clone();
            let challenge_obligation_id = targeted_obligation_id.clone();
            let challenge_result = config
                .adversary_llm
                .complete_streaming(&challenge_prompt, move |chunk| {
                    emit_loop_token(
                        &challenge_handle,
                        chunk,
                        Some("challenger"),
                        challenge_obligation_id.as_deref(),
                    );
                })
                .await;
            emit_loop_thinking_end(app_handle, targeted_obligation_id.as_deref());

            if let Ok(challenge_resp) = challenge_result {
                if response_guard::is_repetition_loop(&challenge_resp.text) {
                    tracing::warn!(
                        "Step {}: adversary response is a repetition loop — skipping challenge",
                        step_number
                    );
                    emit_diagnostic(
                        app_handle,
                        "model",
                        "warn",
                        "response_guard",
                        Some(step_number),
                        "Adversary response: repetition loop detected — challenge skipped",
                        serde_json::json!({"model": &config.adversary_model_name}),
                        &config.attempt_id,
                    );
                } else if let Some(challenge) = critic::parse_node_challenge(&challenge_resp.text) {
                    let display_text = if !challenge.attack_vector.is_empty() {
                        challenge.attack_vector.clone()
                    } else if !challenge.reasoning.is_empty() {
                        challenge.reasoning.clone()
                    } else {
                        String::new()
                    };

                    tracing::info!(
                        "Node challenge (step {}): flaw_found={}, confidence={:.2}, fatal={}, text='{}'",
                        step_number, challenge.flaw_found, challenge.confidence, challenge.fatal,
                        if display_text.len() > 120 { &display_text[..display_text.floor_char_boundary(120)] } else { &display_text }
                    );

                    let _ = app_handle.emit(
                        "loop:node_challenged",
                        LoopNodeChallengedPayload {
                            step_number,
                            adversary_model: config.adversary_model_name.clone(),
                            flaw_found: challenge.flaw_found,
                            attack_vector: display_text.clone(),
                            confidence: challenge.confidence,
                            fatal: challenge.fatal,
                            suggested_fix: if challenge.suggested_fix.is_empty() {
                                None
                            } else {
                                Some(challenge.suggested_fix.clone())
                            },
                            reasoning: if challenge.reasoning.is_empty() {
                                None
                            } else {
                                Some(challenge.reasoning.clone())
                            },
                        },
                    );

                    challenge_info = Some((
                        config.adversary_model_name.clone(),
                        challenge.flaw_found,
                        display_text,
                        challenge.confidence,
                        challenge.fatal,
                    ));

                    let _ = config.state.db.append_dag_event(
                        &config.attempt_id,
                        "node_challenged",
                        &serde_json::json!({
                            "step_number": step_number,
                            "node_id": &current_node_id,
                            "challenger": &config.adversary_model_name,
                            "solver": &config.model_name,
                            "flaw_found": challenge.flaw_found,
                            "fatal": challenge.fatal,
                            "confidence": challenge.confidence,
                            "attack_vector": &challenge.attack_vector,
                        })
                        .to_string(),
                        "critic",
                    );

                    if let Some(ref sid) = current_step_id {
                        let _ = config.state.db.update_step_challenge(
                            sid,
                            &config.adversary_model_name,
                            challenge.flaw_found,
                            &challenge.attack_vector,
                            challenge.confidence,
                            challenge.fatal,
                            None,
                        );
                    }

                    if challenge.flaw_found && challenge.fatal && challenge.confidence >= 0.7 {
                        tracing::warn!(
                            "Step {} REJECTED by adversarial challenge: {} (confidence: {:.0}%)",
                            step_number,
                            challenge.attack_vector,
                            challenge.confidence * 100.0
                        );
                        emit_diagnostic(
                            app_handle,
                            "model",
                            "warn",
                            "challenger",
                            Some(step_number),
                            &format!(
                                "Adversarial veto ({:.0}%): {}",
                                challenge.confidence * 100.0,
                                challenge.attack_vector
                            ),
                            serde_json::json!({"challenger": &config.adversary_model_name, "attack": &challenge.attack_vector,
                                "confidence": challenge.confidence, "fatal": true}),
                            &config.attempt_id,
                        );

                        let attack_reason = format!(
                            "ADVERSARIAL CHALLENGE FAILED ({}): {} — {}",
                            &config.adversary_model_name,
                            challenge.attack_vector,
                            challenge.suggested_fix
                        );
                        state
                            .failures
                            .push((proposal.natural.clone(), attack_reason.clone()));
                        state.failure_buffer.push(discerner::FailureEntry {
                            step_number: Some(step_number),
                            ts: chrono::Utc::now()
                                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                            failure_type: "adversarial_veto".into(),
                            category: "model".into(),
                            reason: attack_reason.clone(),
                            http_status: None,
                            model: config.model_name.clone(),
                            proposal_natural: Some(proposal.natural.clone()),
                        });

                        let _ = app_handle.emit(
                            "loop:step_complete",
                            StepEvent {
                                attempt_id: config.attempt_id.clone(),
                                step_number,
                                proposal_type: proposal_type.to_string(),
                                proposal_natural: proposal.natural.clone(),
                                proposal_formal: proposal.formal.clone(),
                                proposal_reasoning: proposal.reasoning.clone(),
                                verified: false,
                                rejection_reason: Some(attack_reason.clone()),
                                model: config.model_name.clone(),
                                sympy_passed,
                                pint_passed,
                                lean_passed,
                                challenge_model: Some(config.adversary_model_name.clone()),
                                challenge_flaw_found: Some(challenge.flaw_found),
                                challenge_attack: Some(challenge.attack_vector.clone()),
                                challenge_confidence: Some(challenge.confidence),
                                challenge_fatal: Some(challenge.fatal),
                                obligation_id: targeted_obligation_id.clone(),
                                obligation_desc: state
                                    .selected_obligation
                                    .as_ref()
                                    .map(|s| s.obligation.description.clone()),
                                obligation_type: state
                                    .selected_obligation
                                    .as_ref()
                                    .map(|s| s.obligation.obligation_type.clone()),
                                solver_round_id: None,
                                solver_worker_id: None,
                                solver_dispatch_mode: None,
                                stale_sibling: None,
                            },
                        );

                        if let Some(ref sid) = current_step_id {
                            let _ = config.state.db.update_step_challenge(
                                sid,
                                &config.adversary_model_name,
                                challenge.flaw_found,
                                &challenge.attack_vector,
                                challenge.confidence,
                                challenge.fatal,
                                Some(&attack_reason),
                            );
                        }

                        if let Some(ref nid) = current_node_id {
                            let _ = config.state.db.update_node_status(nid, "rejected");
                        }

                        return Ok(StepOutcome::Continue);
                    }
                }
            } else {
                tracing::warn!("Adversarial challenge LLM call failed — step survives (fail-open)");
                emit_diagnostic(
                    app_handle,
                    "mechanical",
                    "warn",
                    "challenger",
                    Some(step_number),
                    "Challenger LLM call failed — step survives (fail-open)",
                    serde_json::json!({"model": &config.adversary_model_name}),
                    &config.attempt_id,
                );
            }
        }

        // Step survived validation + adversarial challenge
        let _ = app_handle.emit(
            "loop:step_complete",
            StepEvent {
                attempt_id: config.attempt_id.clone(),
                step_number,
                proposal_type: proposal_type.to_string(),
                proposal_natural: proposal.natural.clone(),
                proposal_formal: proposal.formal.clone(),
                proposal_reasoning: proposal.reasoning.clone(),
                verified: true,
                rejection_reason: None,
                model: config.model_name.clone(),
                sympy_passed,
                pint_passed,
                lean_passed,
                challenge_model: challenge_info.as_ref().map(|c| c.0.clone()),
                challenge_flaw_found: challenge_info.as_ref().map(|c| c.1),
                challenge_attack: challenge_info.as_ref().map(|c| c.2.clone()),
                challenge_confidence: challenge_info.as_ref().map(|c| c.3),
                challenge_fatal: challenge_info.as_ref().map(|c| c.4),
                obligation_id: targeted_obligation_id.clone(),
                obligation_desc: state
                    .selected_obligation
                    .as_ref()
                    .map(|s| s.obligation.description.clone()),
                obligation_type: state
                    .selected_obligation
                    .as_ref()
                    .map(|s| s.obligation.obligation_type.clone()),
                solver_round_id: None,
                solver_worker_id: None,
                solver_dispatch_mode: None,
                stale_sibling: None,
            },
        );

        // dag_edges
        if let Some(ref nid) = current_node_id {
            if let Some(parent_sid) = verified.last().map(|(id, ..)| id.as_str()) {
                if let Ok(Some(parent_node)) = config.state.db.get_node_by_step_id(parent_sid) {
                    let _ = config.state.db.create_edge(
                        nid,
                        &parent_node.id,
                        "step",
                        "step",
                        "depends_on",
                        None,
                    );
                }
            }
            if let Some(ref ob_id) = targeted_obligation_id {
                let _ =
                    config
                        .state
                        .db
                        .create_edge(nid, ob_id, "step", "obligation", "targets", None);
            }
        }

        let _ = config
            .state
            .db
            .increment_branch_steps(state.current_branch_id as i64);
        // Track steps spent on this obligation for budget enforcement
        if let Some(ref ob_id) = targeted_obligation_id {
            let _ = config.state.db.increment_obligation_steps(ob_id);
        }
        // Backfill contrastive pairs: pair this accepted step with any prior
        // rejected steps at the same step_number (they were recorded before us).
        if let Some(ref accepted_id) = current_step_id {
            let rejected = config
                .state
                .db
                .get_rejected_step_ids_at(step_number, &config.attempt_id);
            for (rejected_id, reason) in &rejected {
                let _ = config.state.db.record_contrastive_pair(
                    accepted_id,
                    rejected_id,
                    step_number,
                    &config.attempt_id,
                    reason.as_deref(),
                );
            }
        }

        let tunnel_detected = state.orchestrator.record_technique(proposal_type);
        if tunnel_detected {
            tracing::info!(
                "Technique tunnel detected: consecutive '{}' steps on branch {}",
                proposal_type,
                state.current_branch_id
            );
        }

        // === Exploration Audit ===
        let current_open = config
            .state
            .db
            .count_open_obligations(&config.attempt_id)
            .unwrap_or(0) as usize;
        if state.verified_since_audit >= AUDIT_INTERVAL {
            state.verified_since_audit = 0;
            let chain = config
                .state
                .db
                .get_branch_verified_chain(&config.attempt_id, state.current_branch_id as i64)
                .or_else(|_| config.state.db.get_verified_chain(&config.attempt_id))
                .unwrap_or_default();
            let obligations_capped = current_open >= MAX_OPEN_OBLIGATIONS;
            tracing::info!("Running exploration audit at {} verified steps ({} open obligations, branch {}, obligations_capped={})", chain.len(), current_open, state.current_branch_id, obligations_capped);

            let _ = config.state.db.append_dag_event(
                &config.attempt_id, "audit_started",
                &serde_json::json!({"step_number": step_number, "chain_length": chain.len(), "branch_id": state.current_branch_id}).to_string(),
                "audit",
            );

            let all_obligations = config
                .state
                .db
                .get_all_obligations(&config.attempt_id)
                .unwrap_or_default();
            let auditable_obligations: Vec<_> = all_obligations
                .iter()
                .filter(|ob| {
                    if ob.status != "open" && ob.status != "assigned" {
                        return true;
                    }
                    let nodes = config
                        .state
                        .db
                        .get_nodes_for_obligation(&ob.id)
                        .unwrap_or_default();
                    obligation_needs_llm_review(&ob.id, None, false, &nodes)
                })
                .cloned()
                .collect();
            let audit_prompt = audit::build_audit_prompt(
                &config.problem.statement,
                &chain,
                config.problem.domain.as_str(),
                &config.enriched_analyst_context,
                &auditable_obligations,
                &config.techniques,
            );

            emit_loop_thinking_start(
                app_handle,
                Some(step_number),
                &config.model_name,
                Some("auditor"),
                targeted_obligation_id.as_deref(),
                None,
                None,
            );
            let audit_handle = app_handle.clone();
            let audit_obligation_id = targeted_obligation_id.clone();
            let audit_result = config
                .reviewer_llm
                .complete_streaming(&audit_prompt, move |chunk| {
                    emit_loop_token(
                        &audit_handle,
                        chunk,
                        Some("auditor"),
                        audit_obligation_id.as_deref(),
                    );
                })
                .await;
            emit_loop_thinking_end(app_handle, targeted_obligation_id.as_deref());

            if let Ok(audit_resp) = audit_result {
                if let Some(parsed) = audit::parse_audit(&audit_resp.text) {
                    tracing::info!(
                        breadth = parsed.exploration_breadth,
                        explored = ?parsed.techniques_explored,
                        missing = ?parsed.techniques_missing,
                        should_branch = parsed.should_branch,
                        "Exploration audit complete"
                    );

                    let audit_session_id = config
                        .state
                        .db
                        .record_council_session(
                            "exploration_audit",
                            &config.problem_id,
                            Some(&config.attempt_id),
                            &config.reviewer_llm.model_name(),
                            &audit_resp.text,
                            parsed.obligations.len() as u32,
                        )
                        .ok();

                    let _ = config.state.db.append_dag_event(
                        &config.attempt_id,
                        "audit_completed",
                        &serde_json::json!({
                            "breadth": parsed.exploration_breadth,
                            "should_branch": parsed.should_branch,
                            "obligations_proposed": parsed.obligations.len(),
                            "confidence": parsed.confidence_in_current_path,
                            "session_id": &audit_session_id,
                        })
                        .to_string(),
                        "audit",
                    );

                    let _ = app_handle.emit(
                        "loop:audit_complete",
                        LoopAuditResult {
                            step_number,
                            breadth: parsed.exploration_breadth,
                            techniques_explored: parsed.techniques_explored.clone(),
                            techniques_missing: parsed.techniques_missing.clone(),
                            recommended_direction: parsed.recommended_direction.clone(),
                            should_branch: parsed.should_branch,
                            confidence: parsed.confidence_in_current_path,
                        },
                    );

                    // Create obligations from audit
                    if parsed.should_branch && !parsed.obligations.is_empty() && !obligations_capped
                    {
                        let parent_node_id = chain.last().and_then(|(step_id, ..)| {
                            config
                                .state
                                .db
                                .get_node_by_step_id(step_id)
                                .ok()
                                .flatten()
                                .map(|n| n.id)
                        });
                        let parent_node = match parent_node_id.as_deref() {
                            Some(id) => id,
                            None => {
                                tracing::error!("Cannot create obligations: no proof_node found for last verified step. Skipping {} obligations.", parsed.obligations.len());
                                state.last_audit = Some(parsed);
                                return Ok(StepOutcome::Continue);
                            }
                        };
                        let evidence_facts = evidence::extract_evidence(&chain);
                        let mut high_priority_obs: Vec<crate::models::dag::Obligation> = Vec::new();
                        for ob in &parsed.obligations {
                            if let Some(reason) = evidence::proposal_conflicts_with_evidence(
                                &evidence_facts,
                                &ob.description,
                            ) {
                                tracing::warn!(
                                    "Obligation BLOCKED by evidence filter: {} — {}",
                                    ob.description,
                                    reason
                                );
                                let _ = app_handle.emit(
                                    "loop:obligation_blocked",
                                    LoopObligationBlockedPayload {
                                        description: ob.description.clone(),
                                        reason: reason.clone(),
                                        audit_session_id: audit_session_id.clone(),
                                    },
                                );
                                continue;
                            }
                            match config.state.db.create_obligation(
                                &config.attempt_id,
                                state.current_branch_id,
                                parent_node,
                                &ob.description,
                                &ob.obligation_type,
                                ob.priority,
                                ob.confidence,
                                Some(2),
                                Some(20),
                            ) {
                                Ok(id) => {
                                    tracing::info!(
                                        "Obligation opened: {} (id: {})",
                                        ob.description,
                                        id
                                    );
                                    let _ = config.state.db.append_dag_event(
                                        &config.attempt_id,
                                        "obligation_opened",
                                        &serde_json::json!({
                                            "obligation_id": &id,
                                            "parent_node": parent_node,
                                            "description": &ob.description,
                                            "priority": ob.priority,
                                            "audit_session_id": &audit_session_id,
                                        })
                                        .to_string(),
                                        "audit",
                                    );
                                    let _ = app_handle.emit(
                                        "loop:obligation_opened",
                                        LoopObligationOpenedPayload {
                                            id: id.clone(),
                                            description: ob.description.clone(),
                                            obligation_type: ob.obligation_type.clone(),
                                            priority: ob.priority,
                                            decomposition_id: None,
                                        },
                                    );
                                    if ob.priority >= 0.7 {
                                        if let Ok(full_ob) = config.state.db.get_obligation(&id) {
                                            high_priority_obs.push(full_ob);
                                        }
                                    }
                                }
                                Err(e) => tracing::warn!("Failed to create obligation: {}", e),
                            }
                        }

                        // Adversarial Critic for high-priority obligations
                        for obligation in &high_priority_obs {
                            let suspected =
                                state.suspected_answer.as_ref().map(|sa| sa.value.as_str());
                            let critic_prompt = critic::build_critic_prompt(
                                &config.problem.statement,
                                obligation,
                                &chain,
                                &config.enriched_analyst_context,
                                suspected,
                            );
                            emit_loop_thinking_start(
                                app_handle,
                                Some(step_number),
                                "critic",
                                Some("critic"),
                                Some(&obligation.id),
                                None,
                                None,
                            );
                            let critic_handle = app_handle.clone();
                            let critic_obligation_id = obligation.id.clone();
                            if let Ok(critic_resp) = config
                                .critic_llm
                                .complete_streaming(&critic_prompt, move |chunk| {
                                    emit_loop_token(
                                        &critic_handle,
                                        chunk,
                                        Some("critic"),
                                        Some(&critic_obligation_id),
                                    );
                                })
                                .await
                            {
                                if let Some(check) = critic::parse_critic(&critic_resp.text) {
                                    tracing::info!(
                                        "Critic check for '{}': {} (likely_wrong: {})",
                                        obligation.description,
                                        check.check_description,
                                        check.likely_wrong
                                    );
                                    let _ = app_handle.emit(
                                        "loop:critic_check",
                                        CriticCheckEvent {
                                            obligation_id: obligation.id.clone(),
                                            check_description: check.check_description.clone(),
                                            expected_if_correct: check.expected_if_correct.clone(),
                                            counterexample_hint: check.counterexample_hint.clone(),
                                            likely_wrong: check.likely_wrong,
                                        },
                                    );
                                    let _ = app_handle.emit(
                                        "agent:critic_evaluation",
                                        AgentCriticEvaluationPayload {
                                            obligation_id: obligation.id.clone(),
                                            check_description: check.check_description,
                                            likely_wrong: check.likely_wrong,
                                        },
                                    );
                                }
                            }
                            emit_loop_thinking_end(app_handle, Some(&obligation.id));
                        }
                    } else if parsed.should_branch
                        && !parsed.obligations.is_empty()
                        && obligations_capped
                    {
                        tracing::info!(
                            "Audit proposed {} obligations but skipped (obligation cap: {}/{} open). Breadth/direction still injected.",
                            parsed.obligations.len(), current_open, MAX_OPEN_OBLIGATIONS
                        );
                    }

                    // Branch Fork
                    if parsed.should_branch {
                        let decision = state.orchestrator.should_branch();
                        match decision {
                            orchestrator::BranchDecision::Branch { ref reason } => {
                                let direction = parsed.recommended_direction.clone();
                                match config.state.db.create_branch(
                                    &config.attempt_id,
                                    state.current_branch_id,
                                    Some(step_number as i32),
                                    Some(reason),
                                    Some(&direction),
                                ) {
                                    Ok(new_branch_id) => {
                                        let old_branch = state.current_branch_id;
                                        state.current_branch_id = new_branch_id as i32;
                                        tracing::info!(
                                            "BRANCHED: {} → {} at step {} (reason: {})",
                                            old_branch,
                                            state.current_branch_id,
                                            step_number,
                                            reason
                                        );

                                        let _ = config.state.db.append_dag_event(
                                            &config.attempt_id,
                                            "branch_created",
                                            &serde_json::json!({
                                                "branch_id": state.current_branch_id,
                                                "parent_branch": old_branch,
                                                "fork_step": step_number,
                                                "reason": reason,
                                                "direction": &direction,
                                            })
                                            .to_string(),
                                            "loop_engine",
                                        );

                                        let _ = app_handle.emit(
                                            "loop:branch_created",
                                            LoopBranchCreatedPayload {
                                                branch_id: state.current_branch_id as u32,
                                                parent_branch: old_branch as u32,
                                                fork_reason: reason.clone(),
                                                direction: direction.clone(),
                                            },
                                        );

                                        state.failures.clear();
                                        state.verified_since_audit = 0;
                                        state.orchestrator.reset_plateau_state();
                                    }
                                    Err(e) => tracing::warn!("Failed to create branch: {}", e),
                                }
                            }
                            _ => {
                                tracing::debug!(
                                    "Audit says should_branch but orchestrator says continue"
                                );
                            }
                        }
                    }

                    state.last_audit = Some(parsed);
                } else {
                    tracing::warn!("Failed to parse audit response");
                }
            }
        }

        // Solver Self-Assessment Diagnostic
        if let Some(closes) = proposal.closes_obligation {
            let ob_type = proposal.targets_obligation.as_deref().unwrap_or("unknown");
            let reason = proposal.closure_reason.as_deref().unwrap_or("");
            tracing::info!(
                "Solver self-assessment: targets={}, closes={}, reason='{}'",
                ob_type,
                closes,
                reason
            );
            let _ = app_handle.emit(
                "loop:solver_self_assessment",
                LoopSolverSelfAssessmentPayload {
                    step_number,
                    targets_obligation: ob_type.to_string(),
                    closes_obligation: closes,
                    closure_reason: reason.to_string(),
                },
            );
        }

        // === Satisfaction Tally System ===
        let open_obs = config
            .state
            .db
            .get_open_obligations(&config.attempt_id)
            .unwrap_or_default();
        if !open_obs.is_empty() {
            handle_satisfaction_tally(
                config,
                state,
                app_handle,
                &proposal,
                verified,
                all_obligations,
                &targeted_obligation_id,
                &current_node_id,
                &current_step_id,
                step_number,
                proposal_type,
            )
            .await;
        } else {
            state.orchestrator.record_closure_event(false);
        }
    } else {
        // Rejected step
        let reason_str = rejection_reason.as_deref().unwrap_or("validation_failed");
        tracing::info!(
            "Step {} REJECTED: {} — {}",
            step_number,
            proposal.natural,
            reason_str
        );
        state
            .failures
            .push((proposal.natural.clone(), reason_str.to_string()));
        state.failure_buffer.push(discerner::FailureEntry {
            step_number: Some(step_number),
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            failure_type: "validator_rejection".into(),
            category: "model".into(),
            reason: reason_str.to_string(),
            http_status: None,
            model: config.model_name.clone(),
            proposal_natural: Some(proposal.natural.clone()),
        });

        let _ = app_handle.emit(
            "loop:step_complete",
            StepEvent {
                attempt_id: config.attempt_id.clone(),
                step_number,
                proposal_type: proposal_type.to_string(),
                proposal_natural: proposal.natural.clone(),
                proposal_formal: proposal.formal.clone(),
                proposal_reasoning: proposal.reasoning.clone(),
                verified: false,
                rejection_reason: rejection_reason.clone(),
                model: config.model_name.clone(),
                sympy_passed,
                pint_passed,
                lean_passed,
                challenge_model: None,
                challenge_flaw_found: None,
                challenge_attack: None,
                challenge_confidence: None,
                challenge_fatal: None,
                obligation_id: targeted_obligation_id.clone(),
                obligation_desc: state
                    .selected_obligation
                    .as_ref()
                    .map(|s| s.obligation.description.clone()),
                obligation_type: state
                    .selected_obligation
                    .as_ref()
                    .map(|s| s.obligation.obligation_type.clone()),
                solver_round_id: None,
                solver_worker_id: None,
                solver_dispatch_mode: None,
                stale_sibling: None,
            },
        );

        // Contrastive pair
        if let Some(ref rejected_id) = current_step_id {
            if let Some(accepted_id) = config
                .state
                .db
                .get_verified_step_id_at(step_number, &config.attempt_id)
            {
                let _ = config.state.db.record_contrastive_pair(
                    &accepted_id,
                    rejected_id,
                    step_number,
                    &config.attempt_id,
                    rejection_reason.as_deref(),
                );
            }
        }

        if state
            .orchestrator
            .should_rotate(&config.model_name, config.failure_threshold)
        {
            let _ = config.state.db.record_orchestrator_decision(
                Some(&config.attempt_id),
                "model_rotation",
                &format!("rotate_from_{}", config.model_name),
                Some(&format!(
                    "{} consecutive failures",
                    config.failure_threshold
                )),
            );
            tracing::warn!(
                "Model {} hit failure threshold, would rotate",
                config.model_name
            );

            if state.current_branch_id != state.main_branch_id {
                tracing::info!(
                    "Abandoning branch {} due to failure threshold",
                    state.current_branch_id
                );
                let _ = config.state.db.close_branch(
                    state.current_branch_id as i64,
                    "abandoned",
                    None,
                    None,
                );
                let _ = config.state.db.append_dag_event(
                    &config.attempt_id,
                    "branch_closed",
                    &serde_json::json!({
                        "branch_id": state.current_branch_id,
                        "status": "abandoned",
                        "reason": "failure_threshold",
                    })
                    .to_string(),
                    "loop_engine",
                );
                let _ = app_handle.emit(
                    "loop:branch_closed",
                    LoopBranchClosedPayload {
                        branch_id: state.current_branch_id as u32,
                        status: "abandoned".to_string(),
                    },
                );

                let active = config
                    .state
                    .db
                    .get_active_branches(&config.attempt_id)
                    .unwrap_or_default();
                let old_branch = state.current_branch_id;
                if let Some(next) = active.first() {
                    state.current_branch_id = next.id as i32;
                } else {
                    state.current_branch_id = state.main_branch_id;
                }
                tracing::info!(
                    "Switched from branch {} to branch {}",
                    old_branch,
                    state.current_branch_id
                );
                let _ = app_handle.emit(
                    "loop:branch_switched",
                    LoopBranchSwitchedPayload {
                        from_branch: old_branch as u32,
                        to_branch: state.current_branch_id as u32,
                        reason: "abandonment".to_string(),
                    },
                );

                state.failures.clear();
                state.orchestrator.reset_plateau_state();
            }
        }
    }

    Ok(StepOutcome::Continue)
}

// ── run_step: orchestrates parallel obligation solving ───────────────

/// Execute a single iteration of the proof loop.
///
/// When open obligations exist, selects up to 3 and fires their LLM calls
/// in parallel via `tokio::task::JoinSet`. Results are processed serially.
/// Falls back to single freeform call when no obligations exist.
pub(super) async fn run_step(
    config: &StepConfig,
    state: &mut StepState,
    app_handle: &tauri::AppHandle,
) -> Result<StepOutcome, String> {
    let step_number = state.step_number;

    // Get current verified chain for the active branch
    let verified = config
        .state
        .db
        .get_branch_verified_chain(&config.attempt_id, state.current_branch_id as i64)
        .or_else(|_| config.state.db.get_verified_chain(&config.attempt_id))
        .unwrap_or_default();

    let goal_state = if verified.is_empty() {
        format!("PROBLEM: {}", config.problem.statement)
    } else {
        format!(
            "PROBLEM: {}\nContinue from step {}: {}",
            config.problem.statement,
            verified.last().map(|(_, n, _, _, _)| n).unwrap_or(&0),
            verified
                .last()
                .map(|(_, _, _, nat, _)| nat.as_str())
                .unwrap_or("")
        )
    };

    // Get patterns if enabled — keep IDs for success/failure tracking
    let patterns_raw = if config.use_patterns {
        config
            .state
            .db
            .search_patterns(&config.problem.domain)
            .unwrap_or_default()
            .into_iter()
            .take(5)
            .collect::<Vec<_>>()
    } else {
        vec![]
    };
    for p in &patterns_raw {
        state.all_injected_pattern_ids.insert(p.id.clone());
    }
    let patterns: Vec<(String, String, String)> = patterns_raw
        .into_iter()
        .map(|p| (p.name, p.trigger, p.strategy))
        .collect();

    // Load ALL obligations (open + closed) so models see the full picture
    let all_obligations = config
        .state
        .db
        .get_all_obligations(&config.attempt_id)
        .unwrap_or_default();
    let open_obligations: Vec<_> = all_obligations
        .iter()
        .filter(|o| o.status == "open" || o.status == "assigned")
        .cloned()
        .collect();

    // === Dequeue pre-parsed batch proposals ===
    // If the previous solver call returned multiple steps (JSON array), we
    // queued the extras. Process them without a new LLM call.
    if !state.pending_proposals.is_empty() {
        let queued = state.pending_proposals.remove(0);
        let remaining = state.pending_proposals.len();
        tracing::info!(
            "Dequeuing batch proposal ({} remaining): {}",
            remaining,
            queued.natural.chars().take(80).collect::<String>()
        );
        let text = serde_json::to_string(&queued).unwrap_or_default();
        let goal_state_clone = goal_state.clone();
        return process_solver_result(
            config,
            state,
            app_handle,
            SolverCallResult {
                obligation: None,
                step_number: state.step_number,
                prompt: String::new(),
                goal_state: goal_state_clone,
                context_refs_json: None,
                response: Ok(crate::api::llm_client::LlmResponse {
                    text,
                    tokens_in: None,
                    tokens_out: None,
                }),
                worker_id: String::new(),
                worker_model_name: config.model_name.clone(),
                solver_round_id: None,
                dispatch_mode: "batch_dequeue".to_string(),
                tool_run_ids: vec![],
            },
            &verified,
            &open_obligations,
            &all_obligations,
            &patterns,
        )
        .await;
    }

    // === Mid-Run Discerner Trigger ===
    if state
        .failure_buffer
        .should_trigger(DISCERNER_TRIGGER_STREAK)
    {
        if let Some(ref dis_llm) = config.discerner_llm {
            tracing::info!(
                "Discerner triggered (streak={})",
                state.failure_buffer.streak()
            );
            emit_diagnostic(
                app_handle,
                "info",
                "info",
                "discerner",
                Some(step_number),
                &format!(
                    "Discerner: classifying {} consecutive failures",
                    state.failure_buffer.streak()
                ),
                serde_json::json!({"streak": state.failure_buffer.streak()}),
                &config.attempt_id,
            );

            match discerner::classify_mid_run(
                &state.failure_buffer,
                &config.model_name,
                &config.problem.domain,
                step_number,
                &config.attempt_id,
                dis_llm,
                app_handle,
            )
            .await
            {
                Ok(verdict) => {
                    let finding = crate::db::discerner::DiscernerFinding {
                        id: uuid::Uuid::new_v4().to_string(),
                        attempt_id: config.attempt_id.clone(),
                        step_number,
                        failure_streak: state.failure_buffer.streak(),
                        failure_window: serde_json::to_string(state.failure_buffer.entries())
                            .unwrap_or_default(),
                        classification: verdict.classification.clone(),
                        root_cause: verdict.root_cause.clone(),
                        recommendation: verdict.recommendation.clone(),
                        confidence: verdict.confidence,
                        suggested_action: verdict.suggested_action.clone(),
                        discerner_model: dis_llm.model_name(),
                        created_at: chrono::Utc::now()
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    };
                    let _ = config.state.db.record_discerner_finding(&finding);

                    let _ = app_handle.emit(
                        "loop:discerner_finding",
                        LoopDiscernerFinding {
                            attempt_id: config.attempt_id.clone(),
                            step_number,
                            failure_streak: state.failure_buffer.streak(),
                            classification: map_discerner_classification(&verdict.classification),
                            root_cause: verdict.root_cause.clone(),
                            recommendation: verdict.recommendation.clone(),
                            confidence: verdict.confidence,
                            suggested_action: map_discerner_suggested_action(
                                &verdict.suggested_action,
                            ),
                            discerner_model: dis_llm.model_name(),
                        },
                    );

                    let _ = config.state.db.append_dag_event(
                        &config.attempt_id,
                        "discerner_fired",
                        &serde_json::json!({
                            "classification": &verdict.classification,
                            "suggested_action": &verdict.suggested_action,
                            "confidence": verdict.confidence,
                            "streak": state.failure_buffer.streak(),
                        })
                        .to_string(),
                        "discerner",
                    );

                    match verdict.suggested_action.as_str() {
                        "add_backoff" => {
                            tracing::info!(
                                "Discerner: add_backoff — waiting 5s before next LLM call"
                            );
                            emit_diagnostic(
                                app_handle,
                                "mechanical",
                                "info",
                                "discerner",
                                Some(step_number),
                                "Discerner: rate limit detected — backing off 5s",
                                serde_json::json!({"action": "add_backoff", "cause": &verdict.root_cause}),
                                &config.attempt_id,
                            );
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            state.failure_buffer.reset();
                        }
                        "retry" => {
                            tracing::info!("Discerner: retry immediately (transient error)");
                            state.failure_buffer.reset();
                        }
                        "switch_model" => {
                            tracing::warn!(
                                "Discerner: switch_model recommended — recording for orchestrator"
                            );
                            emit_diagnostic(
                                app_handle,
                                "mechanical",
                                "warn",
                                "discerner",
                                Some(step_number),
                                &format!(
                                    "Discerner: recommend model switch — {}",
                                    verdict.recommendation
                                ),
                                serde_json::json!({"action": "switch_model", "cause": &verdict.root_cause}),
                                &config.attempt_id,
                            );
                            let _ = config.state.db.record_orchestrator_decision(
                                Some(&config.attempt_id),
                                "discerner_switch_model",
                                "switch_model",
                                Some(&verdict.recommendation),
                            );
                        }
                        "rephrase_prompt" => {
                            tracing::info!("Discerner: rephrase_prompt — injecting diagnostic hint into solver");
                            state.failures.push((
                                "discerner_hint".into(),
                                format!(
                                    "DIAGNOSTIC: {} — {}",
                                    verdict.classification, verdict.recommendation
                                ),
                            ));
                            state.failure_buffer.reset();
                        }
                        _ => {
                            tracing::debug!("Discerner: continue (classified as noise)");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Mid-run Discerner failed: {}", e);
                    emit_diagnostic(
                        app_handle,
                        "mechanical",
                        "warn",
                        "discerner",
                        Some(step_number),
                        &format!("Discerner error: {}", e),
                        serde_json::json!({"error": &e}),
                        &config.attempt_id,
                    );
                }
            }
        }
    }

    // === Obligation Selection (growing pool — add workers, never remove active) ===
    // 1. Prune sticky obligations that are no longer open/assigned in DB
    if !state.sticky_obligations.is_empty() {
        let fresh_open = config
            .state
            .db
            .get_open_obligations(&config.attempt_id)
            .unwrap_or_default();
        let fresh_by_id: std::collections::HashMap<String, crate::models::dag::Obligation> =
            fresh_open
                .into_iter()
                .map(|obligation| (obligation.id.clone(), obligation))
                .collect();
        state.sticky_obligations = state
            .sticky_obligations
            .iter()
            .filter_map(|sticky| {
                fresh_by_id
                    .get(&sticky.obligation.id)
                    .cloned()
                    .map(|obligation| obligation_queue::SelectedObligation {
                        obligation,
                        blacklisted_approaches: state
                            .pivot_tracker
                            .get_blacklist(&sticky.obligation.id),
                    })
            })
            .collect();
        if let Some(ref focus_id) = state.fanin_focus_obligation_id {
            if !state
                .sticky_obligations
                .iter()
                .any(|sticky| sticky.obligation.id == *focus_id)
            {
                state.fanin_focus_obligation_id = None;
            }
        }
    }

    // 2. Check for NEW high-priority obligations not yet in the pool — add them
    {
        let current_ids: std::collections::HashSet<String> = state
            .sticky_obligations
            .iter()
            .map(|s| s.obligation.id.clone())
            .collect();
        // Ask the queue for top unblocked obligations (up to 10 — no artificial cap)
        let candidates = obligation_queue::select_multiple(
            &config.state.db,
            &config.attempt_id,
            state.current_branch_id,
            &state.pivot_tracker,
            10,
        );
        for candidate in candidates {
            if !current_ids.contains(&candidate.obligation.id) {
                tracing::info!(
                    "Growing worker pool: adding obligation '{}' (priority={:.2})",
                    candidate.obligation.description,
                    candidate.obligation.priority,
                );
                state.sticky_obligations.push(candidate);
            }
        }
    }

    // 3. If pool is still empty (first round), do initial selection
    if state.sticky_obligations.is_empty() {
        let fresh = obligation_queue::select_multiple(
            &config.state.db,
            &config.attempt_id,
            state.current_branch_id,
            &state.pivot_tracker,
            10,
        );
        state.sticky_obligations = fresh;
    }

    // 4. Build final selection with refreshed blacklists
    let selected = pick_selected_obligations(
        &state
            .sticky_obligations
            .iter()
            .map(|s| obligation_queue::SelectedObligation {
                obligation: s.obligation.clone(),
                blacklisted_approaches: state.pivot_tracker.get_blacklist(&s.obligation.id),
            })
            .collect::<Vec<_>>(),
        state.fanin_focus_obligation_id.as_deref(),
        config.same_obligation_fanin_enabled,
        config.solver_workers.len(),
        config.max_fanin_workers,
    );

    if state.fanin_focus_obligation_id.is_none() && selected.len() == 1 {
        state.fanin_focus_obligation_id = selected.first().map(|sel| sel.obligation.id.clone());
    }

    if selected.is_empty() {
        let _ = config
            .state
            .db
            .unassign_obligations_except(&config.attempt_id, &std::collections::HashSet::new());
        // === FREEFORM MODE (no obligations) — single call path ===
        let prompt = solver::build_solver_prompt(
            &config.problem.statement,
            &verified,
            &state.failures,
            &patterns,
            state.last_audit.as_ref(),
            &config.prior_findings,
            &open_obligations,
            &all_obligations,
            &config.attempt_constraints,
            &config.techniques,
            &config.enriched_solver_context,
            state.failures.len() as u32,
            state.suspected_answer.as_ref(),
        );
        let context_refs_json = if verified.is_empty() {
            None
        } else {
            let ids: Vec<&str> = verified
                .iter()
                .map(|(id, _, _, _, _)| id.as_str())
                .collect();
            Some(serde_json::to_string(&ids).unwrap_or_default())
        };

        let (response, solver_tool_run_ids) = call_solver(
            config.llm.clone(),
            prompt.clone(),
            state.step_number,
            app_handle.clone(),
            config.model_name.clone(),
            config.attempt_id.clone(),
            None,
            config.state.clone(),
        )
        .await;
        return process_solver_result(
            config,
            state,
            app_handle,
            SolverCallResult {
                obligation: None,
                step_number: state.step_number,
                prompt,
                goal_state,
                context_refs_json,
                response,
                worker_id: config
                    .solver_workers
                    .first()
                    .map(|w| w.worker_id.clone())
                    .unwrap_or_default(),
                worker_model_name: config.model_name.clone(),
                solver_round_id: None,
                dispatch_mode: "freeform".to_string(),
                tool_run_ids: solver_tool_run_ids,
            },
            &verified,
            &open_obligations,
            &all_obligations,
            &patterns,
        )
        .await;
    }

    // === SAME-OBLIGATION FAN-IN (1 obligation, multiple workers) ===
    if selected.len() == 1
        && config.same_obligation_fanin_enabled
        && config.solver_workers.len() >= 2
    {
        return run_parallel_fanin(
            config,
            state,
            app_handle,
            selected
                .into_iter()
                .next()
                .expect("selected.len() == 1 checked above"),
            &verified,
            &open_obligations,
            &all_obligations,
            &patterns,
            goal_state,
            None,
        )
        .await;
    }

    // === PARALLEL DISTINCT MODE (one worker per obligation, all concurrent) ===
    // Each obligation gets its own dedicated parallel solver call. Workers persist
    // across rounds via sticky assignments. When a new obligation becomes priority,
    // it gets ADDED to the pool — existing workers are never stopped.
    return run_parallel_distinct(
        config,
        state,
        app_handle,
        selected,
        &verified,
        &open_obligations,
        &all_obligations,
        &patterns,
        goal_state,
    )
    .await;
}

// ── Parallel distinct dispatch (one worker per obligation) ──────────

/// Per-worker LLM call timeout (5 minutes).
const WORKER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Dispatch one solver call per obligation in parallel via JoinSet.
/// Each obligation gets dedicated attention every round. The pool grows
/// as new obligations become priority — existing obligations keep their
/// workers until closed.
///
/// Safeguards:
/// - Capped at MAX_PARALLEL_DISTINCT_WORKERS concurrent workers
/// - Per-worker timeout (WORKER_TIMEOUT)
/// - Stop signal checked before processing results
/// - Stale obligation check before each result processing
#[allow(clippy::too_many_arguments)]
async fn run_parallel_distinct(
    config: &StepConfig,
    state: &mut StepState,
    app_handle: &tauri::AppHandle,
    selected: Vec<obligation_queue::SelectedObligation>,
    verified: &[(String, u32, String, String, String)],
    open_obligations: &[crate::models::dag::Obligation],
    all_obligations: &[crate::models::dag::Obligation],
    patterns: &[(String, String, String)],
    goal_state: String,
) -> Result<StepOutcome, String> {
    let base_step = state.step_number;
    let keep_ids: std::collections::HashSet<String> = selected
        .iter()
        .map(|sel| sel.obligation.id.clone())
        .collect();
    let _ = config
        .state
        .db
        .unassign_obligations_except(&config.attempt_id, &keep_ids);
    let worker_count = selected.len();

    tracing::info!(
        "Parallel distinct: dispatching {} workers for {} obligations",
        worker_count,
        worker_count
    );

    // Pre-solve research + assign each obligation
    for (i, sel) in selected.iter().enumerate() {
        // Pre-solve research if needed
        if !config.scout_sources.is_empty() {
            let bl_count = state.pivot_tracker.blacklisted_count(&sel.obligation.id);
            let last_scout_bl = state
                .obligation_scout_bl_at
                .get(&sel.obligation.id)
                .copied()
                .unwrap_or(0);
            let needs_scout = last_scout_bl == 0 || bl_count > last_scout_bl;
            if needs_scout {
                let sidecar_scout = crate::api::sidecar::SidecarClient::new();
                let failed_techniques = state
                    .pivot_tracker
                    .blacklisted_techniques(&sel.obligation.id);
                let scout_query = if failed_techniques.is_empty() {
                    format!(
                        "Techniques and theorems for: {}. Problem context: {}",
                        sel.obligation.description,
                        config
                            .problem
                            .statement
                            .chars()
                            .take(200)
                            .collect::<String>(),
                    )
                } else {
                    format!(
                        "How to prove: {}. Failed approaches: {}",
                        sel.obligation.description,
                        failed_techniques.join(", "),
                    )
                };
                match sidecar_scout
                    .scout_briefing_mid_solve(
                        &scout_query,
                        Some(&config.problem.domain),
                        &config.scout_sources,
                    )
                    .await
                {
                    Ok(resp) if !resp.briefing.is_empty() => {
                        state
                            .obligation_scout_results
                            .insert(sel.obligation.id.clone(), resp.briefing);
                        state.obligation_scouted.insert(sel.obligation.id.clone());
                        state
                            .obligation_scout_bl_at
                            .insert(sel.obligation.id.clone(), bl_count);
                    }
                    Ok(_) => {
                        state.obligation_scouted.insert(sel.obligation.id.clone());
                        state
                            .obligation_scout_bl_at
                            .insert(sel.obligation.id.clone(), bl_count);
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Pre-solve research failed for '{}': {}",
                            sel.obligation.description,
                            e
                        );
                    }
                }
            }
        }

        // Pick worker model: cycle through solver_workers, fallback to primary
        let worker_model = if !config.solver_workers.is_empty() {
            &config.solver_workers[i % config.solver_workers.len()].model_name
        } else {
            &config.model_name
        };

        // Assign obligation in DB + emit event
        let _ = config
            .state
            .db
            .assign_obligation(&sel.obligation.id, worker_model);
        let _ = app_handle.emit(
            "loop:obligation_assigned",
            LoopObligationAssignedPayload {
                attempt_id: Some(config.attempt_id.clone()),
                obligation_id: sel.obligation.id.clone(),
                obligation_desc: Some(sel.obligation.description.clone()),
                obligation_type: Some(sel.obligation.obligation_type.clone()),
                priority: Some(sel.obligation.priority),
                assigned_model: worker_model.to_string(),
                steps_spent: sel.obligation.steps_spent.max(0) as u32,
                max_steps: sel.obligation.max_steps.max(0) as u32,
                dispatch_mode: Some("parallel_distinct".to_string()),
                worker_count: Some(worker_count as u32),
                blacklisted_approaches: Some(sel.blacklisted_approaches.len() as u32),
            },
        );
        tracing::info!(
            "  Worker {}: '{}' (type={}, priority={:.2}) → {}",
            i,
            sel.obligation.description,
            sel.obligation.obligation_type,
            sel.obligation.priority,
            worker_model,
        );
    }

    // Build prompts and spawn parallel solver calls with timeout
    let mut join_set = tokio::task::JoinSet::new();
    for (i, sel) in selected.iter().enumerate() {
        let step_num = base_step + i as u32;

        let obligation_history = config
            .state
            .db
            .get_nodes_for_obligation(&sel.obligation.id)
            .unwrap_or_default();
        let satisfaction_signals = config
            .state
            .db
            .get_obligation_signals(&sel.obligation.id)
            .unwrap_or_default();
        let ob_stuck_steps = sel.obligation.steps_spent.max(0) as u32;
        let ob_scout_ctx = state
            .obligation_scout_results
            .get(&sel.obligation.id)
            .cloned()
            .unwrap_or_default();

        let prompt = solver::build_obligation_solver_prompt(
            &config.problem.statement,
            &sel.obligation,
            &sel.blacklisted_approaches,
            verified,
            &state.failures,
            &config.attempt_constraints,
            &config.techniques,
            &config.enriched_solver_context,
            &obligation_history,
            &satisfaction_signals,
            all_obligations,
            ob_stuck_steps,
            &ob_scout_ctx,
            state.suspected_answer.as_ref(),
        );
        let context_refs_json = if verified.is_empty() {
            None
        } else {
            let ids: Vec<&str> = verified
                .iter()
                .map(|(id, _, _, _, _)| id.as_str())
                .collect();
            Some(serde_json::to_string(&ids).unwrap_or_default())
        };

        // Pick worker LLM: cycle through solver_workers, fallback to primary
        let (llm, model_name, worker_id) = if !config.solver_workers.is_empty() {
            let w = &config.solver_workers[i % config.solver_workers.len()];
            (w.llm.clone(), w.model_name.clone(), w.worker_id.clone())
        } else {
            (
                config.llm.clone(),
                config.model_name.clone(),
                format!("worker-{}", i),
            )
        };

        let app = app_handle.clone();
        let attempt_id = config.attempt_id.clone();
        let ob_id = sel.obligation.id.clone();
        let db = config.state.clone();
        let sel_clone = sel.clone();
        let gs = goal_state.clone();
        let ob_desc = sel.obligation.description.clone();

        join_set.spawn(async move {
            // Per-worker timeout: prevents hung LLM calls from blocking forever
            match tokio::time::timeout(
                WORKER_TIMEOUT,
                call_solver(
                    llm,
                    prompt.clone(),
                    step_num,
                    app,
                    model_name.clone(),
                    attempt_id,
                    Some(ob_id),
                    db,
                ),
            )
            .await
            {
                Ok((response, solver_tool_run_ids)) => SolverCallResult {
                    obligation: Some(sel_clone),
                    step_number: step_num,
                    prompt,
                    goal_state: gs,
                    context_refs_json,
                    response,
                    worker_id,
                    worker_model_name: model_name,
                    solver_round_id: None,
                    dispatch_mode: "parallel_distinct".to_string(),
                    tool_run_ids: solver_tool_run_ids,
                },
                Err(_elapsed) => {
                    tracing::error!(
                        "Worker {} timed out after {:?} on obligation '{}'",
                        worker_id,
                        WORKER_TIMEOUT,
                        ob_desc,
                    );
                    SolverCallResult {
                        obligation: Some(sel_clone),
                        step_number: step_num,
                        prompt,
                        goal_state: gs,
                        context_refs_json,
                        response: Err(SolverError::Retryable(format!(
                            "Worker timed out after {:?}",
                            WORKER_TIMEOUT,
                        ))),
                        worker_id,
                        worker_model_name: model_name,
                        solver_round_id: None,
                        dispatch_mode: "parallel_distinct".to_string(),
                        tool_run_ids: vec![],
                    }
                }
            }
        });
    }

    // Collect results, sort by step_number for deterministic processing
    let mut results: Vec<SolverCallResult> = Vec::new();
    while let Some(r) = join_set.join_next().await {
        match r {
            Ok(result) => results.push(result),
            Err(e) => tracing::error!("Parallel distinct JoinSet task panicked: {}", e),
        }
    }
    results.sort_by_key(|r| r.step_number);

    tracing::info!(
        "Parallel distinct: {} results collected, processing serially",
        results.len()
    );

    // SAFEGUARD: Check stop signal before processing any results
    // If user stopped while workers were running, skip processing entirely
    {
        let running = config.state.loop_running.lock().await;
        if !*running {
            tracing::info!(
                "Parallel distinct: stop signal received — discarding {} worker results",
                results.len()
            );
            // Advance step_number past reserved slots to avoid reuse
            if state.step_number < base_step + worker_count as u32 {
                state.step_number = base_step + worker_count as u32;
            }
            return Ok(StepOutcome::Break(BreakReason::MaxConsecutiveFailures));
        }
    }

    // Process each result serially
    let mut processed = 0u32;
    let mut skipped_stale = 0u32;
    let mut skipped_stopped = 0u32;
    for result in results {
        // SAFEGUARD: Re-check stop signal between processing each result
        {
            let running = config.state.loop_running.lock().await;
            if !*running {
                tracing::info!(
                    "Parallel distinct: stop signal mid-processing — {} processed, {} remaining skipped",
                    processed,
                    worker_count as u32 - processed - skipped_stale,
                );
                skipped_stopped += 1;
                continue;
            }
        }

        // SAFEGUARD: Skip if this obligation was already closed
        if let Some(ref sel) = result.obligation {
            if is_obligation_stale(&config.state.db, &sel.obligation.id) {
                tracing::info!(
                    "Parallel distinct: skipping step {} — obligation '{}' already closed",
                    result.step_number,
                    sel.obligation.description,
                );
                skipped_stale += 1;
                continue;
            }
        }

        match process_solver_result(
            config,
            state,
            app_handle,
            result,
            verified,
            open_obligations,
            all_obligations,
            patterns,
        )
        .await?
        {
            StepOutcome::Continue => {
                processed += 1;
            }
            outcome => {
                tracing::info!(
                    "Parallel distinct: early exit after {} processed, {} stale, {} stopped",
                    processed,
                    skipped_stale,
                    skipped_stopped,
                );
                return Ok(outcome);
            }
        }
    }

    tracing::info!(
        "Parallel distinct round complete: {} processed, {} stale-skipped, {} stop-skipped",
        processed,
        skipped_stale,
        skipped_stopped,
    );

    // Advance step_number past all reserved slots
    if state.step_number < base_step + worker_count as u32 {
        state.step_number = base_step + worker_count as u32;
    }

    Ok(StepOutcome::Continue)
}

// ── Same-obligation fan-in (Phase 3+4) ────────────────────────────────

/// Dispatch multiple solver workers against a single obligation in parallel.
/// After the first worker closes the obligation, remaining results are marked
/// as stale siblings and skipped.
#[allow(clippy::too_many_arguments)]
async fn run_parallel_fanin(
    config: &StepConfig,
    state: &mut StepState,
    app_handle: &tauri::AppHandle,
    selected: obligation_queue::SelectedObligation,
    verified: &[(String, u32, String, String, String)],
    open_obligations: &[crate::models::dag::Obligation],
    all_obligations: &[crate::models::dag::Obligation],
    patterns: &[(String, String, String)],
    goal_state: String,
    _context_refs_json: Option<String>,
) -> Result<StepOutcome, String> {
    use std::sync::atomic::{AtomicBool, Ordering};

    let solver_round_id = uuid::Uuid::new_v4().to_string();
    let worker_count = (config.solver_workers.len() as u32).min(config.max_fanin_workers) as usize;
    let base_step = state.step_number;
    let ob_id = selected.obligation.id.clone();
    let keep_ids = std::collections::HashSet::from([ob_id.clone()]);
    let _ = config
        .state
        .db
        .unassign_obligations_except(&config.attempt_id, &keep_ids);

    // Assign collaborative
    let models: Vec<&str> = config
        .solver_workers
        .iter()
        .take(worker_count)
        .map(|w| w.model_name.as_str())
        .collect();
    let models_json = serde_json::to_string(&models).unwrap_or_default();
    let _ = config.state.db.assign_obligation_collaborative(
        &ob_id,
        &config.solver_workers[0].model_name,
        &models_json,
        &solver_round_id,
    );

    // Emit round start
    let reserved_steps: Vec<u32> = (0..worker_count as u32).map(|i| base_step + i).collect();
    let _ = app_handle.emit(
        "loop:fanin_round_start",
        LoopFaninRoundStartPayload {
            attempt_id: config.attempt_id.clone(),
            solver_round_id: solver_round_id.clone(),
            obligation_id: ob_id.clone(),
            obligation_desc: selected.obligation.description.clone(),
            worker_count: worker_count as u32,
            worker_models: models.iter().map(|model| (*model).to_string()).collect(),
            reserved_step_numbers: reserved_steps.clone(),
        },
    );
    let _ = app_handle.emit(
        "loop:obligation_assigned",
        LoopObligationAssignedPayload {
            attempt_id: Some(config.attempt_id.clone()),
            obligation_id: ob_id.clone(),
            obligation_desc: Some(selected.obligation.description.clone()),
            obligation_type: Some(selected.obligation.obligation_type.clone()),
            priority: Some(selected.obligation.priority),
            assigned_model: config.solver_workers[0].model_name.clone(),
            steps_spent: selected.obligation.steps_spent.max(0) as u32,
            max_steps: selected.obligation.max_steps.max(0) as u32,
            dispatch_mode: Some("parallel_fanin".to_string()),
            worker_count: Some(worker_count as u32),
            blacklisted_approaches: None,
        },
    );

    tracing::info!(
        "Fan-in round {}: {} workers attacking '{}' (type={}, priority={:.2}), steps {}-{}",
        &solver_round_id[..8],
        worker_count,
        selected.obligation.description,
        selected.obligation.obligation_type,
        selected.obligation.priority,
        base_step,
        base_step + worker_count as u32 - 1,
    );

    // Build prompt once (shared by all workers)
    let obligation_history = config
        .state
        .db
        .get_nodes_for_obligation(&ob_id)
        .unwrap_or_default();
    let satisfaction_signals = config
        .state
        .db
        .get_obligation_signals(&ob_id)
        .unwrap_or_default();
    let ob_stuck_steps = selected.obligation.steps_spent.max(0) as u32;
    let ob_scout_ctx = state
        .obligation_scout_results
        .get(&ob_id)
        .map(|s| s.as_str())
        .unwrap_or("");
    let prompt = solver::build_obligation_solver_prompt(
        &config.problem.statement,
        &selected.obligation,
        &selected.blacklisted_approaches,
        verified,
        &state.failures,
        &config.attempt_constraints,
        &config.techniques,
        &config.enriched_solver_context,
        &obligation_history,
        &satisfaction_signals,
        all_obligations,
        ob_stuck_steps,
        ob_scout_ctx,
        state.suspected_answer.as_ref(),
    );
    let context_refs_json = if verified.is_empty() {
        None
    } else {
        let ids: Vec<&str> = verified
            .iter()
            .map(|(id, _, _, _, _)| id.as_str())
            .collect();
        Some(serde_json::to_string(&ids).unwrap_or_default())
    };

    // Shared abort flag — set when obligation is closed by any worker
    let round_abort = Arc::new(AtomicBool::new(false));

    // Spawn one task per worker
    let mut join_set = tokio::task::JoinSet::new();
    for (i, worker) in config.solver_workers.iter().take(worker_count).enumerate() {
        let step_num = base_step + i as u32;
        let llm = worker.llm.clone();
        let app = app_handle.clone();
        let model_name = worker.model_name.clone();
        let attempt_id = config.attempt_id.clone();
        let ob_id_clone = ob_id.clone();
        let db = config.state.clone();
        let prompt_clone = prompt.clone();
        let gs = goal_state.clone();
        let crj = context_refs_json.clone();
        let wid = worker.worker_id.clone();
        let srid = solver_round_id.clone();
        let sel_clone = selected.clone();

        join_set.spawn(async move {
            let (response, solver_tool_run_ids) = call_solver(
                llm,
                prompt_clone.clone(),
                step_num,
                app,
                model_name.clone(),
                attempt_id,
                Some(ob_id_clone),
                db,
            )
            .await;
            SolverCallResult {
                obligation: Some(sel_clone),
                step_number: step_num,
                prompt: prompt_clone,
                goal_state: gs,
                context_refs_json: crj,
                response,
                worker_id: wid,
                worker_model_name: model_name,
                solver_round_id: Some(srid),
                dispatch_mode: "parallel_fanin".to_string(),
                tool_run_ids: solver_tool_run_ids,
            }
        });
    }

    // Collect results, sort by step_number
    let mut results: Vec<SolverCallResult> = Vec::new();
    while let Some(r) = join_set.join_next().await {
        match r {
            Ok(result) => results.push(result),
            Err(e) => tracing::error!("Fan-in JoinSet task panicked: {}", e),
        }
    }
    results.sort_by_key(|r| r.step_number);

    tracing::info!(
        "Fan-in round {} complete: {} results collected",
        &solver_round_id[..8],
        results.len()
    );

    // Process serially with stale-sibling guard
    let mut processed = 0u32;
    let mut skipped_stale = 0u32;
    for result in results {
        // Stale-sibling check: re-query obligation status before processing
        if round_abort.load(Ordering::SeqCst) {
            // Obligation already closed — mark this result as stale
            tracing::info!(
                "Fan-in: worker {} result (step {}) is stale — obligation {} already closed",
                result.worker_id,
                result.step_number,
                ob_id
            );
            record_stale_sibling_step(
                config,
                state,
                app_handle,
                &result,
                "Stale sibling: target obligation closed by earlier worker",
            );
            skipped_stale += 1;
            let _ = app_handle.emit(
                "loop:fanin_round_update",
                LoopFaninRoundUpdatePayload {
                    solver_round_id: solver_round_id.clone(),
                    status: Some("stale_skip".to_string()),
                    action: Some("skip_remaining".to_string()),
                    reason: Some(format!(
                        "Worker {} skipped — obligation resolved by earlier worker",
                        result.worker_id
                    )),
                    worker_id: Some(result.worker_id.clone()),
                    step_number: Some(result.step_number),
                },
            );
            continue;
        }

        // Also check DB directly for freshness (handles external closures)
        if is_obligation_stale(&config.state.db, &ob_id) {
            round_abort.store(true, Ordering::SeqCst);
            record_stale_sibling_step(
                config,
                state,
                app_handle,
                &result,
                "Stale sibling: obligation closed externally",
            );
            skipped_stale += 1;
            continue;
        }

        match process_solver_result(
            config,
            state,
            app_handle,
            result,
            verified,
            open_obligations,
            all_obligations,
            patterns,
        )
        .await?
        {
            StepOutcome::Continue => {
                processed += 1;
                // Check if the obligation was just closed by this result
                if is_obligation_stale(&config.state.db, &ob_id) {
                    round_abort.store(true, Ordering::SeqCst);
                }
            }
            outcome => {
                // Fatal/break — clean up and propagate
                let _ = config.state.db.clear_obligation_active_round(&ob_id);
                let _ = app_handle.emit(
                    "loop:fanin_round_complete",
                    LoopFaninRoundCompletePayload {
                        solver_round_id: solver_round_id.clone(),
                        workers_dispatched: Some(worker_count as u32),
                        results_processed: processed,
                        results_skipped_stale: Some(skipped_stale),
                        early_exit: Some(true),
                    },
                );
                return Ok(outcome);
            }
        }
    }

    // Clean up round metadata
    let _ = config.state.db.clear_obligation_active_round(&ob_id);

    let _ = app_handle.emit(
        "loop:fanin_round_complete",
        LoopFaninRoundCompletePayload {
            solver_round_id: solver_round_id.clone(),
            workers_dispatched: Some(worker_count as u32),
            results_processed: processed,
            results_skipped_stale: Some(skipped_stale),
            early_exit: None,
        },
    );

    // Advance step_number past all reserved slots
    if state.step_number < base_step + worker_count as u32 {
        state.step_number = base_step + worker_count as u32;
    }

    Ok(StepOutcome::Continue)
}

/// Check if an obligation has already been closed (not open/assigned).
fn is_obligation_stale(db: &crate::db::Database, obligation_id: &str) -> bool {
    match db.get_obligation(obligation_id) {
        Ok(ob) => !matches!(ob.status.as_str(), "open" | "assigned"),
        Err(_) => true,
    }
}

/// Record a step as rejected due to stale-sibling condition (obligation closed by earlier worker).
fn record_stale_sibling_step(
    config: &StepConfig,
    _state: &mut StepState,
    app_handle: &tauri::AppHandle,
    result: &SolverCallResult,
    reason: &str,
) {
    use crate::db::StepRecord;
    let rec = StepRecord {
        attempt_id: &config.attempt_id,
        parent_step_id: None,
        step_number: result.step_number,
        model: &result.worker_model_name,
        context_refs: result.context_refs_json.as_deref(),
        goal_state: &result.goal_state,
        context_provided: Some(&result.prompt),
        proposal_type: "stale_sibling",
        proposal_natural: reason,
        proposal_formal: None,
        proposal_reasoning: None,
        sympy_result: None,
        sympy_passed: None,
        pint_result: None,
        pint_passed: None,
        lean_result: None,
        lean_passed: None,
        verified: false,
        rejection_reason: Some(reason),
        model_tokens_in: None,
        model_tokens_out: None,
        wall_time_ms: None,
        challenge_model: None,
        challenge_flaw_found: None,
        challenge_attack: None,
        challenge_confidence: None,
        challenge_fatal: None,
        obligation_id: result.obligation.as_ref().map(|o| o.obligation.id.as_str()),
        solver_round_id: result.solver_round_id.as_deref(),
        solver_worker_id: Some(&result.worker_id),
        solver_dispatch_mode: Some("parallel_fanin"),
        stale_sibling: true,
    };
    let stale_step_id = db_write_or_log(
        config.state.db.record_step(&rec),
        "record_step(stale)",
        app_handle,
        &config.attempt_id,
    );

    // Backfill step_id on tool_runs
    if !stale_step_id.is_empty() && !result.tool_run_ids.is_empty() {
        let _ = config
            .state
            .db
            .backfill_tool_runs_step_id(&result.tool_run_ids, &stale_step_id);
    }

    // Emit step_complete event for the stale sibling
    let _ = app_handle.emit(
        "loop:step_complete",
        serde_json::json!({
            "step_number": result.step_number,
            "attempt_id": &config.attempt_id,
            "model": &result.worker_model_name,
            "proposal_type": "stale_sibling",
            "proposal_natural": reason,
            "verified": false,
            "rejection_reason": reason,
            "solver_round_id": &result.solver_round_id,
            "solver_worker_id": &result.worker_id,
            "solver_dispatch_mode": "parallel_fanin",
            "stale_sibling": true,
            "obligation_id": result.obligation.as_ref().map(|o| &o.obligation.id),
            "obligation_desc": result.obligation.as_ref().map(|o| &o.obligation.description),
        }),
    );
}

// ── Conclusion handling (extracted for size) ─────────────────────────

/// Handle the conclusion path when is_conclusion && verified.len() >= 3.
/// This includes evidence-obligation reconciliation, obligation gate,
/// known answer gate, claim extraction gate, conclusion review gate,
/// and final acceptance with branch closure.
#[allow(clippy::too_many_arguments)]
async fn handle_conclusion(
    config: &StepConfig,
    state: &mut StepState,
    app_handle: &tauri::AppHandle,
    proposal: &LlmProposal,
    verified: &[(String, u32, String, String, String)],
    _open_obligations: &[crate::models::dag::Obligation],
    _all_obligations: &[crate::models::dag::Obligation],
    goal_state: &str,
    context_refs_json: &Option<String>,
    llm_result: &crate::api::llm_client::LlmResponse,
    targeted_obligation_id: &Option<String>,
    step_number: u32,
    tool_run_ids: &[String],
) -> Result<StepOutcome, String> {
    let formal = proposal.formal.as_deref();

    // === EVIDENCE-OBLIGATION RECONCILIATION ===
    {
        let evidence_facts = evidence::extract_evidence(verified);
        let pre_obs = config
            .state
            .db
            .get_open_obligations(&config.attempt_id)
            .unwrap_or_default();
        let conflicts = evidence::find_obligation_conflicts(&evidence_facts, &pre_obs);

        if !conflicts.is_empty() {
            let best_lb = evidence::best_lower_bound(&evidence_facts).unwrap_or(0.0);
            tracing::warn!(
                "EVIDENCE SINGULARITY: {} obligation(s) contradicted by verified evidence (lower bound {:.4})",
                conflicts.len(), best_lb
            );
            emit_diagnostic(
                app_handle,
                "evidence_singularity",
                "warn",
                "evidence",
                Some(step_number),
                &format!(
                    "Collapsing {} obligations contradicted by verified evidence (c >= {:.4})",
                    conflicts.len(),
                    best_lb
                ),
                serde_json::json!({
                    "collapsed_count": conflicts.len(),
                    "evidence_bound": best_lb,
                    "obligations": conflicts.iter().map(|c| &c.obligation_id).collect::<Vec<_>>(),
                }),
                &config.attempt_id,
            );

            let closure_ref = verified
                .last()
                .and_then(|(sid, ..)| config.state.db.get_node_by_step_id(sid).ok().flatten())
                .map(|n| n.id)
                .unwrap_or_else(|| "evidence-collapse".to_string());

            for conflict in &conflicts {
                let note = format!(
                    "Auto-closed: verified evidence shows c >= {:.4}, \
                     contradicting obligation's claim of c <= {:.4}. \
                     Evidence from steps {:?}.",
                    conflict.evidence_bound, conflict.obligation_bound, conflict.evidence_steps
                );
                if let Err(e) = config.state.db.close_obligation(
                    &conflict.obligation_id,
                    &closure_ref,
                    "invalidated_by_evidence",
                    Some(&note),
                ) {
                    tracing::warn!(
                        "Failed to close obligation {}: {}",
                        conflict.obligation_id,
                        e
                    );
                } else {
                    tracing::info!(
                        "Collapsed obligation '{}' — evidence bound {:.4} > obligation bound {:.4}",
                        truncate_str(&conflict.obligation_desc, 80),
                        conflict.evidence_bound,
                        conflict.obligation_bound,
                    );
                }
            }

            let _ = app_handle.emit(
                "loop:evidence_collapse",
                LoopEvidenceCollapsePayload {
                    step_number,
                    collapsed_count: conflicts.len() as u32,
                    evidence_bound: best_lb,
                },
            );
        }
    }

    // === OBLIGATION GATE ===
    let open_count = config
        .state
        .db
        .count_open_obligations(&config.attempt_id)
        .unwrap_or(0);
    if open_count > 0 {
        let open_obs = config
            .state
            .db
            .get_open_obligations(&config.attempt_id)
            .unwrap_or_default();
        let ob_list: Vec<String> = open_obs.iter().map(|o| o.description.clone()).collect();
        let reason = format!(
            "GATED: Cannot conclude with {} open obligation(s). Resolve these first: {}",
            open_count,
            ob_list.join("; ")
        );
        tracing::info!("Step {} GATED: {}", step_number, reason);
        emit_diagnostic(
            app_handle,
            "gate",
            "warn",
            "engine",
            Some(step_number),
            &format!("Conclusion blocked: {} open obligation(s)", open_count),
            serde_json::json!({"open_count": open_count, "obligations": &ob_list}),
            &config.attempt_id,
        );

        let _ = app_handle.emit(
            "loop:obligation_gate",
            LoopObligationGatePayload {
                step_number,
                open_count,
                obligations: ob_list,
            },
        );

        use crate::db::StepRecord;
        let rec = StepRecord {
            attempt_id: &config.attempt_id,
            parent_step_id: verified.last().map(|(id, _, _, _, _)| id.as_str()),
            step_number,
            model: &config.model_name,
            context_refs: context_refs_json.as_deref(),
            goal_state,
            context_provided: None,
            proposal_type: "conclusion",
            proposal_natural: &proposal.natural,
            proposal_formal: formal,
            proposal_reasoning: proposal.reasoning.as_deref(),
            sympy_result: None,
            sympy_passed: None,
            pint_result: None,
            pint_passed: None,
            lean_result: None,
            lean_passed: None,
            verified: false,
            rejection_reason: Some(&reason),
            model_tokens_in: llm_result.tokens_in,
            model_tokens_out: llm_result.tokens_out,
            wall_time_ms: None,
            challenge_model: None,
            challenge_flaw_found: None,
            challenge_attack: None,
            challenge_confidence: None,
            challenge_fatal: None,
            obligation_id: targeted_obligation_id.as_deref(),
            solver_round_id: None,
            solver_worker_id: None,
            solver_dispatch_mode: None,
            stale_sibling: false,
        };
        let gated_step_id = db_write_or_log(
            config.state.db.record_step(&rec),
            "record_step(gated)",
            app_handle,
            &config.attempt_id,
        );

        // Backfill step_id on tool_runs
        if !gated_step_id.is_empty() && !tool_run_ids.is_empty() {
            let _ = config
                .state
                .db
                .backfill_tool_runs_step_id(tool_run_ids, &gated_step_id);
        }

        let parent_node_id = if let Some(parent_sid) = verified.last().map(|(id, ..)| id.as_str()) {
            config
                .state
                .db
                .get_node_by_step_id(parent_sid)
                .ok()
                .flatten()
                .map(|n| n.id)
        } else {
            None
        };
        let gated_node_id = db_write_or_log(
            config.state.db.create_node(
                &config.attempt_id,
                state.current_branch_id,
                "closure",
                parent_node_id.as_deref(),
                &proposal.natural,
                formal,
                None,
                None,
                "rejected",
                None,
                Some(&format!(
                    "{{\"gated\": true, \"open_obligations\": {}}}",
                    open_count
                )),
                Some(&config.model_name),
                None,
                Some(&gated_step_id),
                llm_result.tokens_in,
                step_number,
            ),
            "create_node(gated)",
            app_handle,
            &config.attempt_id,
        );
        let _ = config.state.db.append_dag_event(
            &config.attempt_id,
            "conclusion_gated",
            &serde_json::json!({"node_id": &gated_node_id, "open_obligations": open_count})
                .to_string(),
            "loop_engine",
        );

        let step_event = StepEvent {
            attempt_id: config.attempt_id.clone(),
            step_number,
            proposal_type: "conclusion".to_string(),
            proposal_natural: proposal.natural.clone(),
            proposal_formal: proposal.formal.clone(),
            proposal_reasoning: proposal.reasoning.clone(),
            verified: false,
            rejection_reason: Some(reason.clone()),
            model: config.model_name.clone(),
            sympy_passed: None,
            pint_passed: None,
            lean_passed: None,
            challenge_model: None,
            challenge_flaw_found: None,
            challenge_attack: None,
            challenge_confidence: None,
            challenge_fatal: None,
            obligation_id: targeted_obligation_id.clone(),
            obligation_desc: state
                .selected_obligation
                .as_ref()
                .map(|s| s.obligation.description.clone()),
            obligation_type: state
                .selected_obligation
                .as_ref()
                .map(|s| s.obligation.obligation_type.clone()),
            solver_round_id: None,
            solver_worker_id: None,
            solver_dispatch_mode: None,
            stale_sibling: None,
        };
        let _ = app_handle.emit("loop:step_complete", step_event);
        state
            .failures
            .push((proposal.natural.clone(), reason.clone()));
        state.failure_buffer.push(discerner::FailureEntry {
            step_number: Some(step_number),
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            failure_type: "conclusion_gated".into(),
            category: "model".into(),
            reason,
            http_status: None,
            model: config.model_name.clone(),
            proposal_natural: Some(proposal.natural.clone()),
        });
        return Ok(StepOutcome::Continue);
    }

    // === KNOWN ANSWER GATE ===
    if let Some(ref known) = config.problem.known_answer {
        let answer_text = proposal.formal.as_deref().unwrap_or(&proposal.natural);
        let known_lower = known.trim().to_lowercase();
        let answer_lower = answer_text.to_lowercase();
        let natural_lower = proposal.natural.to_lowercase();
        let matches_known =
            answer_lower.contains(&known_lower) || natural_lower.contains(&known_lower);
        if !matches_known {
            let reason = format!(
                "ANSWER MISMATCH: Proposed conclusion does not match known answer '{}'. \
                 Your answer: '{}'. Re-examine your reasoning — you may have a wrong formula or missed a construction.",
                known, answer_text
            );
            tracing::warn!(
                "Step {} REJECTED (known_answer mismatch): proposed='{}', known='{}'",
                step_number,
                answer_text,
                known
            );
            emit_diagnostic(
                app_handle,
                "model",
                "warn",
                "engine",
                Some(step_number),
                &format!(
                    "Answer mismatch: proposed '{}', known '{}'",
                    answer_text, known
                ),
                serde_json::json!({"proposed": answer_text, "known": known}),
                &config.attempt_id,
            );

            let _ = app_handle.emit(
                "loop:answer_mismatch",
                LoopAnswerMismatchPayload {
                    step_number,
                    proposed_answer: answer_text.to_string(),
                    known_answer: known.to_string(),
                },
            );

            use crate::db::StepRecord;
            let rec = StepRecord {
                attempt_id: &config.attempt_id,
                parent_step_id: verified.last().map(|(id, _, _, _, _)| id.as_str()),
                step_number,
                model: &config.model_name,
                context_refs: context_refs_json.as_deref(),
                goal_state,
                context_provided: None,
                proposal_type: "conclusion",
                proposal_natural: &proposal.natural,
                proposal_formal: formal,
                proposal_reasoning: proposal.reasoning.as_deref(),
                sympy_result: None,
                sympy_passed: None,
                pint_result: None,
                pint_passed: None,
                lean_result: None,
                lean_passed: None,
                verified: false,
                rejection_reason: Some(&reason),
                model_tokens_in: llm_result.tokens_in,
                model_tokens_out: llm_result.tokens_out,
                wall_time_ms: None,
                challenge_model: None,
                challenge_flaw_found: None,
                challenge_attack: None,
                challenge_confidence: None,
                challenge_fatal: None,
                obligation_id: targeted_obligation_id.as_deref(),
                solver_round_id: None,
                solver_worker_id: None,
                solver_dispatch_mode: None,
                stale_sibling: false,
            };
            let _ = config.state.db.record_step(&rec);

            let step_event = StepEvent {
                attempt_id: config.attempt_id.clone(),
                step_number,
                proposal_type: "conclusion".to_string(),
                proposal_natural: proposal.natural.clone(),
                proposal_formal: proposal.formal.clone(),
                proposal_reasoning: proposal.reasoning.clone(),
                verified: false,
                rejection_reason: Some(reason.clone()),
                model: config.model_name.clone(),
                sympy_passed: None,
                pint_passed: None,
                lean_passed: None,
                challenge_model: None,
                challenge_flaw_found: None,
                challenge_attack: None,
                challenge_confidence: None,
                challenge_fatal: None,
                obligation_id: targeted_obligation_id.clone(),
                obligation_desc: state
                    .selected_obligation
                    .as_ref()
                    .map(|s| s.obligation.description.clone()),
                obligation_type: state
                    .selected_obligation
                    .as_ref()
                    .map(|s| s.obligation.obligation_type.clone()),
                solver_round_id: None,
                solver_worker_id: None,
                solver_dispatch_mode: None,
                stale_sibling: None,
            };
            let _ = app_handle.emit("loop:step_complete", step_event);
            state
                .failures
                .push((proposal.natural.clone(), reason.clone()));
            state.failure_buffer.push(discerner::FailureEntry {
                step_number: Some(step_number),
                ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                failure_type: "answer_mismatch".into(),
                category: "model".into(),
                reason,
                http_status: None,
                model: config.model_name.clone(),
                proposal_natural: Some(proposal.natural.clone()),
            });
            return Ok(StepOutcome::Continue);
        }
        tracing::info!(
            "Step {} passed known_answer check (matches '{}')",
            step_number,
            known
        );
    }

    // === CLAIM EXTRACTION GATE ===
    {
        let claims =
            claim_extractor::extract_claims(&proposal.natural, proposal.reasoning.as_deref());
        if !claims.is_empty() {
            tracing::info!(
                "Extracted {} claims from conclusion text: {:?}",
                claims.len(),
                claims.iter().map(|c| &c.formal).collect::<Vec<_>>()
            );

            if let Some(conflict) =
                claim_extractor::check_answer_consistency(&claims, formal, &proposal.natural)
            {
                tracing::warn!("Step {} claim inconsistency: {}", step_number, conflict);
                let _ = app_handle.emit(
                    "loop:claim_conflict",
                    LoopClaimConflictPayload {
                        step_number,
                        conflict: conflict.clone(),
                        claims: claims
                            .iter()
                            .map(|claim| {
                                claim_event_record(
                                    &claim.raw_text,
                                    &claim.formal,
                                    &claim.source,
                                    None,
                                )
                            })
                            .collect(),
                    },
                );
            }
        }
    }

    // === CONCLUSION REVIEW GATE ===
    {
        let conclusion_enriched = if !config.enriched_analyst_context.is_empty() {
            format!("{}\n\n", config.enriched_analyst_context)
        } else {
            String::new()
        };
        let conclusion_review_prompt = format!(
"You are a proof conclusion reviewer. A solver claims to have proved the following:\n\n\
{}\
PROBLEM: {}\n\n\
SOLVER'S CONCLUSION: {}\n\
FORMAL EXPRESSION: {}\n\n\
VERIFIED CHAIN ({} steps):\n{}\n\n\
Your job: Is this conclusion SOUND? Check specifically:\n\
1. Does the formal expression actually capture what the natural language claims?\n\
2. Do the verified steps logically lead to this conclusion, or are there gaps?\n\
3. Has the solver confused a sufficient condition for a necessary one (or vice versa)?\n\
4. Is the answer numerically correct for the problem as stated?\n\n\
Use your thinking phase to reason through all four checks above before committing to a verdict.\n\
Respond with ONLY a JSON object (no markdown fences). Fields in this order — reason before verdict:\n\
{{\"reason\": \"full analysis of each check\", \"confidence\": 0.0-1.0, \"sound\": true/false}}",
            conclusion_enriched,
            config.problem.statement,
            proposal.natural,
            formal.unwrap_or("(none)"),
            verified.len(),
            verified.iter().map(|(_, n, _, nat, f)| format!("  Step {}: {} [{}]", n, nat, f)).collect::<Vec<_>>().join("\n"),
        );

        let review_handle = app_handle.clone();
        emit_loop_thinking_start(
            app_handle,
            Some(step_number),
            &config.model_name,
            Some("reviewer"),
            targeted_obligation_id.as_deref(),
            Some(true),
            None,
        );
        let review_obligation_id = targeted_obligation_id.clone();
        let conclusion_review = config
            .reviewer_llm
            .complete_streaming(&conclusion_review_prompt, move |chunk| {
                emit_loop_token(
                    &review_handle,
                    chunk,
                    Some("reviewer"),
                    review_obligation_id.as_deref(),
                );
            })
            .await;
        emit_loop_thinking_end(app_handle, targeted_obligation_id.as_deref());

        if let Ok(review_resp) = conclusion_review {
            let review_text = review_resp.text.trim();
            let review_json = if let Some(start) = review_text.find('{') {
                review_text.get(start..=review_text.rfind('}').unwrap_or(review_text.len() - 1))
            } else {
                None
            };

            if let Some(json_str) = review_json {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                    let sound = parsed
                        .get("sound")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    let confidence = parsed
                        .get("confidence")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.5);
                    let reason = parsed
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    tracing::info!(
                        "Conclusion review: sound={}, confidence={:.2}, reason='{}'",
                        sound,
                        confidence,
                        reason
                    );

                    let _ = app_handle.emit(
                        "loop:conclusion_review",
                        LoopConclusionReviewPayload {
                            step_number,
                            sound,
                            confidence,
                            reason: reason.to_string(),
                        },
                    );

                    if !sound && confidence >= 0.6 {
                        let reject_reason = format!(
                            "CONCLUSION UNSOUND (confidence {:.0}%): {}. Re-examine your reasoning.",
                            confidence * 100.0, reason
                        );
                        tracing::warn!(
                            "Step {} REJECTED by conclusion review: {}",
                            step_number,
                            reject_reason
                        );
                        emit_diagnostic(
                            app_handle,
                            "model",
                            "warn",
                            "reviewer",
                            Some(step_number),
                            &format!(
                                "Conclusion unsound ({:.0}%): {}",
                                confidence * 100.0,
                                reason
                            ),
                            serde_json::json!({"sound": false, "confidence": confidence, "reason": reason}),
                            &config.attempt_id,
                        );

                        use crate::db::StepRecord;
                        let rec = StepRecord {
                            attempt_id: &config.attempt_id,
                            parent_step_id: verified.last().map(|(id, _, _, _, _)| id.as_str()),
                            step_number,
                            model: &config.model_name,
                            context_refs: context_refs_json.as_deref(),
                            goal_state,
                            context_provided: None,
                            proposal_type: "conclusion",
                            proposal_natural: &proposal.natural,
                            proposal_formal: formal,
                            proposal_reasoning: proposal.reasoning.as_deref(),
                            sympy_result: None,
                            sympy_passed: None,
                            pint_result: None,
                            pint_passed: None,
                            lean_result: None,
                            lean_passed: None,
                            verified: false,
                            rejection_reason: Some(&reject_reason),
                            model_tokens_in: llm_result.tokens_in,
                            model_tokens_out: llm_result.tokens_out,
                            wall_time_ms: None,
                            challenge_model: None,
                            challenge_flaw_found: None,
                            challenge_attack: None,
                            challenge_confidence: None,
                            challenge_fatal: None,
                            obligation_id: targeted_obligation_id.as_deref(),
                            solver_round_id: None,
                            solver_worker_id: None,
                            solver_dispatch_mode: None,
                            stale_sibling: false,
                        };
                        let _ = config.state.db.record_step(&rec);

                        let step_event = StepEvent {
                            attempt_id: config.attempt_id.clone(),
                            step_number,
                            proposal_type: "conclusion".to_string(),
                            proposal_natural: proposal.natural.clone(),
                            proposal_formal: proposal.formal.clone(),
                            proposal_reasoning: proposal.reasoning.clone(),
                            verified: false,
                            rejection_reason: Some(reject_reason.clone()),
                            model: config.model_name.clone(),
                            sympy_passed: None,
                            pint_passed: None,
                            lean_passed: None,
                            challenge_model: None,
                            challenge_flaw_found: None,
                            challenge_attack: None,
                            challenge_confidence: None,
                            challenge_fatal: None,
                            obligation_id: targeted_obligation_id.clone(),
                            obligation_desc: state
                                .selected_obligation
                                .as_ref()
                                .map(|s| s.obligation.description.clone()),
                            obligation_type: state
                                .selected_obligation
                                .as_ref()
                                .map(|s| s.obligation.obligation_type.clone()),
                            solver_round_id: None,
                            solver_worker_id: None,
                            solver_dispatch_mode: None,
                            stale_sibling: None,
                        };
                        let _ = app_handle.emit("loop:step_complete", step_event);
                        state
                            .failures
                            .push((proposal.natural.clone(), reject_reason.clone()));
                        state.failure_buffer.push(discerner::FailureEntry {
                            step_number: Some(step_number),
                            ts: chrono::Utc::now()
                                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                            failure_type: "conclusion_unsound".into(),
                            category: "model".into(),
                            reason: reject_reason,
                            http_status: None,
                            model: config.model_name.clone(),
                            proposal_natural: Some(proposal.natural.clone()),
                        });
                        return Ok(StepOutcome::Continue);
                    }
                }
            }
        } else {
            tracing::warn!(
                "Conclusion review LLM call failed — proceeding with acceptance (fail-open)"
            );
            emit_diagnostic(
                app_handle,
                "mechanical",
                "warn",
                "reviewer",
                Some(step_number),
                "Conclusion review LLM call failed — accepting (fail-open)",
                serde_json::json!({}),
                &config.attempt_id,
            );
        }
    }

    // All gates passed — conclusion accepted
    tracing::info!("Step {} accepted as conclusion ({}+ verified steps, 0 open obligations, review passed) — proof complete", step_number, verified.len());
    use crate::db::StepRecord;
    let rec = StepRecord {
        attempt_id: &config.attempt_id,
        parent_step_id: verified.last().map(|(id, _, _, _, _)| id.as_str()),
        step_number,
        model: &config.model_name,
        context_refs: context_refs_json.as_deref(),
        goal_state,
        context_provided: None,
        proposal_type: "conclusion",
        proposal_natural: &proposal.natural,
        proposal_formal: formal,
        proposal_reasoning: proposal.reasoning.as_deref(),
        sympy_result: None,
        sympy_passed: None,
        pint_result: None,
        pint_passed: None,
        lean_result: None,
        lean_passed: None,
        verified: true,
        rejection_reason: None,
        model_tokens_in: llm_result.tokens_in,
        model_tokens_out: llm_result.tokens_out,
        wall_time_ms: None,
        challenge_model: None,
        challenge_flaw_found: None,
        challenge_attack: None,
        challenge_confidence: None,
        challenge_fatal: None,
        obligation_id: targeted_obligation_id.as_deref(),
        solver_round_id: None,
        solver_worker_id: None,
        solver_dispatch_mode: None,
        stale_sibling: false,
    };
    let conclusion_step_id = db_write_or_log(
        config.state.db.record_step(&rec),
        "record_step(conclusion)",
        app_handle,
        &config.attempt_id,
    );

    // Backfill step_id on tool_runs
    if !conclusion_step_id.is_empty() && !tool_run_ids.is_empty() {
        let _ = config
            .state
            .db
            .backfill_tool_runs_step_id(tool_run_ids, &conclusion_step_id);
    }

    let parent_node_id = if let Some(parent_sid) = verified.last().map(|(id, ..)| id.as_str()) {
        config
            .state
            .db
            .get_node_by_step_id(parent_sid)
            .ok()
            .flatten()
            .map(|n| n.id)
    } else {
        None
    };
    let conclusion_node_id = db_write_or_log(
        config.state.db.create_node(
            &config.attempt_id,
            state.current_branch_id,
            "closure",
            parent_node_id.as_deref(),
            &proposal.natural,
            formal,
            None,
            None,
            "verified",
            None,
            None,
            Some(&config.model_name),
            None,
            Some(&conclusion_step_id),
            llm_result.tokens_in,
            step_number,
        ),
        "create_node(conclusion)",
        app_handle,
        &config.attempt_id,
    );
    let _ = config.state.db.append_dag_event(
        &config.attempt_id,
        "conclusion_verified",
        &serde_json::json!({"node_id": &conclusion_node_id, "step_number": step_number})
            .to_string(),
        "loop_engine",
    );

    let step_event = StepEvent {
        attempt_id: config.attempt_id.clone(),
        step_number,
        proposal_type: "conclusion".to_string(),
        proposal_natural: proposal.natural.clone(),
        proposal_formal: proposal.formal.clone(),
        proposal_reasoning: proposal.reasoning.clone(),
        verified: true,
        rejection_reason: None,
        model: config.model_name.clone(),
        sympy_passed: None,
        pint_passed: None,
        lean_passed: None,
        challenge_model: None,
        challenge_flaw_found: None,
        challenge_attack: None,
        challenge_confidence: None,
        challenge_fatal: None,
        obligation_id: targeted_obligation_id.clone(),
        obligation_desc: state
            .selected_obligation
            .as_ref()
            .map(|s| s.obligation.description.clone()),
        obligation_type: state
            .selected_obligation
            .as_ref()
            .map(|s| s.obligation.obligation_type.clone()),
        solver_round_id: None,
        solver_worker_id: None,
        solver_dispatch_mode: None,
        stale_sibling: None,
    };
    let _ = app_handle.emit("loop:step_complete", step_event);

    let _ = config.state.db.close_branch(
        state.current_branch_id as i64,
        "completed",
        Some(&proposal.natural),
        formal,
    );
    let _ = config.state.db.append_dag_event(
        &config.attempt_id,
        "branch_closed",
        &serde_json::json!({
            "branch_id": state.current_branch_id,
            "status": "completed",
        })
        .to_string(),
        "loop_engine",
    );
    let _ = app_handle.emit(
        "loop:branch_closed",
        LoopBranchClosedPayload {
            branch_id: state.current_branch_id as u32,
            status: "completed".to_string(),
        },
    );

    state.proof_complete = true;
    Ok(StepOutcome::ProofComplete)
}

// ── Satisfaction tally (extracted for size) ──────────────────────────

/// Run the multi-voter satisfaction tally system for all open obligations.
/// Includes mechanical pre-screen, solver self-assessment, reviewer + adversary council,
/// and periodic checkpoint every 10 steps.
#[allow(clippy::too_many_arguments)]
async fn handle_satisfaction_tally(
    config: &StepConfig,
    state: &mut StepState,
    app_handle: &tauri::AppHandle,
    proposal: &LlmProposal,
    verified: &[(String, u32, String, String, String)],
    all_obligations: &[crate::models::dag::Obligation],
    targeted_obligation_id: &Option<String>,
    current_node_id: &Option<String>,
    current_step_id: &Option<String>,
    step_number: u32,
    _proposal_type: &str,
) {
    let open_obs = config
        .state
        .db
        .get_open_obligations(&config.attempt_id)
        .unwrap_or_default();

    let closure_node = current_node_id.as_deref().unwrap_or("unknown");
    let mut any_closed = false;
    let step_id_ref = current_step_id.as_deref();
    let mut ambiguous_obs: Vec<(
        &crate::models::dag::Obligation,
        Vec<crate::models::dag::ProofNode>,
    )> = Vec::new();
    let mut round_tally: std::collections::HashMap<String, (i32, i32)> =
        std::collections::HashMap::new();

    for ob in &open_obs {
        let ob_nodes = config
            .state
            .db
            .get_nodes_for_obligation(&ob.id)
            .unwrap_or_default();

        // VOTE 0: Mechanical
        let result = satisfaction::check_obligation_satisfaction(
            ob,
            &proposal.natural,
            proposal.formal.as_deref(),
            verified,
            &ob_nodes,
        );
        if let satisfaction::SatisfactionResult::Satisfied { ref note } = result {
            let _ = config.state.db.record_satisfaction_signal(
                &ob.id,
                step_id_ref,
                "mechanical",
                None,
                true,
                1.0,
                Some(note.as_str()),
            );
            let entry = round_tally.entry(ob.id.clone()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += 1;
            emit_satisfaction_signal(
                app_handle,
                &ob.id,
                SatisfactionSource::Mechanical,
                true,
                entry.0 as u32,
                entry.1 as u32,
                Some(note.as_str()),
            );
        }

        // VOTE 1: Solver self-assessment
        let is_targeted = state
            .selected_obligation
            .as_ref()
            .map_or(false, |sel| sel.obligation.id == ob.id);
        let solver_satisfies = is_targeted && proposal.closes_obligation == Some(true);
        let solver_note = if is_targeted {
            proposal.closure_reason.as_deref()
        } else {
            None
        };

        // Determine if this obligation should go to the LLM council.
        // Only fire expensive reviewer + adversary calls when there's a signal:
        //   1. Mechanical check passed (strong heuristic evidence)
        //   2. Solver claims closure (targeted obligation, closes_obligation=true)
        //   3. Periodic safety net (every 5 steps catches drift)
        let mechanical_satisfied =
            matches!(result, satisfaction::SatisfactionResult::Satisfied { .. });
        let periodic_check = step_number > 0 && step_number % 5 == 0;
        let needs_council = mechanical_satisfied || solver_satisfies || periodic_check;

        // Only record solver vote when council will also run — avoids wasted
        // DB writes on steps where the tally can never reach quorum anyway.
        if needs_council {
            let _ = config.state.db.record_satisfaction_signal(
                &ob.id,
                step_id_ref,
                "solver",
                Some(&config.model_name),
                solver_satisfies,
                1.0,
                solver_note,
            );
            let entry = round_tally.entry(ob.id.clone()).or_insert((0, 0));
            if solver_satisfies {
                entry.0 += 1;
            }
            entry.1 += 1;
            emit_satisfaction_signal(
                app_handle,
                &ob.id,
                SatisfactionSource::Solver,
                solver_satisfies,
                entry.0 as u32,
                entry.1 as u32,
                solver_note,
            );
            ambiguous_obs.push((ob, ob_nodes));
        }
    }

    // Stage 3: LLM council
    if !ambiguous_obs.is_empty() {
        let step_desc = format!(
            "{} [formal: {}]",
            proposal.natural,
            proposal.formal.as_deref().unwrap_or("none")
        );

        let ob_list_str: String = ambiguous_obs
            .iter()
            .enumerate()
            .map(|(i, (ob, nodes))| {
                let verified_nodes: Vec<_> =
                    nodes.iter().filter(|n| n.status == "verified").collect();
                let mut entry = format!(
                    "  {}. [id={}] [{}] {}",
                    i + 1,
                    ob.id,
                    ob.obligation_type,
                    ob.description
                );
                if let Some(ref criteria) = ob.satisfaction_criteria {
                    entry.push_str(&format!("\n     Done when: {}", criteria));
                }
                if !verified_nodes.is_empty() {
                    entry.push_str(&format!(
                        "\n     Prior verified steps ({} total):",
                        verified_nodes.len()
                    ));
                    for node in verified_nodes.iter().rev().take(5).rev() {
                        let preview: String = node.content.chars().take(120).collect();
                        let formal = node.formal_content.as_deref().unwrap_or("none");
                        entry.push_str(&format!("\n       - \"{}\" [formal: {}]", preview, formal));
                    }
                }
                entry
            })
            .collect::<Vec<_>>()
            .join("\n");

        let solver_hint = if proposal.closes_obligation == Some(true) {
            let reason = proposal
                .closure_reason
                .as_deref()
                .unwrap_or("no reason given");
            let target = proposal.targets_obligation.as_deref().unwrap_or("unknown");
            format!(
                "\n\nSOLVER SELF-ASSESSMENT (advisory only — verify independently):\n\
                The solver believes this step closes a {} obligation. Reason: {}\n",
                target, reason
            )
        } else {
            String::new()
        };

        let closed_obs: Vec<&crate::models::dag::Obligation> = all_obligations
            .iter()
            .filter(|o| o.status != "open" && o.status != "assigned")
            .collect();
        let closed_section = if !closed_obs.is_empty() {
            let mut s = format!(
                "\nALREADY RESOLVED ({} obligations — do NOT re-evaluate):\n",
                closed_obs.len()
            );
            for ob in closed_obs.iter().take(6) {
                let status_label = match ob.status.as_str() {
                    "closed_proved" => "PROVED",
                    "closed_spurious" => "SPURIOUS",
                    _ => "CLOSED",
                };
                s.push_str(&format!(
                    "  [{}] [{}] {}\n",
                    status_label, ob.obligation_type, ob.description
                ));
            }
            s.push_str("Only evaluate the OPEN obligations below.\n");
            s
        } else {
            String::new()
        };

        let enriched_section = if !config.enriched_analyst_context.is_empty() {
            format!("{}\n\n", config.enriched_analyst_context)
        } else {
            String::new()
        };

        let resolution_prompt = format!(
"A verified proof step was just added. Check which (if any) of the following obligations \
are NOW SATISFIED by the CUMULATIVE work done (not just this single step).\n\n\
IMPORTANT: Judge ONLY the verified steps shown below. Do NOT import answers from your training data.\n\
If you recognize this problem, ignore what you think the answer should be. Evaluate only what the steps prove.\n\n\
{}\
LATEST STEP: {}{}\n{}\n\
OPEN OBLIGATIONS (with prior work shown):\n{}\n\n\
For each obligation, assess whether the ACCUMULATED verified steps collectively \
fulfill the obligation's requirements.\n\
Use your thinking phase to reason through each obligation before committing.\n\
Respond with ONLY a JSON array with ONE entry per obligation. Include ALL obligations, not just resolved ones.\n\
Fields in this order — note before id:\n\
- \"note\": your reasoning — what the steps prove and what's still missing (or why it IS satisfied)\n\
- \"id\": the obligation id\n\
- \"satisfied\": true if you believe the cumulative work satisfies it, false if not\n\n\
Example: [{{\"note\": \"Steps 3-7 prove the identity for n>0 but n=0 case is missing\", \"id\": \"abc-123\", \"satisfied\": false}}, \
{{\"note\": \"All cases covered by steps 2, 5, and 8\", \"id\": \"def-456\", \"satisfied\": true}}]", enriched_section, step_desc, solver_hint, closed_section, ob_list_str);

        // VOTE 3: Reviewer
        let reviewer_model_str = config.reviewer_llm.model_name();
        let res_handle = app_handle.clone();
        let reviewer_obligation_id = targeted_obligation_id.clone();
        emit_loop_thinking_start(
            app_handle,
            Some(step_number),
            &reviewer_model_str,
            Some("reviewer"),
            reviewer_obligation_id.as_deref(),
            None,
            None,
        );
        if let Ok(res) = config
            .reviewer_llm
            .complete_streaming(&resolution_prompt, move |chunk| {
                emit_loop_token(
                    &res_handle,
                    chunk,
                    Some("reviewer"),
                    reviewer_obligation_id.as_deref(),
                );
            })
            .await
        {
            // Parse reviewer verdicts — new format includes all obligations with note+satisfied
            let reviewer_verdicts = parse_reviewer_verdicts(&res.text);
            // Also parse legacy format as fallback
            let resolved = parse_resolved_obligations(&res.text);
            let resolved_ids: std::collections::HashSet<&str> =
                resolved.iter().map(|(id, _)| id.as_str()).collect();

            for (ob, _) in &ambiguous_obs {
                // Try new format first: explicit satisfied + note per obligation
                let (satisfies, note) = if let Some(verdict) =
                    reviewer_verdicts.iter().find(|(id, _, _)| id == &ob.id)
                {
                    (
                        verdict.1,
                        if verdict.2.is_empty() {
                            None
                        } else {
                            Some(verdict.2.as_str())
                        },
                    )
                } else {
                    // Fallback: legacy format (only resolved obligations have notes)
                    let sat = resolved_ids.contains(ob.id.as_str());
                    let n = resolved
                        .iter()
                        .find(|(id, _)| id == &ob.id)
                        .map(|(_, n)| n.as_str());
                    (sat, n)
                };
                let _ = config.state.db.record_satisfaction_signal(
                    &ob.id,
                    step_id_ref,
                    "reviewer",
                    Some(&reviewer_model_str),
                    satisfies,
                    1.0,
                    note,
                );
                let entry = round_tally.entry(ob.id.clone()).or_insert((0, 0));
                if satisfies {
                    entry.0 += 1;
                }
                entry.1 += 1;
                emit_satisfaction_signal(
                    app_handle,
                    &ob.id,
                    SatisfactionSource::Reviewer,
                    satisfies,
                    entry.0 as u32,
                    entry.1 as u32,
                    note,
                );
            }
        }
        emit_loop_thinking_end(app_handle, targeted_obligation_id.as_deref());

        // VOTE 4: Adversary
        let adversary_prompt = critic::build_adversary_satisfaction_prompt(
            &ambiguous_obs,
            &proposal.natural,
            proposal.formal.as_deref(),
            all_obligations,
            &solver_hint,
            step_number,
            &config.enriched_analyst_context,
        );
        let adv_handle = app_handle.clone();
        let adversary_obligation_id = targeted_obligation_id.clone();
        emit_loop_thinking_start(
            app_handle,
            Some(step_number),
            &config.adversary_model_name,
            Some("adversary"),
            adversary_obligation_id.as_deref(),
            None,
            None,
        );
        if let Ok(adv_res) = config
            .adversary_llm
            .complete_streaming(&adversary_prompt, move |chunk| {
                emit_loop_token(
                    &adv_handle,
                    chunk,
                    Some("adversary"),
                    adversary_obligation_id.as_deref(),
                );
            })
            .await
        {
            let verdicts = critic::parse_adversary_satisfaction(&adv_res.text);
            for (ob_id, satisfied, objection) in &verdicts {
                let note = if objection.is_empty() {
                    None
                } else {
                    Some(objection.as_str())
                };
                let _ = config.state.db.record_satisfaction_signal(
                    ob_id,
                    step_id_ref,
                    "adversary",
                    Some(&config.adversary_model_name),
                    *satisfied,
                    1.0,
                    note,
                );
                let entry = round_tally.entry(ob_id.clone()).or_insert((0, 0));
                if *satisfied {
                    entry.0 += 1;
                }
                entry.1 += 1;
                emit_satisfaction_signal(
                    app_handle,
                    ob_id,
                    SatisfactionSource::Adversary,
                    *satisfied,
                    entry.0 as u32,
                    entry.1 as u32,
                    note,
                );
            }
        }
        emit_loop_thinking_end(app_handle, targeted_obligation_id.as_deref());
    }

    // === Tally Check ===
    for ob in &open_obs {
        if let Some(&(yes, total)) = round_tally.get(&ob.id) {
            if tally_has_closing_majority(yes as u32, total as u32) {
                let tally_note = format!("Round tally: {}/{} votes satisfied", yes, total);
                let _ = config.state.db.close_obligation(
                    &ob.id,
                    closure_node,
                    "proved",
                    Some(&tally_note),
                );
                tracing::info!(
                    "Obligation closed (round majority): {} — {}",
                    ob.description,
                    tally_note
                );
                emit_diagnostic(
                    app_handle,
                    "info",
                    "info",
                    "satisfaction",
                    Some(step_number),
                    &format!("Obligation closed (tally): {}", ob.description),
                    serde_json::json!({"tally_yes": yes, "tally_total": total}),
                    &config.attempt_id,
                );
                let _ = config.state.db.append_dag_event(
                    &config.attempt_id,
                    "obligation_closed",
                    &serde_json::json!({
                        "obligation_id": &ob.id,
                        "closure_node_id": closure_node,
                        "closure_type": "proved",
                        "closure_note": &tally_note,
                        "resolution_stage": "tally",
                        "tally_yes": yes,
                        "tally_total": total,
                    })
                    .to_string(),
                    "satisfaction",
                );
                emit_obligation_closed(
                    app_handle,
                    &ob.id,
                    ObligationStatus::ClosedProved,
                    Some(closure_node),
                    Some(step_number),
                    Some(&tally_note),
                    Some(yes as u32),
                    Some(total as u32),
                );
                let _ = app_handle.emit(
                    "agent:council_finding",
                    AgentCouncilFindingPayload {
                        obligation_id: ob.id.clone(),
                        tally_yes: yes as u32,
                        tally_total: total as u32,
                        outcome: "closed_proved".to_string(),
                        note: Some(tally_note.clone()),
                    },
                );
                any_closed = true;
            }
        }
    }

    // === Periodic Obligation Checkpoint (every 10 steps) ===
    if step_number > 0 && step_number.is_multiple_of(10) {
        tracing::info!("Step {} — periodic obligation checkpoint", step_number);
        let _ = app_handle.emit(
            "loop:checkpoint_start",
            LoopCheckpointStartPayload {
                step_number,
                open_obligations: open_obs.len() as u32,
            },
        );

        let checkpoint_obs = config
            .state
            .db
            .get_open_obligations(&config.attempt_id)
            .unwrap_or_default();

        let checkpoint_review_pairs: Vec<(
            &crate::models::dag::Obligation,
            Vec<crate::models::dag::ProofNode>,
        )> = checkpoint_obs
            .iter()
            .filter_map(|ob| {
                let nodes = config
                    .state
                    .db
                    .get_nodes_for_obligation(&ob.id)
                    .unwrap_or_default();
                if obligation_needs_llm_review(&ob.id, None, false, &nodes) {
                    Some((ob, nodes))
                } else {
                    None
                }
            })
            .collect();

        if !checkpoint_review_pairs.is_empty() {
            let checkpoint_ob_list: String = checkpoint_review_pairs
                .iter()
                .enumerate()
                .map(|(i, (ob, nodes))| {
                    let vn: Vec<_> = nodes.iter().filter(|n| n.status == "verified").collect();
                    let mut entry = format!(
                        "  {}. [id={}] [{}] {}",
                        i + 1,
                        ob.id,
                        ob.obligation_type,
                        ob.description
                    );
                    if let Some(ref criteria) = ob.satisfaction_criteria {
                        entry.push_str(&format!("\n     Done when: {}", criteria));
                    }
                    if !vn.is_empty() {
                        entry.push_str(&format!("\n     Verified steps ({}):", vn.len()));
                        for n in vn.iter().rev().take(6).rev() {
                            let preview: String = n.content.chars().take(120).collect();
                            let formal = n.formal_content.as_deref().unwrap_or("none");
                            entry.push_str(&format!(
                                "\n       - \"{}\" [formal: {}]",
                                preview, formal
                            ));
                        }
                    } else {
                        entry.push_str("\n     No verified steps yet.");
                    }
                    entry
                })
                .collect::<Vec<_>>()
                .join("\n");

            let closed_section = {
                let closed: Vec<_> = all_obligations
                    .iter()
                    .filter(|o| o.status != "open" && o.status != "assigned")
                    .collect();
                if closed.is_empty() {
                    String::new()
                } else {
                    let mut s = format!("\nALREADY RESOLVED ({}):\n", closed.len());
                    for o in closed.iter().take(6) {
                        s.push_str(&format!(
                            "  [{}] [{}] {}\n",
                            if o.status == "closed_proved" {
                                "PROVED"
                            } else {
                                "CLOSED"
                            },
                            o.obligation_type,
                            o.description
                        ));
                    }
                    s
                }
            };

            let checkpoint_prompt = format!(
"CHECKPOINT REVIEW — step {} of the proof attempt.\n\
Review all open obligations against the FULL accumulated proof work shown below.\n\
Do not import answers from your training data — judge only the verified steps listed.\n\
{}\n\
OPEN OBLIGATIONS (with full accumulated evidence):\n{}\n\n\
For each obligation, assess whether the cumulative verified steps collectively satisfy it.\n\
Use your thinking phase to reason through each obligation.\n\
Respond with ONLY a JSON array with ONE entry per obligation. Include ALL obligations, not just resolved ones.\n\
Fields: \"note\" (your reasoning), \"id\" (obligation id), \"satisfied\" (true/false).\n\
Example: [{{\"note\": \"Steps 3-12 prove X but case Y is missing\", \"id\": \"abc-123\", \"satisfied\": false}}]",
                step_number, closed_section, checkpoint_ob_list);

            let mut checkpoint_tally: std::collections::HashMap<String, (i32, i32)> =
                std::collections::HashMap::new();
            let closure_node_chk = current_node_id.as_deref().unwrap_or("unknown");
            let step_id_chk = current_step_id.as_deref();

            // Reviewer checkpoint vote
            let res_handle = app_handle.clone();
            if let Ok(res) = config
                .reviewer_llm
                .complete_streaming(&checkpoint_prompt, move |chunk| {
                    emit_loop_token(&res_handle, chunk, Some("checkpoint_reviewer"), None);
                })
                .await
            {
                let reviewer_verdicts = parse_reviewer_verdicts(&res.text);
                let resolved = parse_resolved_obligations(&res.text);
                let resolved_ids: std::collections::HashSet<&str> =
                    resolved.iter().map(|(id, _)| id.as_str()).collect();
                for (ob, _) in &checkpoint_review_pairs {
                    let (satisfies, note) = if let Some(verdict) =
                        reviewer_verdicts.iter().find(|(id, _, _)| id == &ob.id)
                    {
                        (
                            verdict.1,
                            if verdict.2.is_empty() {
                                None
                            } else {
                                Some(verdict.2.as_str())
                            },
                        )
                    } else {
                        let sat = resolved_ids.contains(ob.id.as_str());
                        let n = resolved
                            .iter()
                            .find(|(id, _)| id == &ob.id)
                            .map(|(_, n)| n.as_str());
                        (sat, n)
                    };
                    let _ = config.state.db.record_satisfaction_signal(
                        &ob.id,
                        step_id_chk,
                        "checkpoint_reviewer",
                        None,
                        satisfies,
                        1.0,
                        note,
                    );
                    let entry = checkpoint_tally.entry(ob.id.clone()).or_insert((0, 0));
                    if satisfies {
                        entry.0 += 1;
                    }
                    entry.1 += 1;
                    emit_satisfaction_signal(
                        app_handle,
                        &ob.id,
                        SatisfactionSource::CheckpointReviewer,
                        satisfies,
                        entry.0 as u32,
                        entry.1 as u32,
                        note,
                    );
                }
            }

            // Adversary checkpoint vote
            let adv_ob_pairs: Vec<(
                &crate::models::dag::Obligation,
                Vec<crate::models::dag::ProofNode>,
            )> = checkpoint_review_pairs
                .iter()
                .map(|(ob, nodes)| (*ob, nodes.clone()))
                .collect();
            let adv_checkpoint_prompt = critic::build_adversary_satisfaction_prompt(
                &adv_ob_pairs,
                &proposal.natural,
                proposal.formal.as_deref(),
                all_obligations,
                "", // no solver hint at checkpoint
                step_number,
                &config.enriched_analyst_context,
            );
            let adv_handle = app_handle.clone();
            if let Ok(adv_res) = config
                .adversary_llm
                .complete_streaming(&adv_checkpoint_prompt, move |chunk| {
                    emit_loop_token(&adv_handle, chunk, Some("checkpoint_adversary"), None);
                })
                .await
            {
                let verdicts = critic::parse_adversary_satisfaction(&adv_res.text);
                for (ob_id, satisfied, objection) in &verdicts {
                    let note = if objection.is_empty() {
                        None
                    } else {
                        Some(objection.as_str())
                    };
                    let _ = config.state.db.record_satisfaction_signal(
                        ob_id,
                        step_id_chk,
                        "checkpoint_adversary",
                        Some(&config.adversary_model_name),
                        *satisfied,
                        1.0,
                        note,
                    );
                    let entry = checkpoint_tally.entry(ob_id.clone()).or_insert((0, 0));
                    if *satisfied {
                        entry.0 += 1;
                    }
                    entry.1 += 1;
                    emit_satisfaction_signal(
                        app_handle,
                        ob_id,
                        SatisfactionSource::CheckpointAdversary,
                        *satisfied,
                        entry.0 as u32,
                        entry.1 as u32,
                        note,
                    );
                }
            }

            // Tally check: close obligations with checkpoint majority
            for (ob, _) in &checkpoint_review_pairs {
                if let Some(&(yes, total)) = checkpoint_tally.get(&ob.id) {
                    if tally_has_closing_majority(yes as u32, total as u32) {
                        let note = format!("Checkpoint tally: {}/{}", yes, total);
                        let _ = config.state.db.close_obligation(
                            &ob.id,
                            closure_node_chk,
                            "proved",
                            Some(&note),
                        );
                        tracing::info!(
                            "Obligation closed (checkpoint): {} — {}",
                            &ob.description,
                            &note
                        );
                        emit_obligation_closed(
                            app_handle,
                            &ob.id,
                            ObligationStatus::ClosedProved,
                            Some(closure_node_chk),
                            Some(step_number),
                            Some(&note),
                            Some(yes as u32),
                            Some(total as u32),
                        );
                        any_closed = true;
                    }
                }
            }
        }
    }

    state.orchestrator.record_closure_event(any_closed);
}
