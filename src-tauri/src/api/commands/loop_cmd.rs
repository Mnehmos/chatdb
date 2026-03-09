use crate::api::llm_client::LlmClient;
use crate::contracts::loop_events::{
    EmptyPayload, LoopReviewStartPayload, LoopThinkingEndPayload, LoopThinkingStartPayload,
    LoopTokenPayload, ReviewFinding, ReviewResult as ContractReviewResult,
};
use crate::loop_engine::context_enricher;
use crate::loop_engine::review;
use crate::loop_engine::LoopEngine;
use crate::models::agents::MultiAgentConfig;
use crate::AppState;
use serde::Serialize;
use std::sync::Arc;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize)]
pub struct LoopStatusResponse {
    pub running: bool,
    pub attempt_id: Option<String>,
}

#[tauri::command]
pub async fn start_solve(
    state: tauri::State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    problem_id: String,
    config: Option<MultiAgentConfig>,
    force: Option<bool>,
) -> Result<String, String> {
    // If force flag is set and loop is running, stop it and wait for it to finish
    if force.unwrap_or(false) {
        let mut running = state.loop_running.lock().await;
        if *running {
            *running = false;
            drop(running);
            // Wait for the old loop to actually stop (checks every 50ms, up to 3s)
            for _ in 0..60 {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                let still = *state.loop_running.lock().await;
                if !still {
                    break;
                }
            }
        }
    }

    let mut running = state.loop_running.lock().await;
    if *running {
        return Err("Loop already running — previous solve didn't stop in time".to_string());
    }
    *running = true;
    drop(running);

    let cfg = config.unwrap_or_default();
    let models: Vec<String> = cfg
        .models
        .iter()
        .map(|m| format!("{}/{}", m.provider, m.model))
        .collect();
    let attempt_id = state
        .db
        .create_attempt(&problem_id, &models)
        .map_err(|e| e.to_string())?;

    *state.current_attempt_id.lock().await = Some(attempt_id.clone());

    let attempt_id_clone = attempt_id.clone();
    let state_inner = state.inner().clone();
    let problem_id_clone = problem_id.clone();

    tokio::spawn(async move {
        if let Err(e) = LoopEngine::run_outer_loop(
            state_inner.clone(),
            cfg,
            problem_id_clone,
            attempt_id_clone,
            app_handle.clone(),
        )
        .await
        {
            tracing::error!("Loop engine error: {}", e);
            // Reset loop state so frontend doesn't stay stuck on "running"
            *state_inner.loop_running.lock().await = false;
            *state_inner.current_attempt_id.lock().await = None;
            // Surface the error to the frontend
            let _ = app_handle.emit(
                "loop:error",
                serde_json::json!({
                    "message": e.to_string(),
                }),
            );
            let _ = app_handle.emit(
                "loop:outer_complete",
                serde_json::json!({
                    "attempts_used": 0,
                    "error": e.to_string(),
                }),
            );
        }
    });

    tracing::info!("Started solve for {} attempt {}", problem_id, attempt_id);
    Ok(attempt_id)
}

#[tauri::command]
pub async fn continue_solve(
    state: tauri::State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    problem_id: String,
    config: Option<MultiAgentConfig>,
    attempt_id: Option<String>,
) -> Result<String, String> {
    let mut running = state.loop_running.lock().await;
    if *running {
        return Err("Loop already running".to_string());
    }
    *running = true;
    drop(running);

    // Resume a specific attempt, or fall back to latest
    let attempt_id = match attempt_id {
        Some(id) => id,
        None => state
            .db
            .get_latest_attempt_id(&problem_id)
            .map_err(|e| e.to_string())?,
    };
    let starting_step = state
        .db
        .get_max_step_number(&attempt_id)
        .map_err(|e| e.to_string())?;

    *state.current_attempt_id.lock().await = Some(attempt_id.clone());

    let cfg = config.unwrap_or_default();
    let attempt_id_clone = attempt_id.clone();
    let state_inner = state.inner().clone();
    let problem_id_clone = problem_id.clone();

    tokio::spawn(async move {
        let engine = LoopEngine::new(
            state_inner.clone(),
            cfg,
            problem_id_clone,
            attempt_id_clone,
            starting_step,
        );
        if let Err(e) = engine.run(app_handle.clone()).await {
            tracing::error!("Loop engine error: {}", e);
            *state_inner.loop_running.lock().await = false;
            *state_inner.current_attempt_id.lock().await = None;
            let _ = app_handle.emit(
                "loop:error",
                serde_json::json!({
                    "message": e.to_string(),
                }),
            );
            let _ = app_handle.emit(
                "loop:outer_complete",
                serde_json::json!({
                    "attempts_used": 0,
                    "error": e.to_string(),
                }),
            );
        }
    });

    tracing::info!(
        "Continuing solve for {} attempt {} from step {}",
        problem_id,
        attempt_id,
        starting_step
    );
    Ok(attempt_id)
}

#[tauri::command]
pub async fn pause_solve(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    *state.loop_running.lock().await = false;
    Ok(())
}

#[tauri::command]
pub async fn stop_solve(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    *state.loop_running.lock().await = false;
    *state.current_attempt_id.lock().await = None;
    Ok(())
}

