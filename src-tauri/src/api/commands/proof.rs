use crate::db::signals::SatisfactionSignal;
use crate::models::dag::{Obligation, ProofNode};
use crate::models::proof::Step;
use crate::AppState;
use std::sync::Arc;

#[tauri::command]
pub fn get_verified_chain(
    state: tauri::State<'_, Arc<AppState>>,
    attempt_id: String,
) -> Result<Vec<(String, u32, String, String, String)>, String> {
    state
        .db
        .get_verified_chain(&attempt_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_problem_steps(
    state: tauri::State<'_, Arc<AppState>>,
    problem_id: String,
) -> Result<Vec<Step>, String> {
    state
        .db
        .get_problem_steps(&problem_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_attempt_steps(
    state: tauri::State<'_, Arc<AppState>>,
    attempt_id: String,
) -> Result<Vec<Step>, String> {
    state
        .db
        .get_attempt_steps(&attempt_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_obligation_graph(
    state: tauri::State<'_, Arc<AppState>>,
    attempt_id: String,
) -> Result<Vec<Obligation>, String> {
    state
        .db
        .get_all_obligations(&attempt_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_obligation_detail(
    state: tauri::State<'_, Arc<AppState>>,
    obligation_id: String,
) -> Result<Obligation, String> {
    state
        .db
        .get_obligation(&obligation_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_obligation_proof_nodes(
    state: tauri::State<'_, Arc<AppState>>,
    obligation_id: String,
) -> Result<Vec<ProofNode>, String> {
    state
        .db
        .get_nodes_for_obligation(&obligation_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_obligation_signals(
    state: tauri::State<'_, Arc<AppState>>,
    obligation_id: String,
) -> Result<Vec<SatisfactionSignal>, String> {
    state
        .db
        .get_obligation_signals(&obligation_id)
        .map_err(|e| e.to_string())
}
