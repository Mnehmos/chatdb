use crate::models::management::{
    AfterActionReportRecord, AttemptSummary, Claim, Conflict, DagEdge,
};
use crate::models::proof::Problem;
use crate::AppState;
use std::sync::Arc;

#[tauri::command]
pub fn list_attempts(
    state: tauri::State<'_, Arc<AppState>>,
    problem_id: String,
) -> Result<Vec<AttemptSummary>, String> {
    state
        .db
        .list_attempts_for_problem(&problem_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_claims_for_attempt(
    state: tauri::State<'_, Arc<AppState>>,
    attempt_id: String,
) -> Result<Vec<Claim>, String> {
    state
        .db
        .get_claims_for_attempt(&attempt_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_claims_for_step(
    state: tauri::State<'_, Arc<AppState>>,
    step_id: String,
) -> Result<Vec<Claim>, String> {
    state
        .db
        .get_claims_for_step(&step_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_conflicts_for_attempt(
    state: tauri::State<'_, Arc<AppState>>,
    attempt_id: String,
) -> Result<Vec<Conflict>, String> {
    state
        .db
        .get_conflicts_for_attempt(&attempt_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_dag_edges_from(
    state: tauri::State<'_, Arc<AppState>>,
    source_id: String,
) -> Result<Vec<DagEdge>, String> {
    state
        .db
        .get_edges_from(&source_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_dag_edges_to(
    state: tauri::State<'_, Arc<AppState>>,
    target_id: String,
) -> Result<Vec<DagEdge>, String> {
    state.db.get_edges_to(&target_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_report_for_attempt(
    state: tauri::State<'_, Arc<AppState>>,
    attempt_id: String,
) -> Result<AfterActionReportRecord, String> {
    state
        .db
        .get_report_for_attempt(&attempt_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_reports_for_problem(
    state: tauri::State<'_, Arc<AppState>>,
    problem_id: String,
) -> Result<Vec<AfterActionReportRecord>, String> {
    state
        .db
        .get_reports_for_problem(&problem_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_problems(
    state: tauri::State<'_, Arc<AppState>>,
    query: String,
    domain: Option<String>,
    status: Option<String>,
    difficulty: Option<String>,
) -> Result<Vec<Problem>, String> {
    state
        .db
        .search_problems(
            &query,
            domain.as_deref(),
            status.as_deref(),
            difficulty.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_problem_title(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    title: String,
) -> Result<(), String> {
    state
        .db
        .update_problem_title(&id, &title)
        .map_err(|e| e.to_string())
}

/// Backfill dag_edges from existing steps and proof_nodes.
/// Creates depends_on edges from parent relationships and targets edges
/// from obligation_ref fields. Safe to re-run (INSERT OR IGNORE).
#[tauri::command]
pub fn backfill_dag_edges(state: tauri::State<'_, Arc<AppState>>) -> Result<u32, String> {
    let conn = state.db.conn();
    let mut count = 0u32;

    // 1. Step → parent step edges from proof_nodes.parent_ids
    {
        let mut stmt = conn.prepare(
            "SELECT id, parent_ids FROM proof_nodes WHERE parent_ids IS NOT NULL AND parent_ids != ''"
        ).map_err(|e| e.to_string())?;
        let nodes: Vec<(String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        for (node_id, parent_ids_json) in &nodes {
            if let Ok(parent_ids) = serde_json::from_str::<Vec<String>>(parent_ids_json) {
                for pid in &parent_ids {
                    let _ = state
                        .db
                        .create_edge(node_id, pid, "step", "step", "depends_on", None);
                    count += 1;
                }
            }
        }
    }

    // 2. Step → obligation edges from steps.obligation_id (if column exists)
    {
        let result = conn
            .prepare("SELECT s.id, s.obligation_id FROM steps s WHERE s.obligation_id IS NOT NULL");
        if let Ok(mut stmt) = result {
            let rows: Vec<(String, String)> = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            for (step_id, ob_id) in &rows {
                let _ = state
                    .db
                    .create_edge(step_id, ob_id, "step", "obligation", "targets", None);
                count += 1;
            }
        }
    }

    // 3. Obligation → obligation edges from obligations.depends_on
    {
        let result = conn.prepare(
            "SELECT id, depends_on FROM obligations WHERE depends_on IS NOT NULL AND depends_on != ''"
        );
        if let Ok(mut stmt) = result {
            let rows: Vec<(String, String)> = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            for (ob_id, depends_json) in &rows {
                if let Ok(deps) = serde_json::from_str::<Vec<String>>(depends_json) {
                    for dep_id in &deps {
                        let _ = state.db.create_edge(
                            ob_id,
                            dep_id,
                            "obligation",
                            "obligation",
                            "depends_on",
                            None,
                        );
                        count += 1;
                    }
                }
            }
        }
    }

    tracing::info!("Backfill dag_edges: created {} edges", count);
    Ok(count)
}

/// Delete an attempt and all associated data (steps, obligations, proof_nodes, etc.)
#[tauri::command]
pub fn delete_attempt(
    state: tauri::State<'_, Arc<AppState>>,
    attempt_id: String,
) -> Result<(), String> {
    state
        .db
        .delete_attempt(&attempt_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_tool_runs_for_step(
    state: tauri::State<'_, Arc<AppState>>,
    step_id: String,
) -> Result<Vec<crate::models::tool_use::ToolRun>, String> {
    state
        .db
        .get_tool_runs_for_step(&step_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_tool_runs_for_obligation(
    state: tauri::State<'_, Arc<AppState>>,
    obligation_id: String,
) -> Result<Vec<crate::models::tool_use::ToolRun>, String> {
    state
        .db
        .get_tool_runs_for_obligation(&obligation_id)
        .map_err(|e| e.to_string())
}
