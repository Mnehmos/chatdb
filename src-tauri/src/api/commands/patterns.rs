use crate::models::patterns::Pattern;
use crate::AppState;
use std::sync::Arc;

#[tauri::command]
pub fn search_patterns(
    state: tauri::State<'_, Arc<AppState>>,
    query: String,
) -> Result<Vec<Pattern>, String> {
    state.db.search_patterns(&query).map_err(|e| e.to_string())
}