#[tauri::command]
pub async fn get_loop_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<LoopStatusResponse, String> {
    let running = *state.loop_running.lock().await;
    let attempt_id = state.current_attempt_id.lock().await.clone();
    Ok(LoopStatusResponse {
        running,
        attempt_id,
    })
}

/// Manual review: run the post-attempt review prompt on demand (outside the loop).
/// Returns the parsed review result as JSON.
#[tauri::command]
pub async fn run_manual_review(
    state: tauri::State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    problem_id: String,
    config: Option<MultiAgentConfig>,
) -> Result<serde_json::Value, String> {
    let problem = state
        .db
        .get_problem(&problem_id)
        .map_err(|e| e.to_string())?;
    let all_steps = state
        .db
        .get_problem_steps(&problem_id)
        .map_err(|e| e.to_string())?;

    if all_steps.is_empty() {
        return Err("No steps to review".to_string());
    }

    // Build reviewer LLM
    let cfg = config.unwrap_or_default();
    let (provider, model, api_key, budget, temperature) =
        if let Some(ref rev_cfg) = cfg.reviewer_model {
            let key = crate::loop_engine::resolve_api_key(&rev_cfg.api_key_ref)?;
            (
                rev_cfg.provider.clone(),
                rev_cfg.model.clone(),
                key,
                rev_cfg.max_budget_tokens as u32,
                rev_cfg.temperature,
            )
        } else if let Some(model_cfg) = cfg.models.first() {
            let key = crate::loop_engine::resolve_api_key(&model_cfg.api_key_ref)?;
            (
                model_cfg.provider.clone(),
                model_cfg.model.clone(),
                key,
                model_cfg.max_budget_tokens as u32,
                model_cfg.temperature,
            )
        } else {
            return Err("No model configured for review".to_string());
        };

    let reviewer_llm =
        LlmClient::with_budget(&provider, &model, &api_key, budget.clamp(2_000, 16_000))
            .with_temperature(temperature);

    // Build enriched context for the reviewer (prior attempts, pattern stats, etc.)
    let analyst_context = context_enricher::ProofContext::build(
        &state.db,
        &problem_id,
        &problem.domain,
        "",            // no current attempt to exclude
        String::new(), // no scout briefing for manual review
    )
    .format_for_analyst();

    let review_prompt = review::build_review_prompt(
        &problem.statement,
        &all_steps,
        problem.known_answer.as_deref(),
        &analyst_context,
    );

    let _ = app_handle.emit(
        "loop:review_start",
        LoopReviewStartPayload {
            attempt_id: None,
            manual: Some(true),
            problem_id: Some(problem_id.clone()),
        },
    );

    let _ = app_handle.emit(
        "loop:thinking_start",
        LoopThinkingStartPayload {
            step_number: None,
            model: reviewer_llm.model_name(),
            agent_role: Some("reviewer".to_string()),
            obligation_id: None,
            review: Some(true),
            manual: Some(true),
        },
    );
    let review_handle = app_handle.clone();
    let review_resp = reviewer_llm
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
        .await
        .map_err(|e| format!("Review LLM error: {}", e))?;
    let _ = app_handle.emit(
        "loop:thinking_end",
        LoopThinkingEndPayload {
            obligation_id: None,
        },
    );

    let parsed =
        review::parse_review(&review_resp.text).ok_or("Failed to parse review response")?;

    tracing::info!(
        findings = parsed.findings.len(),
        conclusion_sound = parsed.conclusion_sound,
        label = %parsed.training_label,
        "Manual review complete"
    );

    // Record to DB
    if let Ok(attempt_id) = state.db.get_latest_attempt_id(&problem_id) {
        let _ = state.db.append_dag_event(
            &attempt_id,
            "review_completed",
            &serde_json::json!({
                "manual": true,
                "findings": parsed.findings.len(),
                "coverage": parsed.exploration_coverage,
                "label": &parsed.training_label,
                "conclusion_sound": parsed.conclusion_sound,
            })
            .to_string(),
            "reviewer",
        );

        if let Ok(session_id) = state.db.record_council_session(
            "manual_review",
            &problem_id,
            Some(&attempt_id),
            &reviewer_llm.model_name(),
            &review_resp.text,
            parsed.findings.len() as u32,
        ) {
            for finding in &parsed.findings {
                let _ = state.db.record_council_finding(
                    &session_id,
                    &finding.finding_type,
                    &finding.summary,
                    &finding.detail,
                    finding.target_agent.as_deref(),
                );
            }
        }
    }

    let result = serde_json::json!({
        "findings": parsed.findings.iter().map(|f| serde_json::json!({
            "type": f.finding_type,
            "summary": f.summary,
            "detail": f.detail,
            "priority": f.priority,
        })).collect::<Vec<_>>(),
        "conclusion_sound": parsed.conclusion_sound,
        "conclusion_confidence": parsed.conclusion_confidence,
        "exploration_coverage": parsed.exploration_coverage,
        "missing_constructions": parsed.missing_constructions,
        "training_label": parsed.training_label,
    });

    let _ = app_handle.emit(
        "loop:review_complete",
        ContractReviewResult {
            attempt_id: state
                .db
                .get_latest_attempt_id(&problem_id)
                .unwrap_or_default(),
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
            manual: Some(true),
            problem_id: Some(problem_id.clone()),
        },
    );

    let _ = app_handle.emit("loop:review_end", EmptyPayload {});

    Ok(result)
}
