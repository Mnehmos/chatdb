use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// OpenAI device-code OAuth flow for ChatGPT subscription users.
/// Derived from LiteLLM/Codex CLI implementation.
///
/// Flow:
/// 1. POST /api/accounts/deviceauth/usercode → device_auth_id + user_code
/// 2. User visits https://auth.openai.com/codex/device and enters code
/// 3. Poll POST /api/accounts/deviceauth/token → authorization_code + code_verifier
/// 4. Exchange POST /oauth/token → access_token + refresh_token + id_token

const AUTH_BASE: &str = "https://auth.openai.com";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const CHATGPT_API_BASE: &str = "https://chatgpt.com/backend-api/codex";
pub const CHATGPT_VERIFY_URL: &str = "https://auth.openai.com/codex/device";
const TOKEN_FILE: &str = "chatgpt_oauth.json";

fn build_client() -> Client {
    Client::builder().build().unwrap_or_default()
}

/// Stored token state — persisted to disk and loaded on startup.
static TOKEN_STATE: Lazy<Mutex<Option<TokenState>>> =
    Lazy::new(|| Mutex::new(load_token_from_disk()));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenState {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_at: Option<i64>,
    pub account_id: Option<String>,
}

// Step 1 response
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_auth_id: String,
    pub user_code: String,
    pub verification_url: String,
    pub interval: u64,
}

// Step 1: internal API response
#[derive(Debug, Deserialize)]
struct DeviceCodeApiResponse {
    device_auth_id: String,
    user_code: Option<String>,
    usercode: Option<String>,
    interval: Option<serde_json::Value>, // can be string or number
    expires_at: Option<String>,
}

// Step 3: poll response (intermediate)
#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    authorization_code: Option<String>,
    code_challenge: Option<String>,
    code_verifier: Option<String>,
}

// Step 4: final token response
#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<u64>,
}

/// Step 1: Request a device code from OpenAI.
pub async fn start_device_flow() -> Result<DeviceCodeResponse, String> {
    let client = build_client();
    let url = format!("{AUTH_BASE}/api/accounts/deviceauth/usercode");

    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "client_id": CLIENT_ID }))
        .send()
        .await
        .map_err(|e| format!("Device code request failed: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Device code request returned error: {body}"));
    }

    let api_resp: DeviceCodeApiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse device code response: {e}"))?;

    let user_code = api_resp
        .user_code
        .or(api_resp.usercode)
        .ok_or_else(|| "No user_code in device code response".to_string())?;

    let interval = match api_resp.interval {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(5),
        Some(serde_json::Value::String(s)) => s.parse::<u64>().unwrap_or(5),
        _ => 5,
    };

    Ok(DeviceCodeResponse {
        device_auth_id: api_resp.device_auth_id,
        user_code,
        verification_url: CHATGPT_VERIFY_URL.to_string(),
        interval,
    })
}

/// Step 2+3: Poll for authorization, then exchange for tokens.
/// Returns Ok(Some(token)) when complete, Ok(None) when still pending.
pub async fn poll_for_token(
    device_auth_id: &str,
    user_code: &str,
) -> Result<Option<TokenState>, String> {
    let client = build_client();

    // Poll the device token endpoint
    let poll_url = format!("{AUTH_BASE}/api/accounts/deviceauth/token");
    let resp = client
        .post(&poll_url)
        .json(&serde_json::json!({
            "device_auth_id": device_auth_id,
            "user_code": user_code,
        }))
        .send()
        .await
        .map_err(|e| format!("Token poll failed: {e}"))?;

    let status = resp.status();

    // 403/404 = still pending
    if status.as_u16() == 403 || status.as_u16() == 404 {
        return Ok(None);
    }

    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Token poll returned {status}: {body}"));
    }

    let poll_resp: DeviceTokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse poll response: {e}"))?;

    // Check if we have all required fields
    let auth_code = match poll_resp.authorization_code {
        Some(c) => c,
        None => return Ok(None), // Not ready yet
    };
    let code_verifier = poll_resp
        .code_verifier
        .ok_or_else(|| "Missing code_verifier in poll response".to_string())?;

    // Exchange authorization code for tokens
    let token_url = format!("{AUTH_BASE}/oauth/token");
    let redirect_uri = format!("{AUTH_BASE}/deviceauth/callback");

    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
        urlencoding::encode(&auth_code),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(CLIENT_ID),
        urlencoding::encode(&code_verifier),
    );

    let token_resp = client
        .post(&token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Token exchange failed: {e}"))?;

    if !token_resp.status().is_success() {
        let err_body = token_resp.text().await.unwrap_or_default();
        return Err(format!("Token exchange error: {err_body}"));
    }

    let tokens: OAuthTokenResponse = token_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {e}"))?;

    let expires_at = tokens
        .expires_in
        .map(|secs| chrono::Utc::now().timestamp() + secs as i64);

    let account_id = extract_account_id(tokens.id_token.as_deref().or(Some(&tokens.access_token)));

    let state = TokenState {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        id_token: tokens.id_token,
        expires_at,
        account_id,
    };

    // Store in memory + disk
    if let Ok(mut guard) = TOKEN_STATE.lock() {
        *guard = Some(state.clone());
    }
    save_token_to_disk(&state);

    // Set env var for key resolution
    std::env::set_var("CHATGPT_OAUTH_TOKEN", &state.access_token);

    Ok(Some(state))
}

