use serde::Serialize;

#[derive(Serialize)]
pub struct DeviceFlowInfo {
    pub device_auth_id: String,
    pub user_code: String,
    pub verification_url: String,
    pub interval: u64,
}

#[derive(Serialize)]
pub struct OAuthStatus {
    pub authenticated: bool,
    pub expires_at: Option<i64>,
}

#[derive(Serialize)]
pub struct PollResult {
    pub complete: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn chatgpt_oauth_start() -> Result<DeviceFlowInfo, String> {
    let resp = crate::api::chatgpt_oauth::start_device_flow().await?;
    Ok(DeviceFlowInfo {
        device_auth_id: resp.device_auth_id,
        user_code: resp.user_code,
        verification_url: resp.verification_url,
        interval: resp.interval,
    })
}

#[tauri::command]
pub async fn chatgpt_oauth_poll(
    device_auth_id: String,
    user_code: String,
) -> Result<PollResult, String> {
    match crate::api::chatgpt_oauth::poll_for_token(&device_auth_id, &user_code).await {
        Ok(Some(_)) => Ok(PollResult {
            complete: true,
            error: None,
        }),
        Ok(None) => Ok(PollResult {
            complete: false,
            error: None,
        }),
        Err(e) => Ok(PollResult {
            complete: false,
            error: Some(e),
        }),
    }
}

#[tauri::command]
pub fn chatgpt_oauth_status() -> OAuthStatus {
    OAuthStatus {
        authenticated: crate::api::chatgpt_oauth::is_authenticated(),
        expires_at: crate::api::chatgpt_oauth::token_expires_at(),
    }
}

#[tauri::command]
pub fn chatgpt_oauth_logout() {
    crate::api::chatgpt_oauth::clear_token();
}
