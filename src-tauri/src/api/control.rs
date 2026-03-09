/// Axum HTTP control server — runs on :9744 alongside the Tauri window.
///
/// Exposes a minimal control plane for the MCP server and external tools:
///   POST /loop/start   { problem_id, config? }  → { attempt_id }
///   POST /loop/stop    {}                         → { stopped: true }
///   POST /loop/pause   {}                         → { paused: true }
///   GET  /loop/status                             → { running, attempt_id?, step_number }
///
/// The Tauri app must be running for loop control to work. The MCP server
/// falls back gracefully when this server is unreachable.
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::loop_engine::LoopEngine;
use crate::models::agents::MultiAgentConfig;
use crate::AppState;

type SharedState = Arc<AppState>;

#[derive(Debug, Deserialize)]
pub struct StartRequest {
    pub problem_id: String,
    pub config: Option<MultiAgentConfig>,
}

#[derive(Debug, Serialize)]
pub struct LoopStatusResponse {
    pub running: bool,
    pub attempt_id: Option<String>,
    pub step_number: u32,
}

pub fn control_router() -> Router<SharedState> {
    Router::new()
        .route("/loop/start", post(start_handler))
        .route("/loop/stop", post(stop_handler))
        .route("/loop/pause", post(pause_handler))
        .route("/loop/status", get(status_handler))
        .layer(CorsLayer::permissive())
}

async fn start_handler(
    State(state): State<SharedState>,
    Json(req): Json<StartRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut running = state.loop_running.lock().await;
    if *running {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "Loop already running" })),
        ));
    }
    *running = true;
    drop(running);

    let cfg = req.config.unwrap_or_default();
    let models: Vec<String> = cfg
        .models
        .iter()
        .map(|m| format!("{}/{}", m.provider, m.model))
        .collect();

    let attempt_id = state
        .db
        .create_attempt(&req.problem_id, &models)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        })?;

    *state.current_attempt_id.lock().await = Some(attempt_id.clone());

    // Retrieve the stored AppHandle to emit Tauri events from the loop engine.
    let app_handle_opt = state.app_handle.lock().await.clone();
    let Some(app_handle) = app_handle_opt else {
        // App handle not yet set — loop would have no event bus.
        *state.loop_running.lock().await = false;
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "Tauri AppHandle not ready — try again in a moment" })),
        ));
    };

    let attempt_id_clone = attempt_id.clone();
    let state_inner = state.clone();
    let problem_id = req.problem_id.clone();

    tokio::spawn(async move {
        if let Err(e) =
            LoopEngine::run_outer_loop(state_inner, cfg, problem_id, attempt_id_clone, app_handle)
                .await
        {
            tracing::error!("Control-plane loop error: {}", e);
        }
    });

    tracing::info!("Control-plane: started solve attempt {}", attempt_id);
    Ok(Json(json!({ "attempt_id": attempt_id })))
}

async fn stop_handler(State(state): State<SharedState>) -> Json<Value> {
    *state.loop_running.lock().await = false;
    *state.current_attempt_id.lock().await = None;
    Json(json!({ "stopped": true }))
}

async fn pause_handler(State(state): State<SharedState>) -> Json<Value> {
    *state.loop_running.lock().await = false;
    Json(json!({ "paused": true }))
}

async fn status_handler(State(state): State<SharedState>) -> Json<LoopStatusResponse> {
    let running = *state.loop_running.lock().await;
    let attempt_id = state.current_attempt_id.lock().await.clone();

    let step_number = if let Some(ref aid) = attempt_id {
        state.db.get_max_step_number(aid).unwrap_or(0)
    } else {
        0
    };

    Json(LoopStatusResponse {
        running,
        attempt_id,
        step_number,
    })
}
