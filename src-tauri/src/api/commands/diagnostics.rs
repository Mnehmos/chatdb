use crate::models::dag::DagEvent;
use crate::AppState;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct SystemHealth {
    pub sidecar_reachable: bool,
    pub lean_available: bool,
    pub lean_ready: bool,
    pub lean_warming_up: bool,
    pub lean_warmup_attempts: u32,
    pub active_attempt: Option<String>,
    pub loop_running: bool,
}

#[tauri::command]
pub fn get_diagnostic_events(
    state: tauri::State<'_, Arc<AppState>>,
    attempt_id: String,
    since_sequence: Option<i64>,
) -> Result<Vec<DagEvent>, String> {
    state
        .db
        .get_dag_events(&attempt_id, since_sequence.unwrap_or(0))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_system_health(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SystemHealth, String> {
    let loop_running = *state.loop_running.lock().await;
    let active_attempt = state.current_attempt_id.lock().await.clone();

    // Check sidecar health
    let sidecar = crate::api::sidecar::SidecarClient::new();
    let (sidecar_reachable, lean_available, lean_ready, lean_warming_up, lean_warmup_attempts) =
        match sidecar.health_extended().await {
            Ok(h) => (
                true,
                h.lean_available,
                h.lean_ready,
                h.lean_warming_up,
                h.lean_warmup_attempts,
            ),
            Err(_) => (false, false, false, false, 0),
        };

    Ok(SystemHealth {
        sidecar_reachable,
        lean_available,
        lean_ready,
        lean_warming_up,
        lean_warmup_attempts,
        active_attempt,
        loop_running,
    })
}
