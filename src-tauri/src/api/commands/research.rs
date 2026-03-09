use crate::api::sidecar::{ResearchSearchRequest, SidecarClient};
use crate::db::research::ResearchApiKey;
use crate::AppState;
use std::sync::Arc;

// ---- API Key Management ----

/// Masked key info for frontend display (never expose full keys).
#[derive(serde::Serialize)]
pub struct ResearchApiKeyInfo {
    pub id: String,
    pub service: String,
    pub key_masked: String,
    pub label: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "*".repeat(key.len())
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

fn to_info(k: ResearchApiKey) -> ResearchApiKeyInfo {
    ResearchApiKeyInfo {
        id: k.id,
        service: k.service,
        key_masked: mask_key(&k.key_value),
        label: k.label,
        active: k.active,
        created_at: k.created_at,
        updated_at: k.updated_at,
    }
}

#[tauri::command]
pub fn set_research_api_key(
    state: tauri::State<'_, Arc<AppState>>,
    service: String,
    key_value: String,
    label: Option<String>,
) -> Result<ResearchApiKeyInfo, String> {
    if service.trim().is_empty() || key_value.trim().is_empty() {
        return Err("Service and key value are required".to_string());
    }
    state
        .db
        .set_research_api_key(service.trim(), key_value.trim(), label.as_deref())
        .map(to_info)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_research_api_keys(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<ResearchApiKeyInfo>, String> {
    state
        .db
        .list_research_api_keys()
        .map(|keys| keys.into_iter().map(to_info).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_research_api_key(
    state: tauri::State<'_, Arc<AppState>>,
    service: String,
) -> Result<(), String> {
    state
        .db
        .delete_research_api_key(&service)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_research_api_key(
    state: tauri::State<'_, Arc<AppState>>,
    service: String,
    active: bool,
) -> Result<(), String> {
    state
        .db
        .toggle_research_api_key(&service, active)
        .map_err(|e| e.to_string())
}

// ---- Research Queries (delegated to sidecar) ----

#[tauri::command]
pub async fn research_search(
    source: String,
    query: String,
    max_results: Option<u32>,
    sort: Option<String>,
) -> Result<serde_json::Value, String> {
    let client = SidecarClient::new();
    client
        .research_search(&ResearchSearchRequest {
            source,
            query,
            max_results,
            sort,
            year: None,
            categories: None,
            fields_of_study: None,
            task: None,
            format_type: None,
            db: None,
        })
        .await
        .map_err(|e| format!("Sidecar: {}", e))
}

#[tauri::command]
pub async fn research_get(source: String, id: String) -> Result<serde_json::Value, String> {
    let client = SidecarClient::new();
    client
        .research_get(&source, &id)
        .await
        .map_err(|e| format!("Sidecar: {}", e))
}

#[tauri::command]
pub async fn research_multi(
    query: String,
    sources: Vec<String>,
    max_per_source: Option<u32>,
) -> Result<serde_json::Value, String> {
    let client = SidecarClient::new();
    client
        .research_multi(&query, &sources, max_per_source.unwrap_or(5))
        .await
        .map_err(|e| format!("Sidecar: {}", e))
}

#[tauri::command]
pub async fn research_sources() -> Result<serde_json::Value, String> {
    let client = SidecarClient::new();
    client
        .research_sources()
        .await
        .map_err(|e| format!("Sidecar: {}", e))
}
