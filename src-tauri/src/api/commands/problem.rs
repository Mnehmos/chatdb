use crate::models::proof::Problem;
use crate::AppState;
use std::sync::Arc;

#[tauri::command]
pub fn create_problem(
    state: tauri::State<'_, Arc<AppState>>,
    statement: String,
    domain: String,
) -> Result<Problem, String> {
    state
        .db
        .create_problem(&statement, &domain, "user")
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_problem(state: tauri::State<'_, Arc<AppState>>, id: String) -> Result<Problem, String> {
    state.db.get_problem(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_problems(
    state: tauri::State<'_, Arc<AppState>>,
    status: Option<String>,
) -> Result<Vec<Problem>, String> {
    state
        .db
        .list_problems(status.as_deref())
        .map_err(|e| e.to_string())
}
