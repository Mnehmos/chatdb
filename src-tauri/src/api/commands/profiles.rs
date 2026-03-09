use crate::models::agents::AgentProfile;
use crate::AppState;
use std::sync::Arc;

#[tauri::command]
pub fn save_profile(
    state: tauri::State<'_, Arc<AppState>>,
    name: String,
    description: Option<String>,
    config_json: String,
    is_default: Option<bool>,
) -> Result<AgentProfile, String> {
    tracing::info!(
        "save_profile called: name='{}', desc={:?}, is_default={:?}, json_len={}",
        name,
        description,
        is_default,
        config_json.len()
    );

    if name.trim().is_empty() {
        return Err("Profile name cannot be empty".to_string());
    }

    // Validate that config_json parses as MultiAgentConfig
    let parsed: crate::models::agents::MultiAgentConfig = serde_json::from_str(&config_json)
        .map_err(|e| {
            tracing::error!("save_profile: config_json validation failed: {}", e);
            tracing::error!("save_profile: config_json (first 500 chars): {}", &config_json[..config_json.len().min(500)]);
            format!("Invalid config JSON: {}. Check that all required fields are present and types match.", e)
        })?;

    // Sanity check: config must have at least one model
    if parsed.models.is_empty() {
        tracing::error!("save_profile: config has no models");
        return Err("Config must have at least one solver model".to_string());
    }

    // Re-serialize the parsed config to normalize it (strip unknown fields, fill defaults)
    let normalized = serde_json::to_string(&parsed).map_err(|e| {
        tracing::error!("save_profile: re-serialize failed: {}", e);
        format!("Config normalization failed: {}", e)
    })?;

    match state.db.save_profile(
        name.trim(),
        description.as_deref(),
        &normalized,
        is_default.unwrap_or(false),
    ) {
        Ok(profile) => {
            tracing::info!(
                "save_profile: saved id='{}', name='{}'",
                profile.id,
                profile.name
            );
            Ok(profile)
        }
        Err(e) => {
            tracing::error!("save_profile: DB write failed: {}", e);
            Err(format!("Database error saving profile: {}", e))
        }
    }
}

#[tauri::command]
pub fn load_profile(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<AgentProfile, String> {
    state.db.get_profile(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_profiles(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<AgentProfile>, String> {
    state.db.list_profiles().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_profile(state: tauri::State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.db.delete_profile(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_default_profile(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.db.set_default_profile(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_default_profile(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<AgentProfile>, String> {
    state.db.get_default_profile().map_err(|e| e.to_string())
}

/// Diagnostic: test the full profile save → read → delete round-trip.
/// Returns a JSON string with the test result details.
#[tauri::command]
pub fn test_profile_roundtrip(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let test_name = format!(
        "__diag_test_{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("x")
    );
    let test_config = crate::models::agents::MultiAgentConfig::default();
    let config_json =
        serde_json::to_string(&test_config).map_err(|e| format!("serialize: {}", e))?;

    // Save
    let saved = state
        .db
        .save_profile(&test_name, Some("diagnostic test"), &config_json, false)
        .map_err(|e| format!("save failed: {}", e))?;

    // Read back
    let loaded = state
        .db
        .get_profile(&saved.id)
        .map_err(|e| format!("read-back failed: {}", e))?;

    // Verify fields
    let mut issues: Vec<String> = Vec::new();
    if loaded.name != test_name {
        issues.push(format!(
            "name mismatch: '{}' vs '{}'",
            loaded.name, test_name
        ));
    }
    if loaded.config_json != config_json {
        issues.push("config_json mismatch after round-trip".to_string());
    }
    if loaded.description.as_deref() != Some("diagnostic test") {
        issues.push(format!("description mismatch: {:?}", loaded.description));
    }

    // List should include it
    let all = state
        .db
        .list_profiles()
        .map_err(|e| format!("list failed: {}", e))?;
    if !all.iter().any(|p| p.id == saved.id) {
        issues.push("saved profile not found in list".to_string());
    }

    // Delete
    state
        .db
        .delete_profile(&saved.id)
        .map_err(|e| format!("delete failed: {}", e))?;

    // Verify deletion
    let after_delete = state.db.list_profiles().unwrap_or_default();
    if after_delete.iter().any(|p| p.id == saved.id) {
        issues.push("profile still present after delete".to_string());
    }

    let result = serde_json::json!({
        "passed": issues.is_empty(),
        "issues": issues,
        "saved_id": saved.id,
        "config_json_len": config_json.len(),
        "profiles_count": all.len() - 1, // minus the test profile
    });

    Ok(result.to_string())
}
