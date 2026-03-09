pub mod api;
pub mod contracts;
pub mod db;
pub mod loop_engine;
pub mod models;
pub mod runtime_paths;
pub mod verification;

use db::Database;
use tokio::sync::Mutex;

pub struct AppState {
    pub db: Database,
    pub loop_running: Mutex<bool>,
    pub current_attempt_id: Mutex<Option<String>>,
    /// Stored during Tauri setup so the Axum control server (:9744) can emit events.
    pub app_handle: Mutex<Option<tauri::AppHandle>>,
}
