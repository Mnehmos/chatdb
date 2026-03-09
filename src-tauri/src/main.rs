#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chatdb::db::Database;
use chatdb::AppState;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn main() {
    // Load .env from both current dir AND parent (handles tauri dev CWD = src-tauri/).
    // Parent overrides local so the root .env is the source of truth.
    let _ = dotenvy::dotenv();
    let _ = dotenvy::from_path("../.env");

    // Rolling daily log file in logs/ directory
    let log_dir = std::path::Path::new("logs");
    std::fs::create_dir_all(log_dir).ok();
    let file_appender = tracing_appender::rolling::daily(log_dir, "chatdb.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,chatdb=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true).with_thread_ids(false))
        .with(
            fmt::layer()
                .with_target(true)
                .with_ansi(false)
                .with_writer(non_blocking),
        )
        .init();
    let db_path = chatdb::runtime_paths::resolve_chatdb_db_path();
    tracing::info!("Using ChatDB database at {}", db_path.display());
    let db = Database::new(
        db_path
            .to_str()
            .expect("ChatDB database path must be valid UTF-8"),
    )
    .expect("Failed to initialize ChatDB");

    // Startup cleanup: mark stale "active" attempts as "stopped" and reset
    // "assigned" obligations back to "open" — these are leftovers from a crash.
    match db.cleanup_stale_on_startup() {
        Ok((attempts, obligations)) if attempts > 0 || obligations > 0 => {
            tracing::info!(
                "Startup cleanup: {} stale attempts stopped, {} assigned obligations reset",
                attempts,
                obligations
            );
        }
        Err(e) => tracing::warn!("Startup cleanup failed: {}", e),
        _ => {}
    }

    // Register Wolfram Alpha API key from env if available
    if let Ok(wolfram_key) = std::env::var("WOLFRAM_ALPHA_APP_ID") {
        match db.set_research_api_key("wolfram_alpha", &wolfram_key, Some("Wolfram App")) {
            Ok(_) => tracing::info!("Wolfram Alpha API key registered from env"),
            Err(e) => tracing::warn!("Failed to register Wolfram Alpha key: {}", e),
        }
    }

    let state = Arc::new(AppState {
        db,
        loop_running: Mutex::new(false),
        current_attempt_id: Mutex::new(None),
        app_handle: Mutex::new(None),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .setup(|app| {
            let handle = app.handle().clone();
            let state: tauri::State<'_, Arc<AppState>> = app.state();
            let state_clone = state.inner().clone();

            // Store AppHandle so the Axum control server can emit Tauri events.
            tauri::async_runtime::spawn(async move {
                *state_clone.app_handle.lock().await = Some(handle);
            });

            // Spawn the Axum control server on :9744 for MCP loop control.
            let state_for_axum: Arc<AppState> = app.state::<Arc<AppState>>().inner().clone();
            tauri::async_runtime::spawn(async move {
                let router = chatdb::api::control::control_router().with_state(state_for_axum);
                match tokio::net::TcpListener::bind("127.0.0.1:9744").await {
                    Ok(listener) => {
                        tracing::info!("Control server listening on :9744");
                        if let Err(e) = axum::serve(listener, router).await {
                            tracing::error!("Control server error: {}", e);
                        }
                    }
                    Err(e) => tracing::warn!(
                        "Could not bind :9744 ({}). MCP loop control unavailable.",
                        e
                    ),
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            chatdb::api::commands::problem::create_problem,
            chatdb::api::commands::problem::get_problem,
            chatdb::api::commands::problem::list_problems,
            chatdb::api::commands::loop_cmd::start_solve,
            chatdb::api::commands::loop_cmd::continue_solve,
            chatdb::api::commands::loop_cmd::pause_solve,
            chatdb::api::commands::loop_cmd::stop_solve,
            chatdb::api::commands::loop_cmd::get_loop_status,
            chatdb::api::commands::loop_cmd::run_manual_review,
            chatdb::api::commands::proof::get_verified_chain,
            chatdb::api::commands::proof::get_problem_steps,
            chatdb::api::commands::proof::get_attempt_steps,
            chatdb::api::commands::proof::get_obligation_graph,
            chatdb::api::commands::proof::get_obligation_detail,
            chatdb::api::commands::proof::get_obligation_proof_nodes,
            chatdb::api::commands::proof::get_obligation_signals,
            chatdb::api::commands::patterns::search_patterns,
            chatdb::api::commands::analytics::get_training_data_stats,
            chatdb::api::commands::analytics::list_all_steps,
            chatdb::api::commands::analytics::get_after_action_report,
            chatdb::api::commands::analytics::formalize_proof,
            chatdb::api::commands::profiles::save_profile,
            chatdb::api::commands::profiles::load_profile,
            chatdb::api::commands::profiles::list_profiles,
            chatdb::api::commands::profiles::delete_profile,
            chatdb::api::commands::profiles::set_default_profile,
            chatdb::api::commands::profiles::get_default_profile,
            chatdb::api::commands::profiles::test_profile_roundtrip,
            chatdb::api::commands::diagnostics::get_diagnostic_events,
            chatdb::api::commands::diagnostics::get_system_health,
            // Management System commands
            chatdb::api::commands::management::list_attempts,
            chatdb::api::commands::management::get_claims_for_attempt,
            chatdb::api::commands::management::get_claims_for_step,
            chatdb::api::commands::management::get_conflicts_for_attempt,
            chatdb::api::commands::management::get_dag_edges_from,
            chatdb::api::commands::management::get_dag_edges_to,
            chatdb::api::commands::management::get_report_for_attempt,
            chatdb::api::commands::management::get_reports_for_problem,
            chatdb::api::commands::management::search_problems,
            chatdb::api::commands::management::update_problem_title,
            chatdb::api::commands::management::backfill_dag_edges,
            chatdb::api::commands::management::delete_attempt,
            chatdb::api::commands::management::get_tool_runs_for_step,
            chatdb::api::commands::management::get_tool_runs_for_obligation,
            // Export commands
            chatdb::api::commands::export::export_training_data,
            chatdb::api::commands::export::get_export_directory,
            // Research API commands
            chatdb::api::commands::research::set_research_api_key,
            chatdb::api::commands::research::list_research_api_keys,
            chatdb::api::commands::research::delete_research_api_key,
            chatdb::api::commands::research::toggle_research_api_key,
            chatdb::api::commands::research::research_search,
            chatdb::api::commands::research::research_get,
            chatdb::api::commands::research::research_multi,
            chatdb::api::commands::research::research_sources,
            // ChatGPT OAuth commands
            chatdb::api::commands::oauth::chatgpt_oauth_start,
            chatdb::api::commands::oauth::chatgpt_oauth_poll,
            chatdb::api::commands::oauth::chatgpt_oauth_status,
            chatdb::api::commands::oauth::chatgpt_oauth_logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ChatDB");
}