/// Refresh an expired access token using the stored refresh token.
pub async fn refresh_token() -> Result<TokenState, String> {
    let refresh_tok = {
        let guard = TOKEN_STATE.lock().map_err(|e| format!("Lock error: {e}"))?;
        guard
            .as_ref()
            .and_then(|s| s.refresh_token.clone())
            .ok_or_else(|| "No refresh token available".to_string())?
    };

    let client = build_client();
    let token_url = format!("{AUTH_BASE}/oauth/token");

    let resp = client
        .post(&token_url)
        .json(&serde_json::json!({
            "client_id": CLIENT_ID,
            "grant_type": "refresh_token",
            "refresh_token": refresh_tok,
            "scope": "openid profile email",
        }))
        .send()
        .await
        .map_err(|e| format!("Refresh request failed: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Refresh failed: {body}"));
    }

    let tokens: OAuthTokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse refresh response: {e}"))?;

    let expires_at = tokens
        .expires_in
        .map(|secs| chrono::Utc::now().timestamp() + secs as i64);

    let account_id = extract_account_id(tokens.id_token.as_deref().or(Some(&tokens.access_token)));

    let state = TokenState {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token.or(Some(refresh_tok)),
        id_token: tokens.id_token,
        expires_at,
        account_id,
    };

    if let Ok(mut guard) = TOKEN_STATE.lock() {
        *guard = Some(state.clone());
    }
    save_token_to_disk(&state);
    std::env::set_var("CHATGPT_OAUTH_TOKEN", &state.access_token);

    Ok(state)
}

/// Get the current access token, refreshing if expired.
pub async fn get_access_token() -> Result<String, String> {
    let state = {
        let guard = TOKEN_STATE.lock().map_err(|e| format!("Lock error: {e}"))?;
        guard.clone()
    };

    match state {
        Some(s) => {
            if let Some(exp) = s.expires_at {
                if chrono::Utc::now().timestamp() > exp - 60 {
                    let refreshed = refresh_token().await?;
                    return Ok(refreshed.access_token);
                }
            }
            Ok(s.access_token)
        }
        None => std::env::var("CHATGPT_OAUTH_TOKEN")
            .map_err(|_| "No ChatGPT OAuth token. Please authenticate first.".to_string()),
    }
}

/// Synchronous token getter for key resolution at startup.
/// Returns the stored access token without attempting refresh.
pub fn get_token_sync() -> Option<String> {
    if let Ok(guard) = TOKEN_STATE.lock() {
        if let Some(ref s) = *guard {
            return Some(s.access_token.clone());
        }
    }
    std::env::var("CHATGPT_OAUTH_TOKEN").ok()
}

/// Get the account ID for API requests.
pub fn get_account_id() -> Option<String> {
    if let Ok(guard) = TOKEN_STATE.lock() {
        guard.as_ref().and_then(|s| s.account_id.clone())
    } else {
        None
    }
}

/// Get the expiry timestamp of the current token.
pub fn token_expires_at() -> Option<i64> {
    if let Ok(guard) = TOKEN_STATE.lock() {
        guard.as_ref().and_then(|s| s.expires_at)
    } else {
        None
    }
}

/// Check if we have a valid token stored.
pub fn is_authenticated() -> bool {
    if let Ok(guard) = TOKEN_STATE.lock() {
        if guard.is_some() {
            return true;
        }
    }
    std::env::var("CHATGPT_OAUTH_TOKEN").is_ok()
}

/// Clear stored token (logout).
pub fn clear_token() {
    if let Ok(mut guard) = TOKEN_STATE.lock() {
        *guard = None;
    }
    std::env::remove_var("CHATGPT_OAUTH_TOKEN");
    remove_token_from_disk();
}

/// Extract account_id from JWT claims.
fn extract_account_id(token: Option<&str>) -> Option<String> {
    let token = token?;
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    // Pad base64
    let mut payload = parts[1].to_string();
    let pad = 4 - payload.len() % 4;
    if pad < 4 {
        payload.push_str(&"=".repeat(pad));
    }
    let bytes = base64_decode(&payload)?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .get("https://api.openai.com/auth")
        .and_then(|a| a.get("chatgpt_account_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Save token state to disk so it survives app restarts.
fn save_token_to_disk(state: &TokenState) {
    match serde_json::to_string_pretty(state) {
        Ok(json) => {
            if let Err(e) = std::fs::write(TOKEN_FILE, json) {
                tracing::warn!("Failed to save OAuth token to disk: {e}");
            }
        }
        Err(e) => tracing::warn!("Failed to serialize OAuth token: {e}"),
    }
}

/// Load token state from disk on startup.
fn load_token_from_disk() -> Option<TokenState> {
    let data = std::fs::read_to_string(TOKEN_FILE).ok()?;
    let state: TokenState = serde_json::from_str(&data).ok()?;
    // Set env var so key resolution works immediately
    std::env::set_var("CHATGPT_OAUTH_TOKEN", &state.access_token);
    tracing::info!("Restored ChatGPT OAuth token from disk");
    Some(state)
}

/// Remove persisted token file on logout.
fn remove_token_from_disk() {
    let _ = std::fs::remove_file(TOKEN_FILE);
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(input.trim_end_matches('='))
        .ok()
}
