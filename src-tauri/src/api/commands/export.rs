use crate::AppState;
use serde::Serialize;
use std::sync::Arc;

/// A single JSONL training record for step verification pairs.
#[derive(Serialize)]
struct StepVerificationRecord {
    problem_statement: String,
    problem_domain: String,
    step_number: u32,
    model: String,
    goal_state: String,
    proposal_type: String,
    proposal_natural: String,
    proposal_formal: Option<String>,
    proposal_reasoning: Option<String>,
    prior_chain: Vec<ChainContext>,
    verified: bool,
    rejection_reason: Option<String>,
    sympy_passed: Option<bool>,
    pint_passed: Option<bool>,
    lean_passed: Option<bool>,
    obligation_id: Option<String>,
    obligation_desc: Option<String>,
    obligation_type: Option<String>,
    stale_sibling: bool,
    semantic_redundant: bool,
    challenge_flaw_found: Option<bool>,
    challenge_attack: Option<String>,
    challenge_confidence: Option<f64>,
    challenge_fatal: Option<bool>,
}

#[derive(Clone, Serialize)]
struct ChainContext {
    step_number: u32,
    proposal_natural: String,
    proposal_formal: Option<String>,
}

/// A single JSONL training record for planning decisions.
#[derive(Serialize)]
struct PlanningRecord {
    problem_statement: String,
    problem_domain: String,
    attempt_id: String,
    obligation_id: String,
    obligation_type: String,
    obligation_description: String,
    priority: f64,
    status: String,
    steps_spent: i32,
    max_steps: i32,
    depends_on: Option<String>,
}

/// A single JSONL training record for contradiction detection.
#[derive(Serialize)]
struct ContradictionRecord {
    problem_statement: String,
    attempt_id: String,
    claim_a: ClaimSummary,
    claim_b: ClaimSummary,
    conflict_type: String,
    severity: String,
    description: String,
    resolution: Option<String>,
}

#[derive(Serialize)]
struct ClaimSummary {
    claim_type: String,
    object: String,
    natural_text: String,
    confidence: f64,
    verified: bool,
}

/// A single JSONL training record for accepted-vs-rejected step pairs.
#[derive(Serialize)]
struct ContrastiveRecord {
    problem_id: String,
    problem_statement: String,
    problem_domain: String,
    attempt_id: String,
    step_number: u32,
    accepted: ContrastiveStepSummary,
    rejected: ContrastiveStepSummary,
    rejection_reason: Option<String>,
}

#[derive(Serialize)]
struct ContrastiveStepSummary {
    step_id: String,
    model: String,
    proposal_type: String,
    proposal_natural: String,
    proposal_formal: Option<String>,
    obligation_id: Option<String>,
    obligation_desc: Option<String>,
    obligation_type: Option<String>,
    stale_sibling: bool,
    semantic_redundant: bool,
}

/// A single JSONL training record for after-action analysis.
#[derive(Serialize)]
struct AfterActionRecord {
    problem_id: String,
    problem_statement: String,
    problem_domain: String,
    attempt_id: String,
    total_steps: u32,
    verified_steps: u32,
    rejected_steps: u32,
    accuracy_pct: f64,
    total_tokens_in: u64,
    total_tokens_out: u64,
    total_wall_ms: u64,
    models_used: Vec<String>,
    failure_modes: Vec<FailureModeEntry>,
    verified_chain_length: usize,
    proof_complete: bool,
    open_obligations: u32,
    final_answer: Option<String>,
}

#[derive(Serialize)]
struct FailureModeEntry {
    reason: String,
    count: u32,
}

/// Returns the default export directory (Desktop/chatdb-exports) as an absolute path.
#[tauri::command]
pub fn get_export_directory() -> Result<String, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let export_dir = std::path::Path::new(&home)
        .join("Desktop")
        .join("chatdb-exports");
    Ok(export_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn export_training_data(
    state: tauri::State<'_, Arc<AppState>>,
    problem_id: Option<String>,
    export_type: String,
    output_path: String,
) -> Result<u32, String> {
    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(&output_path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }
    match export_type.as_str() {
        "step_verification" => {
            export_step_verification(&state, problem_id.as_deref(), &output_path)
        }
        "planning" => export_planning(&state, problem_id.as_deref(), &output_path),
        "contrastive" => export_contrastive(&state, problem_id.as_deref(), &output_path),
        "contradiction" => export_contradiction(&state, problem_id.as_deref(), &output_path),
        "after_action" => export_after_action(&state, problem_id.as_deref(), &output_path),
        _ => Err(format!("Unknown export type: {}", export_type)),
    }
}

fn export_step_verification(
    state: &tauri::State<'_, Arc<AppState>>,
    problem_id: Option<&str>,
    output_path: &str,
) -> Result<u32, String> {
    use std::io::Write;
    let conn = state.db.conn();

    // Get problems to export
    let problems = get_target_problems(&conn, problem_id)?;
    let mut count = 0u32;
    let mut file =
        std::fs::File::create(output_path).map_err(|e| format!("Failed to create file: {}", e))?;

    for (pid, pstatement, pdomain) in &problems {
        // Get all attempts for this problem
        let mut att_stmt = conn
            .prepare("SELECT id FROM attempts WHERE problem_id = ?1 ORDER BY started_at")
            .map_err(|e| e.to_string())?;
        let attempt_ids: Vec<String> = att_stmt
            .query_map(rusqlite::params![pid], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        for attempt_id in &attempt_ids {
            // Get all steps for this attempt ordered by step_number
            let mut step_stmt = conn.prepare(
                "SELECT s.step_number, s.model, s.goal_state, s.proposal_type, s.proposal_natural, s.proposal_formal, s.proposal_reasoning, s.verified, s.rejection_reason, s.sympy_passed, s.pint_passed, s.lean_passed, s.obligation_id, COALESCE(s.obligation_desc, o.description), COALESCE(s.obligation_type, o.obligation_type), s.stale_sibling, s.semantic_redundant, s.challenge_flaw_found, s.challenge_attack, s.challenge_confidence, s.challenge_fatal
                 FROM steps s
                 LEFT JOIN obligations o ON s.obligation_id = o.id
                 WHERE s.attempt_id = ?1 ORDER BY s.step_number"
            ).map_err(|e| e.to_string())?;

            let steps: Vec<(
                u32,
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                bool,
                Option<String>,
                Option<bool>,
                Option<bool>,
                Option<bool>,
                Option<String>,
                Option<String>,
                Option<String>,
                bool,
                bool,
                Option<bool>,
                Option<String>,
                Option<f64>,
                Option<bool>,
            )> = step_stmt
                .query_map(rusqlite::params![attempt_id], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get::<_, i32>(7).map(|v| v != 0)?,
                        row.get(8)?,
                        row.get::<_, Option<i32>>(9)?.map(|v| v != 0),
                        row.get::<_, Option<i32>>(10)?.map(|v| v != 0),
                        row.get::<_, Option<i32>>(11)?.map(|v| v != 0),
                        row.get(12)?,
                        row.get(13)?,
                        row.get(14)?,
                        row.get::<_, Option<i32>>(15)?.unwrap_or(0) != 0,
                        row.get::<_, Option<i32>>(16)?.unwrap_or(0) != 0,
                        row.get::<_, Option<i32>>(17)?.map(|v| v != 0),
                        row.get(18)?,
                        row.get(19)?,
                        row.get::<_, Option<i32>>(20)?.map(|v| v != 0),
                    ))
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            // Build prior chain incrementally
            let mut prior_chain: Vec<ChainContext> = Vec::new();
            for step in &steps {
                let record = StepVerificationRecord {
                    problem_statement: pstatement.clone(),
                    problem_domain: pdomain.clone(),
                    step_number: step.0,
                    model: step.1.clone(),
                    goal_state: step.2.clone(),
                    proposal_type: step.3.clone(),
                    proposal_natural: step.4.clone(),
                    proposal_formal: step.5.clone(),
                    proposal_reasoning: step.6.clone(),
                    prior_chain: prior_chain.clone(),
                    verified: step.7,
                    rejection_reason: step.8.clone(),
                    sympy_passed: step.9,
                    pint_passed: step.10,
                    lean_passed: step.11,
                    obligation_id: step.12.clone(),
                    obligation_desc: step.13.clone(),
                    obligation_type: step.14.clone(),
                    stale_sibling: step.15,
                    semantic_redundant: step.16,
                    challenge_flaw_found: step.17,
                    challenge_attack: step.18.clone(),
                    challenge_confidence: step.19,
                    challenge_fatal: step.20,
                };
                let json = serde_json::to_string(&record).map_err(|e| e.to_string())?;
                writeln!(file, "{}", json).map_err(|e| e.to_string())?;
                count += 1;

                // Add verified steps to prior chain
                if step.7 {
                    prior_chain.push(ChainContext {
                        step_number: step.0,
                        proposal_natural: step.4.clone(),
                        proposal_formal: step.5.clone(),
                    });
                }
            }
        }
    }
    Ok(count)
}

fn export_planning(
    state: &tauri::State<'_, Arc<AppState>>,
    problem_id: Option<&str>,
    output_path: &str,
) -> Result<u32, String> {
    use std::io::Write;
    let conn = state.db.conn();
    let problems = get_target_problems(&conn, problem_id)?;
    let mut count = 0u32;
    let mut file =
        std::fs::File::create(output_path).map_err(|e| format!("Failed to create file: {}", e))?;

    for (pid, pstatement, pdomain) in &problems {
        let mut att_stmt = conn
            .prepare("SELECT id FROM attempts WHERE problem_id = ?1 ORDER BY started_at")
            .map_err(|e| e.to_string())?;
        let attempt_ids: Vec<String> = att_stmt
            .query_map(rusqlite::params![pid], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        for attempt_id in &attempt_ids {
            let mut ob_stmt = conn.prepare(
                "SELECT id, obligation_type, description, priority, status, steps_spent, max_steps, depends_on
                 FROM obligations WHERE attempt_id = ?1 ORDER BY created_at"
            ).map_err(|e| e.to_string())?;

            let obligations: Vec<(
                String,
                String,
                String,
                f64,
                String,
                i32,
                i32,
                Option<String>,
            )> = ob_stmt
                .query_map(rusqlite::params![attempt_id], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get::<_, Option<i32>>(5)?.unwrap_or(0),
                        row.get::<_, Option<i32>>(6)?.unwrap_or(5),
                        row.get(7)?,
                    ))
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            for ob in &obligations {
                let record = PlanningRecord {
                    problem_statement: pstatement.clone(),
                    problem_domain: pdomain.clone(),
                    attempt_id: attempt_id.clone(),
                    obligation_id: ob.0.clone(),
                    obligation_type: ob.1.clone(),
                    obligation_description: ob.2.clone(),
                    priority: ob.3,
                    status: ob.4.clone(),
                    steps_spent: ob.5,
                    max_steps: ob.6,
                    depends_on: ob.7.clone(),
                };
                let json = serde_json::to_string(&record).map_err(|e| e.to_string())?;
                writeln!(file, "{}", json).map_err(|e| e.to_string())?;
                count += 1;
            }
        }
    }
    Ok(count)
}

fn export_contradiction(
    state: &tauri::State<'_, Arc<AppState>>,
    problem_id: Option<&str>,
    output_path: &str,
) -> Result<u32, String> {
    use std::io::Write;
    let conn = state.db.conn();
    let problems = get_target_problems(&conn, problem_id)?;
    let mut count = 0u32;
    let mut file =
        std::fs::File::create(output_path).map_err(|e| format!("Failed to create file: {}", e))?;

    for (pid, pstatement, _pdomain) in &problems {
        let mut att_stmt = conn
            .prepare("SELECT id FROM attempts WHERE problem_id = ?1")
            .map_err(|e| e.to_string())?;
        let attempt_ids: Vec<String> = att_stmt
            .query_map(rusqlite::params![pid], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        for attempt_id in &attempt_ids {
            // Get conflicts
            let mut conf_stmt = conn.prepare(
                "SELECT c.claim_a_id, c.claim_b_id, c.conflict_type, c.severity, c.description, c.resolution
                 FROM conflicts c WHERE c.attempt_id = ?1"
            ).map_err(|e| e.to_string())?;

            let conflicts: Vec<(String, String, String, String, String, Option<String>)> =
                conf_stmt
                    .query_map(rusqlite::params![attempt_id], |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    })
                    .map_err(|e| e.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;

            for conf in &conflicts {
                let claim_a = get_claim_summary(&conn, &conf.0);
                let claim_b = get_claim_summary(&conn, &conf.1);
                if let (Some(a), Some(b)) = (claim_a, claim_b) {
                    let record = ContradictionRecord {
                        problem_statement: pstatement.clone(),
                        attempt_id: attempt_id.clone(),
                        claim_a: a,
                        claim_b: b,
                        conflict_type: conf.2.clone(),
                        severity: conf.3.clone(),
                        description: conf.4.clone(),
                        resolution: conf.5.clone(),
                    };
                    let json = serde_json::to_string(&record).map_err(|e| e.to_string())?;
                    writeln!(file, "{}", json).map_err(|e| e.to_string())?;
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

fn export_contrastive(
    state: &tauri::State<'_, Arc<AppState>>,
    problem_id: Option<&str>,
    output_path: &str,
) -> Result<u32, String> {
    use std::io::Write;

    let conn = state.db.conn();
    let records = collect_contrastive_records(&conn, problem_id)?;
    let mut file =
        std::fs::File::create(output_path).map_err(|e| format!("Failed to create file: {}", e))?;

    for record in &records {
        let json = serde_json::to_string(record).map_err(|e| e.to_string())?;
        writeln!(file, "{}", json).map_err(|e| e.to_string())?;
    }

    Ok(records.len() as u32)
}

fn export_after_action(
    state: &tauri::State<'_, Arc<AppState>>,
    problem_id: Option<&str>,
    output_path: &str,
) -> Result<u32, String> {
    use std::io::Write;
    let conn = state.db.conn();
    let problems = get_target_problems(&conn, problem_id)?;
    let mut count = 0u32;
    let mut file =
        std::fs::File::create(output_path).map_err(|e| format!("Failed to create file: {}", e))?;

    for (pid, pstatement, pdomain) in &problems {
        let mut att_stmt = conn
            .prepare("SELECT id FROM attempts WHERE problem_id = ?1 ORDER BY started_at")
            .map_err(|e| e.to_string())?;
        let attempt_ids: Vec<String> = att_stmt
            .query_map(rusqlite::params![pid], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        for attempt_id in &attempt_ids {
            // Aggregate stats
            let total_steps: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM steps WHERE attempt_id = ?1",
                    rusqlite::params![attempt_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let verified_steps: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM steps WHERE attempt_id = ?1 AND verified = 1",
                    rusqlite::params![attempt_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let rejected_steps = total_steps.saturating_sub(verified_steps);
            let accuracy_pct = if total_steps > 0 {
                (verified_steps as f64 / total_steps as f64) * 100.0
            } else {
                0.0
            };

            let total_tokens_in: u64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(model_tokens_in), 0) FROM steps WHERE attempt_id = ?1",
                    rusqlite::params![attempt_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let total_tokens_out: u64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(model_tokens_out), 0) FROM steps WHERE attempt_id = ?1",
                    rusqlite::params![attempt_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let total_wall_ms: u64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(wall_time_ms), 0) FROM steps WHERE attempt_id = ?1",
                    rusqlite::params![attempt_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            let mut model_stmt = conn
                .prepare("SELECT DISTINCT model FROM steps WHERE attempt_id = ?1")
                .map_err(|e| e.to_string())?;
            let models_used: Vec<String> = model_stmt
                .query_map(rusqlite::params![attempt_id], |row| row.get(0))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            let mut fail_stmt = conn.prepare(
                "SELECT COALESCE(rejection_reason, 'unknown'), COUNT(*) FROM steps WHERE attempt_id = ?1 AND verified = 0 GROUP BY rejection_reason ORDER BY COUNT(*) DESC"
            ).map_err(|e| e.to_string())?;
            let failure_modes: Vec<FailureModeEntry> = fail_stmt
                .query_map(rusqlite::params![attempt_id], |row| {
                    Ok(FailureModeEntry {
                        reason: row.get(0)?,
                        count: row.get(1)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            let verified_chain_len: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM steps WHERE attempt_id = ?1 AND verified = 1",
                    rusqlite::params![attempt_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let attempt_status: String = conn
                .query_row(
                    "SELECT COALESCE(status, 'unknown') FROM attempts WHERE id = ?1",
                    rusqlite::params![attempt_id],
                    |r| r.get(0),
                )
                .unwrap_or_else(|_| "unknown".to_string());
            let proof_complete = attempt_status == "completed";
            let open_obligations: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM obligations WHERE attempt_id = ?1 AND status IN ('open', 'assigned')",
                    rusqlite::params![attempt_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let final_answer: Option<String> = conn
                .query_row(
                    "SELECT proposal_natural FROM steps WHERE attempt_id = ?1 AND proposal_type = 'conclusion' AND verified = 1 ORDER BY step_number DESC LIMIT 1",
                    rusqlite::params![attempt_id],
                    |r| r.get(0),
                )
                .ok();

            let record = AfterActionRecord {
                problem_id: pid.clone(),
                problem_statement: pstatement.clone(),
                problem_domain: pdomain.clone(),
                attempt_id: attempt_id.clone(),
                total_steps,
                verified_steps,
                rejected_steps,
                accuracy_pct,
                total_tokens_in,
                total_tokens_out,
                total_wall_ms,
                models_used,
                failure_modes,
                verified_chain_length: verified_chain_len as usize,
                proof_complete,
                open_obligations,
                final_answer,
            };
            let json = serde_json::to_string(&record).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    Ok(count)
}

/// Get (id, statement, domain) for target problems.
fn get_target_problems(
    conn: &rusqlite::Connection,
    problem_id: Option<&str>,
) -> Result<Vec<(String, String, String)>, String> {
    if let Some(pid) = problem_id {
        let result = conn
            .query_row(
                "SELECT id, statement, domain FROM problems WHERE id = ?1",
                rusqlite::params![pid],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| format!("Problem not found: {}", e))?;
        Ok(vec![result])
    } else {
        let mut stmt = conn
            .prepare("SELECT id, statement, domain FROM problems ORDER BY created_at DESC")
            .map_err(|e| e.to_string())?;
        let rows: Vec<(String, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    }
}

/// Get a claim summary by ID for contradiction records.
fn get_claim_summary(conn: &rusqlite::Connection, claim_id: &str) -> Option<ClaimSummary> {
    conn.query_row(
        "SELECT claim_type, object, natural_text, confidence, verified FROM claims WHERE id = ?1",
        rusqlite::params![claim_id],
        |row| {
            Ok(ClaimSummary {
                claim_type: row.get(0)?,
                object: row.get(1)?,
                natural_text: row.get(2)?,
                confidence: row.get(3)?,
                verified: row.get::<_, i32>(4).map(|v| v != 0)?,
            })
        },
    )
    .ok()
}

fn collect_contrastive_records(
    conn: &rusqlite::Connection,
    problem_id: Option<&str>,
) -> Result<Vec<ContrastiveRecord>, String> {
    let problems = get_target_problems(conn, problem_id)?;
    let mut records = Vec::new();

    for (pid, pstatement, pdomain) in &problems {
        let mut stmt = conn
            .prepare(
                "SELECT cp.attempt_id, cp.step_number, cp.rejection_reason,
                    accepted.id, accepted.model, accepted.proposal_type, accepted.proposal_natural,
                    accepted.proposal_formal, accepted.obligation_id,
                    COALESCE(accepted.obligation_desc, ao.description),
                    COALESCE(accepted.obligation_type, ao.obligation_type),
                    accepted.stale_sibling,
                    accepted.semantic_redundant,
                    rejected.id, rejected.model, rejected.proposal_type, rejected.proposal_natural,
                    rejected.proposal_formal, rejected.obligation_id,
                    COALESCE(rejected.obligation_desc, ro.description),
                    COALESCE(rejected.obligation_type, ro.obligation_type),
                    rejected.stale_sibling,
                    rejected.semantic_redundant
             FROM contrastive_pairs cp
             JOIN attempts a ON cp.attempt_id = a.id
             JOIN steps accepted ON cp.accepted_step_id = accepted.id
             JOIN steps rejected ON cp.rejected_step_id = rejected.id
             LEFT JOIN obligations ao ON accepted.obligation_id = ao.id
             LEFT JOIN obligations ro ON rejected.obligation_id = ro.id
             WHERE a.problem_id = ?1
             ORDER BY cp.created_at ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(rusqlite::params![pid], |row| {
                Ok(ContrastiveRecord {
                    problem_id: pid.clone(),
                    problem_statement: pstatement.clone(),
                    problem_domain: pdomain.clone(),
                    attempt_id: row.get(0)?,
                    step_number: row.get(1)?,
                    rejection_reason: row.get(2)?,
                    accepted: ContrastiveStepSummary {
                        step_id: row.get(3)?,
                        model: row.get(4)?,
                        proposal_type: row.get(5)?,
                        proposal_natural: row.get(6)?,
                        proposal_formal: row.get(7)?,
                        obligation_id: row.get(8)?,
                        obligation_desc: row.get(9)?,
                        obligation_type: row.get(10)?,
                        stale_sibling: row.get::<_, Option<i32>>(11)?.unwrap_or(0) != 0,
                        semantic_redundant: row.get::<_, Option<i32>>(12)?.unwrap_or(0) != 0,
                    },
                    rejected: ContrastiveStepSummary {
                        step_id: row.get(13)?,
                        model: row.get(14)?,
                        proposal_type: row.get(15)?,
                        proposal_natural: row.get(16)?,
                        proposal_formal: row.get(17)?,
                        obligation_id: row.get(18)?,
                        obligation_desc: row.get(19)?,
                        obligation_type: row.get(20)?,
                        stale_sibling: row.get::<_, Option<i32>>(21)?.unwrap_or(0) != 0,
                        semantic_redundant: row.get::<_, Option<i32>>(22)?.unwrap_or(0) != 0,
                    },
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        records.extend(rows);
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::collect_contrastive_records;
    use crate::db::{Database, StepRecord};

    fn sample_step_record<'a>(
        attempt_id: &'a str,
        step_number: u32,
        natural: &'a str,
        verified: bool,
    ) -> StepRecord<'a> {
        StepRecord {
            attempt_id,
            parent_step_id: None,
            step_number,
            model: "test-model",
            context_refs: None,
            goal_state: "prove something",
            context_provided: None,
            proposal_type: "lemma",
            proposal_natural: natural,
            proposal_formal: Some("1 = 1"),
            proposal_reasoning: Some("by inspection"),
            sympy_result: Some("ok"),
            sympy_passed: Some(verified),
            pint_result: None,
            pint_passed: None,
            lean_result: None,
            lean_passed: None,
            verified,
            rejection_reason: (!verified).then_some("validator rejected"),
            model_tokens_in: Some(5),
            model_tokens_out: Some(3),
            wall_time_ms: Some(2),
            challenge_model: None,
            challenge_flaw_found: None,
            challenge_attack: None,
            challenge_confidence: None,
            challenge_fatal: None,
            obligation_id: None,
            solver_round_id: None,
            solver_worker_id: None,
            solver_dispatch_mode: None,
            stale_sibling: false,
        }
    }

    #[test]
    fn collect_contrastive_records_returns_paired_step_content() {
        let db = Database::new_in_memory().expect("in-memory db");
        let problem = db
            .create_problem("show the bound", "number_theory", "test")
            .expect("create problem");
        let attempt_id = db
            .create_attempt(&problem.id, &["test-model".to_string()])
            .expect("create attempt");

        let accepted_id = db
            .record_step(&sample_step_record(&attempt_id, 4, "accepted step", true))
            .expect("record accepted");
        let rejected_id = db
            .record_step(&sample_step_record(&attempt_id, 4, "rejected step", false))
            .expect("record rejected");
        db.record_contrastive_pair(
            &accepted_id,
            &rejected_id,
            4,
            &attempt_id,
            Some("validator rejected"),
        )
        .expect("record contrastive pair");

        let conn = db.conn();
        let records = collect_contrastive_records(&conn, Some(&problem.id))
            .expect("collect contrastive records");

        assert_eq!(records.len(), 1);
        let record_json = serde_json::to_value(&records[0]).expect("serialize record");
        assert_eq!(record_json["step_number"], 4);
        assert_eq!(record_json["accepted"]["proposal_natural"], "accepted step");
        assert_eq!(record_json["rejected"]["proposal_natural"], "rejected step");
        assert_eq!(record_json["rejection_reason"], "validator rejected");
        assert_eq!(record_json["problem_domain"], "number_theory");
    }
}
