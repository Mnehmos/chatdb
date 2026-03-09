pub mod branches;
pub mod claims;
pub mod conflicts;
pub mod contrastive;
pub mod dag_events;
pub mod decompositions;
pub mod discerner;
pub mod edges;
pub mod obligations;
pub mod profiles;
pub mod proof_nodes;
pub mod reports;
pub mod research;
pub mod resolution_corpus;
pub mod schema;
pub mod scout_sessions;
pub mod seed;
pub mod signals;
pub mod technique_registry;
pub mod tool_policy;
pub mod tool_runs;

use rusqlite::Connection;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Not found: {0}")]
    NotFound(String),
}

/// All data for a single proof step — fills every column in the steps table.
pub struct StepRecord<'a> {
    pub attempt_id: &'a str,
    pub parent_step_id: Option<&'a str>,
    pub step_number: u32,
    pub model: &'a str,
    pub context_refs: Option<&'a str>,
    pub goal_state: &'a str,
    pub context_provided: Option<&'a str>,
    pub proposal_type: &'a str,
    pub proposal_natural: &'a str,
    pub proposal_formal: Option<&'a str>,
    pub proposal_reasoning: Option<&'a str>,
    pub sympy_result: Option<&'a str>,
    pub sympy_passed: Option<bool>,
    pub pint_result: Option<&'a str>,
    pub pint_passed: Option<bool>,
    pub lean_result: Option<&'a str>,
    pub lean_passed: Option<bool>,
    pub verified: bool,
    pub rejection_reason: Option<&'a str>,
    pub model_tokens_in: Option<u32>,
    pub model_tokens_out: Option<u32>,
    pub wall_time_ms: Option<u32>,
    pub challenge_model: Option<&'a str>,
    pub challenge_flaw_found: Option<bool>,
    pub challenge_attack: Option<&'a str>,
    pub challenge_confidence: Option<f64>,
    pub challenge_fatal: Option<bool>,
    /// RED-012: Which obligation this step targets (written to steps.obligation_id column).
    pub obligation_id: Option<&'a str>,
    // V14: Solver round ID for tracking parallel dispatches
    pub solver_round_id: Option<&'a str>,
    // V15: Fan-in metadata
    pub solver_worker_id: Option<&'a str>,
    pub solver_dispatch_mode: Option<&'a str>,
    pub stale_sibling: bool,
}

pub struct Database {
    conn: Mutex<Connection>,
}

fn normalize_step_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn detect_semantic_redundancy(
    conn: &Connection,
    record: &StepRecord<'_>,
) -> Result<bool, rusqlite::Error> {
    let Some(obligation_id) = record.obligation_id else {
        return Ok(false);
    };

    if !record.verified || record.stale_sibling || record.proposal_type == "stale_sibling" {
        return Ok(false);
    }

    let current_natural = normalize_step_text(record.proposal_natural);
    let current_formal = record.proposal_formal.map(normalize_step_text);

    let mut stmt = conn.prepare(
        "SELECT proposal_natural, proposal_formal
         FROM steps
         WHERE attempt_id = ?1
           AND obligation_id = ?2
           AND proposal_type = ?3
           AND verified = 1
         ORDER BY step_number ASC",
    )?;

    let existing_rows = stmt.query_map(
        rusqlite::params![record.attempt_id, obligation_id, record.proposal_type],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
    )?;

    for existing in existing_rows {
        let (existing_natural, existing_formal) = existing?;
        if current_natural == normalize_step_text(&existing_natural) {
            return Ok(true);
        }

        if let (Some(current_formal), Some(existing_formal)) =
            (current_formal.as_ref(), existing_formal.as_deref())
        {
            if current_formal == &normalize_step_text(existing_formal) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

impl Database {
    pub fn new(path: &str) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn new_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute_batch(schema::SCHEMA_V2)?;
        // Migrate: add proposal_reasoning column if missing (existing DBs)
        let _ = conn.execute_batch("ALTER TABLE steps ADD COLUMN proposal_reasoning TEXT;");
        // V3 migrations: exploration audit columns, branches, known_answer
        for sql in schema::MIGRATIONS_V3 {
            let _ = conn.execute_batch(sql); // ignore "duplicate column" errors
        }
        // V4 migrations: DAG architecture tables (CREATE IF NOT EXISTS — safe to re-run)
        conn.execute_batch(schema::MIGRATIONS_V4)?;
        // V5 migrations: Challenge/adversarial columns on steps table
        for sql in schema::MIGRATIONS_V5 {
            let _ = conn.execute_batch(sql); // ignore "duplicate column" errors
        }
        // V6 migrations: Agent profiles table (CREATE IF NOT EXISTS — safe to re-run)
        conn.execute_batch(schema::MIGRATIONS_V6)?;
        // V7 migrations: Discerner decisions table (CREATE IF NOT EXISTS — safe to re-run)
        conn.execute_batch(schema::MIGRATIONS_V7)?;
        // V8 migrations: Mid-run discerner findings table (CREATE IF NOT EXISTS — safe to re-run)
        conn.execute_batch(schema::MIGRATIONS_V8)?;
        // V9 migrations: Decompositions table + obligation dependency columns
        conn.execute_batch(schema::MIGRATIONS_V9)?;
        for sql in schema::MIGRATIONS_V9_COLS {
            let _ = conn.execute_batch(sql); // ignore "duplicate column" errors
        }
        // V10 migrations: Management System — claims, dag_edges, conflicts, after_action_reports
        conn.execute_batch(schema::MIGRATIONS_V10)?;
        for sql in schema::MIGRATIONS_V10_COLS {
            let _ = conn.execute_batch(sql); // ignore "duplicate column" errors
        }
        // V11 migrations: Research API key storage (CREATE IF NOT EXISTS — safe to re-run)
        conn.execute_batch(schema::MIGRATIONS_V11)?;
        // V12 migrations: Obligation satisfaction signals (tally-based closure)
        conn.execute_batch(schema::MIGRATIONS_V12)?;
        // V13 migrations: Contrastive pairs (rejected vs accepted steps for training data)
        conn.execute_batch(schema::MIGRATIONS_V13)?;
        // V14 migrations: Tool-use redesign tables + scout/policy columns
        conn.execute_batch(schema::MIGRATIONS_V14)?;
        for sql in schema::MIGRATIONS_V14_COLS {
            let _ = conn.execute_batch(sql); // ignore "duplicate column" errors
        }
        // V15 migrations: Fan-in solver collaboration columns
        for sql in schema::MIGRATIONS_V15_COLS {
            let _ = conn.execute_batch(sql); // ignore "duplicate column" errors
        }
        // V16 migrations: semantic redundancy labeling on steps
        for sql in schema::MIGRATIONS_V16_COLS {
            let _ = conn.execute_batch(sql); // ignore "duplicate column" errors
        }
        drop(conn);
        // Seed technique registry with competition math techniques
        let _ = self.seed_techniques(seed::TECHNIQUE_SEEDS);
        Ok(())
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("DB mutex was poisoned, recovering");
            poisoned.into_inner()
        })
    }

    // ---- Problems ----

    pub fn create_problem(
        &self,
        statement: &str,
        domain: &str,
        source: &str,
    ) -> Result<crate::models::proof::Problem, DbError> {
        let conn = self.conn();
        // Dedup: if a problem with the same statement exists, return it
        if let Ok(existing) = conn.query_row(
            "SELECT id, statement, formal_statement, domain, source, status, created_at, solved_at, total_attempts, total_steps, known_answer, title, difficulty, metadata FROM problems WHERE statement = ?1 LIMIT 1",
            rusqlite::params![statement],
            |row| {
                Ok(crate::models::proof::Problem {
                    id: row.get(0)?,
                    statement: row.get(1)?,
                    formal_statement: row.get(2)?,
                    domain: row.get(3)?,
                    source: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                    solved_at: row.get(7)?,
                    total_attempts: row.get(8)?,
                    total_steps: row.get(9)?,
                    known_answer: row.get(10)?,
                    title: row.get(11)?,
                    difficulty: row.get(12)?,
                    metadata: row.get(13)?,
                })
            },
        ) {
            return Ok(existing);
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO problems (id, statement, domain, source, status, created_at) VALUES (?1, ?2, ?3, ?4, 'open', ?5)",
            rusqlite::params![id, statement, domain, source, now],
        )?;
        Ok(crate::models::proof::Problem {
            id,
            statement: statement.to_string(),
            formal_statement: None,
            domain: domain.to_string(),
            source: source.to_string(),
            status: "open".to_string(),
            created_at: now,
            solved_at: None,
            total_attempts: 0,
            total_steps: 0,
            known_answer: None,
            title: None,
            difficulty: None,
            metadata: None,
        })
    }

    pub fn get_problem(&self, id: &str) -> Result<crate::models::proof::Problem, DbError> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, statement, formal_statement, domain, source, status, created_at, solved_at, total_attempts, total_steps, known_answer, title, difficulty, metadata FROM problems WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok(crate::models::proof::Problem {
                    id: row.get(0)?,
                    statement: row.get(1)?,
                    formal_statement: row.get(2)?,
                    domain: row.get(3)?,
                    source: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                    solved_at: row.get(7)?,
                    total_attempts: row.get(8)?,
                    total_steps: row.get(9)?,
                    known_answer: row.get(10)?,
                    title: row.get(11)?,
                    difficulty: row.get(12)?,
                    metadata: row.get(13)?,
                })
            },
        )
        .map_err(|_| DbError::NotFound(format!("Problem {}", id)))
    }

    pub fn list_problems(
        &self,
        status_filter: Option<&str>,
    ) -> Result<Vec<crate::models::proof::Problem>, DbError> {
        let conn = self.conn();
        let sql = match status_filter {
            Some(s) => format!(
                "SELECT id, statement, formal_statement, domain, source, status, created_at, solved_at, total_attempts, total_steps, known_answer, title, difficulty, metadata FROM problems WHERE status = '{}' ORDER BY created_at DESC",
                s
            ),
            None => "SELECT id, statement, formal_statement, domain, source, status, created_at, solved_at, total_attempts, total_steps, known_answer, title, difficulty, metadata FROM problems ORDER BY created_at DESC".to_string(),
        };
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(crate::models::proof::Problem {
                    id: row.get(0)?,
                    statement: row.get(1)?,
                    formal_statement: row.get(2)?,
                    domain: row.get(3)?,
                    source: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                    solved_at: row.get(7)?,
                    total_attempts: row.get(8)?,
                    total_steps: row.get(9)?,
                    known_answer: row.get(10)?,
                    title: row.get(11)?,
                    difficulty: row.get(12)?,
                    metadata: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // ---- Attempts ----

    pub fn create_attempt(&self, problem_id: &str, models: &[String]) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let models_json = serde_json::to_string(models).unwrap_or_else(|_| "[]".to_string());
        let conn = self.conn();
        conn.execute_batch("BEGIN")?;
        let result = (|| -> Result<(), DbError> {
            conn.execute(
                "INSERT INTO attempts (id, problem_id, status, started_at, models_used) VALUES (?1, ?2, 'active', ?3, ?4)",
                rusqlite::params![id, problem_id, now, models_json],
            )?;
            conn.execute(
                "UPDATE problems SET total_attempts = total_attempts + 1 WHERE id = ?1",
                rusqlite::params![problem_id],
            )?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT")?;
                Ok(id)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    pub fn get_latest_attempt_id(&self, problem_id: &str) -> Result<String, DbError> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id FROM attempts WHERE problem_id = ?1 ORDER BY started_at DESC LIMIT 1",
            rusqlite::params![problem_id],
            |row| row.get(0),
        )
        .map_err(|_| DbError::NotFound(format!("No attempts for problem {}", problem_id)))
    }

    /// Increment a numeric counter column on the attempts row.
    /// Used for steps_verified and steps_rejected — keeps running totals without a full recount.
    /// Allowed columns are whitelisted to prevent SQL injection.
    pub fn increment_attempt_counter(&self, attempt_id: &str, column: &str) -> Result<(), DbError> {
        // Whitelist to prevent accidental or malicious column injection
        if !matches!(column, "steps_verified" | "steps_rejected") {
            return Err(DbError::NotFound(format!(
                "Unknown counter column: {}",
                column
            )));
        }
        let conn = self.conn();
        let sql = format!(
            "UPDATE attempts SET {} = COALESCE({}, 0) + 1 WHERE id = ?1",
            column, column
        );
        conn.execute(&sql, rusqlite::params![attempt_id])?;
        Ok(())
    }

    /// Startup cleanup: mark stale "active" attempts as "stopped" and reset
    /// "assigned" obligations back to "open". These are leftovers from crashes.
    /// Returns (stale_attempts, stale_obligations) counts.
    pub fn cleanup_stale_on_startup(&self) -> Result<(usize, usize), DbError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute_batch("BEGIN")?;
        let stale_attempts = conn.execute(
            "UPDATE attempts SET status = 'stopped', ended_at = ?1 WHERE status = 'active'",
            rusqlite::params![now],
        )?;
        let stale_obligations = conn.execute(
            "UPDATE obligations SET status = 'open', assigned_model = NULL WHERE status = 'assigned'",
            rusqlite::params![],
        )?;
        conn.execute_batch("COMMIT")?;
        Ok((stale_attempts, stale_obligations))
    }

    pub fn update_attempt_status(&self, attempt_id: &str, status: &str) -> Result<(), DbError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "UPDATE attempts SET status = ?1, ended_at = ?2 WHERE id = ?3",
            rusqlite::params![status, now, attempt_id],
        )?;
        Ok(())
    }

    /// Delete an attempt and all associated data (steps, obligations, proof_nodes, etc.)
    pub fn delete_attempt(&self, attempt_id: &str) -> Result<(), DbError> {
        let conn = self.conn();
        // Get the problem_id before deleting so we can decrement counts
        let problem_id: String = conn
            .query_row(
                "SELECT problem_id FROM attempts WHERE id = ?1",
                rusqlite::params![attempt_id],
                |row| row.get(0),
            )
            .map_err(|_| DbError::NotFound(format!("Attempt {}", attempt_id)))?;

        // Count steps for decrementing problem.total_steps
        let step_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM steps WHERE attempt_id = ?1",
                rusqlite::params![attempt_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // All deletes in a single transaction to prevent orphaned records on crash
        conn.execute_batch("BEGIN")?;

        // Delete in dependency order (children first, then attempt last)
        // Must cover ALL tables with attempt_id to avoid FK constraint violations
        let tables = [
            // V14 tool-use tables (delete before obligations/steps they reference)
            "DELETE FROM corpus_usages WHERE attempt_id = ?1",
            "DELETE FROM obligation_resolutions WHERE attempt_id = ?1",
            "DELETE FROM tool_artifacts WHERE attempt_id = ?1",
            "DELETE FROM tool_runs WHERE attempt_id = ?1",
            "DELETE FROM scout_sessions WHERE attempt_id = ?1",
            "DELETE FROM tool_policy_violations WHERE attempt_id = ?1",
            // Original tables
            "DELETE FROM claims WHERE attempt_id = ?1",
            "DELETE FROM conflicts WHERE attempt_id = ?1",
            "DELETE FROM dag_edges WHERE source_id IN (SELECT id FROM proof_nodes WHERE attempt_id = ?1) OR target_id IN (SELECT id FROM proof_nodes WHERE attempt_id = ?1)",
            "DELETE FROM dag_events WHERE attempt_id = ?1",
            "DELETE FROM proof_nodes WHERE attempt_id = ?1",
            "DELETE FROM branches WHERE attempt_id = ?1",
            "DELETE FROM steps WHERE attempt_id = ?1",
            "DELETE FROM obligations WHERE attempt_id = ?1",
            "DELETE FROM council_findings WHERE attempt_id = ?1",
            "DELETE FROM critic_evaluations WHERE attempt_id = ?1",
            "DELETE FROM orchestrator_decisions WHERE attempt_id = ?1",
            "DELETE FROM discerner_decisions WHERE attempt_id = ?1",
            "DELETE FROM discerner_findings WHERE attempt_id = ?1",
            "DELETE FROM decompositions WHERE attempt_id = ?1",
            "DELETE FROM after_action_reports WHERE attempt_id = ?1",
            "DELETE FROM attempts WHERE id = ?1",
        ];
        for sql in tables {
            let _ = conn.execute(sql, rusqlite::params![attempt_id]);
        }

        // Decrement problem counters
        let _ = conn.execute(
            "UPDATE problems SET total_attempts = MAX(0, total_attempts - 1), total_steps = MAX(0, total_steps - ?1) WHERE id = ?2",
            rusqlite::params![step_count, problem_id],
        );

        conn.execute_batch("COMMIT")?;

        tracing::info!(
            "Deleted attempt {} ({} steps) from problem {}",
            attempt_id,
            step_count,
            problem_id
        );
        Ok(())
    }

    pub fn get_max_step_number(&self, attempt_id: &str) -> Result<u32, DbError> {
        let conn = self.conn();
        let n: u32 = conn.query_row(
            "SELECT COALESCE(MAX(step_number), 0) FROM steps WHERE attempt_id = ?1",
            rusqlite::params![attempt_id],
            |row| row.get(0),
        )?;
        Ok(n)
    }

    // ---- Steps ----

    pub fn record_step(&self, r: &StepRecord) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        let (obligation_desc, obligation_type): (Option<String>, Option<String>) =
            if let Some(obligation_id) = r.obligation_id {
                conn.query_row(
                    "SELECT description, obligation_type FROM obligations WHERE id = ?1",
                    rusqlite::params![obligation_id],
                    |row| Ok((Some(row.get(0)?), Some(row.get(1)?))),
                )
                .unwrap_or((None, None))
            } else {
                (None, None)
            };
        let semantic_redundant = detect_semantic_redundancy(&conn, r)?;
        conn.execute(
            "INSERT INTO steps (id, attempt_id, parent_step_id, step_number, model, context_refs, goal_state, context_provided, proposal_type, proposal_natural, proposal_formal, proposal_reasoning, sympy_result, sympy_passed, pint_result, pint_passed, lean_result, lean_passed, verified, rejection_reason, model_tokens_in, model_tokens_out, wall_time_ms, challenge_model, challenge_flaw_found, challenge_attack, challenge_confidence, challenge_fatal, obligation_id, obligation_desc, obligation_type, solver_round_id, solver_worker_id, solver_dispatch_mode, stale_sibling, semantic_redundant, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37)",
            rusqlite::params![
                id, r.attempt_id, r.parent_step_id, r.step_number, r.model,
                r.context_refs, r.goal_state, r.context_provided,
                r.proposal_type, r.proposal_natural, r.proposal_formal,
                r.proposal_reasoning,
                r.sympy_result, r.sympy_passed.map(|b| b as i32),
                r.pint_result, r.pint_passed.map(|b| b as i32),
                r.lean_result, r.lean_passed.map(|b| b as i32),
                r.verified as i32, r.rejection_reason,
                r.model_tokens_in, r.model_tokens_out, r.wall_time_ms,
                r.challenge_model, r.challenge_flaw_found.map(|b| b as i32),
                r.challenge_attack, r.challenge_confidence,
                r.challenge_fatal.map(|b| b as i32),
                r.obligation_id,
                obligation_desc,
                obligation_type,
                r.solver_round_id, r.solver_worker_id, r.solver_dispatch_mode,
                r.stale_sibling as i32,
                semantic_redundant as i32,
                now
            ],
        )?;
        conn.execute(
            "UPDATE attempts
             SET step_count = step_count + 1,
                 cost_tokens_in = COALESCE(cost_tokens_in, 0) + COALESCE(?1, 0),
                 cost_tokens_out = COALESCE(cost_tokens_out, 0) + COALESCE(?2, 0)
             WHERE id = ?3",
            rusqlite::params![r.model_tokens_in, r.model_tokens_out, r.attempt_id],
        )?;
        // Update problem total_steps
        conn.execute(
            "UPDATE problems SET total_steps = total_steps + 1 WHERE id = (SELECT problem_id FROM attempts WHERE id = ?1)",
            rusqlite::params![r.attempt_id],
        )?;
        if let Some(obligation_id) = r.obligation_id {
            conn.execute(
                "UPDATE obligations SET steps_spent = COALESCE(steps_spent, 0) + 1 WHERE id = ?1",
                rusqlite::params![obligation_id],
            )?;
        }
        tracing::debug!(step_id = %id, step = r.step_number, verified = r.verified, "Step recorded to DB");
        Ok(id)
    }

    /// Update challenge/adversarial fields on an existing step (challenge runs after validation).
    pub fn update_step_challenge(
        &self,
        step_id: &str,
        challenge_model: &str,
        flaw_found: bool,
        attack: &str,
        confidence: f64,
        fatal: bool,
        rejection_reason: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| -> Result<(), DbError> {
            let (attempt_id, was_verified): (String, bool) = conn.query_row(
                "SELECT attempt_id, verified FROM steps WHERE id = ?1",
                rusqlite::params![step_id],
                |row| Ok((row.get(0)?, row.get::<_, i32>(1)? != 0)),
            )?;

            conn.execute(
                "UPDATE steps
                 SET challenge_model = ?1,
                     challenge_flaw_found = ?2,
                     challenge_attack = ?3,
                     challenge_confidence = ?4,
                     challenge_fatal = ?5
                 WHERE id = ?6",
                rusqlite::params![
                    challenge_model,
                    flaw_found as i32,
                    attack,
                    confidence,
                    fatal as i32,
                    step_id,
                ],
            )?;

            if flaw_found && fatal {
                conn.execute(
                    "UPDATE steps
                     SET verified = CASE WHEN verified != 0 THEN 0 ELSE verified END,
                         rejection_reason = COALESCE(?1, rejection_reason, challenge_attack)
                     WHERE id = ?2",
                    rusqlite::params![rejection_reason, step_id],
                )?;
            }

            if flaw_found && fatal && was_verified {
                conn.execute(
                    "UPDATE attempts
                     SET steps_verified = MAX(0, COALESCE(steps_verified, 0) - 1),
                         steps_rejected = COALESCE(steps_rejected, 0) + 1
                     WHERE id = ?1",
                    rusqlite::params![attempt_id],
                )?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => conn.execute_batch("COMMIT")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
        Ok(())
    }

    pub fn get_verified_chain(
        &self,
        attempt_id: &str,
    ) -> Result<Vec<(String, u32, String, String, String)>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, step_number, model, proposal_natural, COALESCE(proposal_formal, '') FROM steps WHERE attempt_id = ?1 AND verified = 1 ORDER BY step_number",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![attempt_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_problem_steps(
        &self,
        problem_id: &str,
    ) -> Result<Vec<crate::models::proof::Step>, DbError> {
        let conn = self.conn();
        // Return ALL steps across ALL attempts for this problem, ordered by
        // attempt start time then step number — preserves full history.
        let mut stmt = conn.prepare(
            "SELECT s.id, s.attempt_id, s.parent_step_id, s.step_number, s.model, s.goal_state, s.proposal_type, s.proposal_natural, s.proposal_formal, s.proposal_reasoning, s.verified, s.rejection_reason, s.sympy_passed, s.pint_passed, s.lean_passed, s.challenge_model, s.challenge_flaw_found, s.challenge_attack, s.challenge_confidence, s.challenge_fatal, s.obligation_id, COALESCE(s.obligation_desc, o.description), COALESCE(s.obligation_type, o.obligation_type), s.solver_round_id, s.solver_worker_id, s.solver_dispatch_mode, s.stale_sibling, s.created_at
             FROM steps s
             JOIN attempts a ON s.attempt_id = a.id
             LEFT JOIN obligations o ON s.obligation_id = o.id
             WHERE a.problem_id = ?1
             ORDER BY a.started_at ASC, s.step_number ASC",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![problem_id], |row| {
                Ok(crate::models::proof::Step {
                    id: row.get(0)?,
                    attempt_id: row.get(1)?,
                    parent_step_id: row.get(2)?,
                    step_number: row.get(3)?,
                    model: row.get(4)?,
                    goal_state: row.get(5)?,
                    proposal_type: row.get(6)?,
                    proposal_natural: row.get(7)?,
                    proposal_formal: row.get(8)?,
                    proposal_reasoning: row.get(9)?,
                    verified: row.get::<_, i32>(10)? != 0,
                    rejection_reason: row.get(11)?,
                    sympy_passed: row.get::<_, Option<i32>>(12)?.map(|v| v != 0),
                    pint_passed: row.get::<_, Option<i32>>(13)?.map(|v| v != 0),
                    lean_passed: row.get::<_, Option<i32>>(14)?.map(|v| v != 0),
                    challenge_model: row.get(15)?,
                    challenge_flaw_found: row.get::<_, Option<i32>>(16)?.map(|v| v != 0),
                    challenge_attack: row.get(17)?,
                    challenge_confidence: row.get(18)?,
                    challenge_fatal: row.get::<_, Option<i32>>(19)?.map(|v| v != 0),
                    obligation_id: row.get(20)?,
                    obligation_desc: row.get(21)?,
                    obligation_type: row.get(22)?,
                    solver_round_id: row.get(23)?,
                    solver_worker_id: row.get(24)?,
                    solver_dispatch_mode: row.get(25)?,
                    stale_sibling: row.get::<_, Option<i32>>(26)?.unwrap_or(0) != 0,
                    created_at: row.get(27)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_attempt_steps(
        &self,
        attempt_id: &str,
    ) -> Result<Vec<crate::models::proof::Step>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.attempt_id, s.parent_step_id, s.step_number, s.model, s.goal_state, s.proposal_type, s.proposal_natural, s.proposal_formal, s.proposal_reasoning, s.verified, s.rejection_reason, s.sympy_passed, s.pint_passed, s.lean_passed, s.challenge_model, s.challenge_flaw_found, s.challenge_attack, s.challenge_confidence, s.challenge_fatal, s.obligation_id, COALESCE(s.obligation_desc, o.description), COALESCE(s.obligation_type, o.obligation_type), s.solver_round_id, s.solver_worker_id, s.solver_dispatch_mode, s.stale_sibling, s.created_at
             FROM steps s
             LEFT JOIN obligations o ON s.obligation_id = o.id
             WHERE s.attempt_id = ?1 ORDER BY s.step_number ASC",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![attempt_id], |row| {
                Ok(crate::models::proof::Step {
                    id: row.get(0)?,
                    attempt_id: row.get(1)?,
                    parent_step_id: row.get(2)?,
                    step_number: row.get(3)?,
                    model: row.get(4)?,
                    goal_state: row.get(5)?,
                    proposal_type: row.get(6)?,
                    proposal_natural: row.get(7)?,
                    proposal_formal: row.get(8)?,
                    proposal_reasoning: row.get(9)?,
                    verified: row.get::<_, i32>(10)? != 0,
                    rejection_reason: row.get(11)?,
                    sympy_passed: row.get::<_, Option<i32>>(12)?.map(|v| v != 0),
                    pint_passed: row.get::<_, Option<i32>>(13)?.map(|v| v != 0),
                    lean_passed: row.get::<_, Option<i32>>(14)?.map(|v| v != 0),
                    challenge_model: row.get(15)?,
                    challenge_flaw_found: row.get::<_, Option<i32>>(16)?.map(|v| v != 0),
                    challenge_attack: row.get(17)?,
                    challenge_confidence: row.get(18)?,
                    challenge_fatal: row.get::<_, Option<i32>>(19)?.map(|v| v != 0),
                    obligation_id: row.get(20)?,
                    obligation_desc: row.get(21)?,
                    obligation_type: row.get(22)?,
                    solver_round_id: row.get(23)?,
                    solver_worker_id: row.get(24)?,
                    solver_dispatch_mode: row.get(25)?,
                    stale_sibling: row.get::<_, Option<i32>>(26)?.unwrap_or(0) != 0,
                    created_at: row.get(27)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_all_steps(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::models::proof::TrainingRow>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.attempt_id, p.id, s.step_number, s.model, s.proposal_type, s.proposal_natural, s.proposal_formal, s.verified, s.rejection_reason, s.sympy_passed, s.pint_passed, s.lean_passed, s.obligation_id, COALESCE(s.obligation_desc, o.description), COALESCE(s.obligation_type, o.obligation_type), s.stale_sibling, s.semantic_redundant, s.created_at, p.statement, p.domain
             FROM steps s
             JOIN attempts a ON s.attempt_id = a.id
             JOIN problems p ON a.problem_id = p.id
             LEFT JOIN obligations o ON s.obligation_id = o.id
             ORDER BY s.created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![limit], |row| {
                Ok(crate::models::proof::TrainingRow {
                    id: row.get(0)?,
                    attempt_id: row.get(1)?,
                    problem_id: row.get(2)?,
                    step_number: row.get(3)?,
                    model: row.get(4)?,
                    proposal_type: row.get(5)?,
                    proposal_natural: row.get(6)?,
                    proposal_formal: row.get(7)?,
                    verified: row.get::<_, i32>(8)? != 0,
                    rejection_reason: row.get(9)?,
                    sympy_passed: row.get::<_, Option<i32>>(10)?.map(|v| v != 0),
                    pint_passed: row.get::<_, Option<i32>>(11)?.map(|v| v != 0),
                    lean_passed: row.get::<_, Option<i32>>(12)?.map(|v| v != 0),
                    obligation_id: row.get(13)?,
                    obligation_desc: row.get(14)?,
                    obligation_type: row.get(15)?,
                    stale_sibling: row.get::<_, Option<i32>>(16)?.unwrap_or(0) != 0,
                    semantic_redundant: row.get::<_, Option<i32>>(17)?.unwrap_or(0) != 0,
                    created_at: row.get(18)?,
                    problem_statement: row.get(19)?,
                    problem_domain: row.get(20)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_training_data_stats(&self) -> Result<(u64, u64, u64, u64), DbError> {
        let conn = self.conn();
        let total: u64 = conn.query_row("SELECT COUNT(*) FROM steps", [], |r| r.get(0))?;
        let verified: u64 =
            conn.query_row("SELECT COUNT(*) FROM steps WHERE verified = 1", [], |r| {
                r.get(0)
            })?;
        let rejected: u64 =
            conn.query_row("SELECT COUNT(*) FROM steps WHERE verified = 0", [], |r| {
                r.get(0)
            })?;
        let contrastive_pairs: u64 =
            conn.query_row("SELECT COUNT(*) FROM contrastive_pairs", [], |r| r.get(0))?;
        Ok((total, verified, rejected, contrastive_pairs))
    }

    pub fn get_after_action_report(
        &self,
        problem_id: &str,
    ) -> Result<crate::models::proof::AfterActionReport, DbError> {
        let conn = self.conn();

        // Get the problem (inline — cannot call self.get_problem() while conn lock is held)
        let (problem_statement, problem_domain): (String, String) = conn
            .query_row(
                "SELECT statement, domain FROM problems WHERE id = ?1",
                rusqlite::params![problem_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| DbError::NotFound(format!("Problem {} not found", problem_id)))?;

        // Get latest attempt
        let (attempt_id, started_at, attempt_status): (String, String, String) = conn.query_row(
            "SELECT id, started_at, COALESCE(status, 'unknown') FROM attempts WHERE problem_id = ?1 ORDER BY started_at DESC LIMIT 1",
            rusqlite::params![problem_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).map_err(|_| DbError::NotFound(format!("No attempts for problem {}", problem_id)))?;
        let proof_complete = attempt_status == "completed";

        // Aggregate stats
        let total_steps: u32 = conn.query_row(
            "SELECT COUNT(*) FROM steps WHERE attempt_id = ?1",
            rusqlite::params![attempt_id],
            |r| r.get(0),
        )?;
        let verified_steps: u32 = conn.query_row(
            "SELECT COUNT(*) FROM steps WHERE attempt_id = ?1 AND verified = 1",
            rusqlite::params![attempt_id],
            |r| r.get(0),
        )?;
        let rejected_steps = total_steps - verified_steps;
        let accuracy_pct = if total_steps > 0 {
            (verified_steps as f64 / total_steps as f64) * 100.0
        } else {
            0.0
        };

        // Token totals
        let total_tokens_in: u64 = conn.query_row(
            "SELECT COALESCE(SUM(model_tokens_in), 0) FROM steps WHERE attempt_id = ?1",
            rusqlite::params![attempt_id],
            |r| r.get(0),
        )?;
        let total_tokens_out: u64 = conn.query_row(
            "SELECT COALESCE(SUM(model_tokens_out), 0) FROM steps WHERE attempt_id = ?1",
            rusqlite::params![attempt_id],
            |r| r.get(0),
        )?;
        let total_wall_ms: u64 = conn.query_row(
            "SELECT COALESCE(SUM(wall_time_ms), 0) FROM steps WHERE attempt_id = ?1",
            rusqlite::params![attempt_id],
            |r| r.get(0),
        )?;

        // Distinct models
        let mut model_stmt =
            conn.prepare("SELECT DISTINCT model FROM steps WHERE attempt_id = ?1")?;
        let models_used: Vec<String> = model_stmt
            .query_map(rusqlite::params![attempt_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        // Verified chain
        let mut chain_stmt = conn.prepare(
            "SELECT step_number, proposal_natural, proposal_formal, model FROM steps WHERE attempt_id = ?1 AND verified = 1 ORDER BY step_number",
        )?;
        let verified_chain: Vec<crate::models::proof::ChainStep> = chain_stmt
            .query_map(rusqlite::params![attempt_id], |row| {
                Ok(crate::models::proof::ChainStep {
                    step_number: row.get(0)?,
                    natural: row.get(1)?,
                    formal: row.get(2)?,
                    model: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Failure modes — group by rejection_reason
        let mut fail_stmt = conn.prepare(
            "SELECT COALESCE(rejection_reason, 'unknown'), COUNT(*) FROM steps WHERE attempt_id = ?1 AND verified = 0 GROUP BY rejection_reason ORDER BY COUNT(*) DESC",
        )?;
        let failure_modes: Vec<crate::models::proof::FailureMode> = fail_stmt
            .query_map(rusqlite::params![attempt_id], |row| {
                Ok(crate::models::proof::FailureMode {
                    reason: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Open obligations count
        let open_obligations: u32 = conn.query_row(
            "SELECT COUNT(*) FROM obligations WHERE attempt_id = ?1 AND status IN ('open', 'assigned')",
            rusqlite::params![attempt_id],
            |r| r.get(0),
        ).unwrap_or(0);

        // Final answer — from the conclusion step (if any)
        let final_answer: Option<String> = conn.query_row(
            "SELECT proposal_natural FROM steps WHERE attempt_id = ?1 AND proposal_type = 'conclusion' AND verified = 1 ORDER BY step_number DESC LIMIT 1",
            rusqlite::params![attempt_id],
            |r| r.get(0),
        ).ok();

        Ok(crate::models::proof::AfterActionReport {
            problem_id: problem_id.to_string(),
            problem_statement,
            problem_domain,
            attempt_id,
            total_steps,
            verified_steps,
            rejected_steps,
            accuracy_pct,
            total_tokens_in,
            total_tokens_out,
            total_wall_ms,
            models_used,
            verified_chain,
            failure_modes,
            started_at,
            proof_complete,
            open_obligations,
            final_answer,
        })
    }

    // ---- Patterns ----

    pub fn search_patterns(
        &self,
        query: &str,
    ) -> Result<Vec<crate::models::patterns::Pattern>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, domain, trigger_text, strategy, source_steps, success_count, failure_count, technique_class, deprecated, created_at, last_used_at FROM patterns WHERE deprecated = 0 AND (name LIKE '%' || ?1 || '%' OR trigger_text LIKE '%' || ?1 || '%') ORDER BY success_count DESC LIMIT 20",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![query], |row| {
                Ok(crate::models::patterns::Pattern {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    domain: row.get(3)?,
                    trigger: row.get(4)?,
                    strategy: row.get(5)?,
                    source_steps: row.get(6)?,
                    success_count: row.get(7)?,
                    failure_count: row.get(8)?,
                    technique_class: row.get(9)?,
                    deprecated: row.get(10)?,
                    created_at: row.get(11)?,
                    last_used_at: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn insert_pattern(
        &self,
        name: &str,
        description: &str,
        domain: Option<&str>,
        trigger_text: &str,
        strategy: &str,
        source_steps_json: &str,
        technique_class: Option<&str>,
    ) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO patterns (id, name, description, domain, trigger_text, strategy, source_steps, technique_class, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![id, name, description, domain, trigger_text, strategy, source_steps_json, technique_class, now],
        )?;
        Ok(id)
    }

    pub fn find_pattern_by_trigger(
        &self,
        domain: &str,
        trigger_text: &str,
    ) -> Result<Option<crate::models::patterns::Pattern>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, domain, trigger_text, strategy, source_steps, success_count, failure_count, technique_class, deprecated, created_at, last_used_at FROM patterns WHERE deprecated = 0 AND domain = ?1 AND trigger_text LIKE '%' || ?2 || '%' LIMIT 1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![domain, trigger_text], |row| {
            Ok(crate::models::patterns::Pattern {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                domain: row.get(3)?,
                trigger: row.get(4)?,
                strategy: row.get(5)?,
                source_steps: row.get(6)?,
                success_count: row.get(7)?,
                failure_count: row.get(8)?,
                technique_class: row.get(9)?,
                deprecated: row.get(10)?,
                created_at: row.get(11)?,
                last_used_at: row.get(12)?,
            })
        })?;
        match rows.next() {
            Some(Ok(p)) => Ok(Some(p)),
            Some(Err(e)) => Err(DbError::from(e)),
            None => Ok(None),
        }
    }

    pub fn increment_pattern_success(&self, pattern_id: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE patterns SET success_count = success_count + 1 WHERE id = ?1",
            rusqlite::params![pattern_id],
        )?;
        Ok(())
    }

    pub fn increment_pattern_failure(&self, pattern_id: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE patterns SET failure_count = failure_count + 1 WHERE id = ?1",
            rusqlite::params![pattern_id],
        )?;
        Ok(())
    }

    pub fn update_pattern_last_used(&self, pattern_id: &str) -> Result<(), DbError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "UPDATE patterns SET last_used_at = ?1 WHERE id = ?2",
            rusqlite::params![now, pattern_id],
        )?;
        Ok(())
    }

    // ---- Agent recording ----

    pub fn record_orchestrator_decision(
        &self,
        attempt_id: Option<&str>,
        decision_type: &str,
        decision: &str,
        reasoning: Option<&str>,
    ) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO orchestrator_decisions (id, attempt_id, decision_type, decision, reasoning, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, attempt_id, decision_type, decision, reasoning, now],
        )?;
        Ok(id)
    }

    pub fn record_critic_evaluation(
        &self,
        step_id: &str,
        attempt_id: &str,
        prediction: &str,
        confidence: f64,
        reasoning: &str,
    ) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO critic_evaluations (id, step_id, attempt_id, prediction, confidence, reasoning, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, step_id, attempt_id, prediction, confidence, reasoning, now],
        )?;
        Ok(id)
    }

    pub fn record_council_session(
        &self,
        trigger_type: &str,
        problem_id: &str,
        attempt_id: Option<&str>,
        model: &str,
        transcript: &str,
        findings_count: u32,
    ) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let models_json = format!("[\"{}\"]", model);
        let conn = self.conn();
        conn.execute(
            "INSERT INTO council_sessions (id, trigger_type, problem_id, attempt_id, council_models, transcript, findings_count, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, trigger_type, problem_id, attempt_id, models_json, transcript, findings_count, now],
        )?;
        Ok(id)
    }

    pub fn record_council_finding(
        &self,
        session_id: &str,
        finding_type: &str,
        summary: &str,
        detail: &str,
        target_agent: Option<&str>,
    ) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO council_findings (id, session_id, finding_type, summary, detail, consensus, target_agent, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'auto', ?6, ?7)",
            rusqlite::params![id, session_id, finding_type, summary, detail, target_agent, now],
        )?;
        Ok(id)
    }

    pub fn get_findings_for_problem(
        &self,
        problem_id: &str,
    ) -> Result<Vec<crate::models::council::CouncilFinding>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT f.id, f.session_id, f.finding_type, f.summary, f.detail, f.consensus, f.dissent, f.target_agent, f.acted_on \
             FROM council_findings f \
             JOIN council_sessions s ON f.session_id = s.id \
             WHERE s.problem_id = ?1 \
             ORDER BY f.created_at DESC",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![problem_id], |row| {
                Ok(crate::models::council::CouncilFinding {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    finding_type: row.get(2)?,
                    summary: row.get(3)?,
                    detail: row.get(4)?,
                    consensus: row.get(5)?,
                    dissent: row.get(6)?,
                    target_agent: row.get(7)?,
                    acted_on: row.get::<_, i32>(8).map(|v| v != 0).unwrap_or(false),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn record_scout_query(
        &self,
        trigger_type: &str,
        query_text: &str,
        sources: &str,
    ) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO scout_queries (id, trigger_type, query_text, sources_queried, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, trigger_type, query_text, sources, now],
        )?;
        Ok(id)
    }

    // ---- Management System: Extended Queries ----

    /// List all attempts for a problem with aggregated step stats.
    pub fn list_attempts_for_problem(
        &self,
        problem_id: &str,
    ) -> Result<Vec<crate::models::management::AttemptSummary>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.problem_id, a.attempt_number, a.status, a.strategy,
                    a.step_count, a.steps_verified, a.steps_rejected,
                    a.coverage, a.efficiency, a.models_used,
                    a.started_at, a.ended_at
             FROM attempts a
             WHERE a.problem_id = ?1
             ORDER BY a.started_at DESC",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![problem_id], |row| {
                Ok(crate::models::management::AttemptSummary {
                    id: row.get(0)?,
                    problem_id: row.get(1)?,
                    attempt_number: row.get(2)?,
                    status: row.get(3)?,
                    strategy: row.get(4)?,
                    step_count: row.get::<_, Option<i32>>(5)?.unwrap_or(0),
                    steps_verified: row.get::<_, Option<i32>>(6)?.unwrap_or(0),
                    steps_rejected: row.get::<_, Option<i32>>(7)?.unwrap_or(0),
                    coverage: row.get(8)?,
                    efficiency: row.get(9)?,
                    models_used: row.get(10)?,
                    started_at: row.get(11)?,
                    ended_at: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Search problems by text query with optional filters.
    pub fn search_problems(
        &self,
        query: &str,
        domain: Option<&str>,
        status: Option<&str>,
        difficulty: Option<&str>,
    ) -> Result<Vec<crate::models::proof::Problem>, DbError> {
        let conn = self.conn();
        let like_pattern = format!("%{}%", query);
        let mut sql = String::from(
            "SELECT id, statement, formal_statement, domain, source, status, created_at, solved_at, total_attempts, total_steps, known_answer, title, difficulty, metadata
             FROM problems WHERE (statement LIKE ?1 OR title LIKE ?1 OR source LIKE ?1)"
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(like_pattern)];
        if let Some(d) = domain {
            params.push(Box::new(d.to_string()));
            sql.push_str(&format!(" AND domain = ?{}", params.len()));
        }
        if let Some(s) = status {
            params.push(Box::new(s.to_string()));
            sql.push_str(&format!(" AND status = ?{}", params.len()));
        }
        if let Some(df) = difficulty {
            params.push(Box::new(df.to_string()));
            sql.push_str(&format!(" AND difficulty = ?{}", params.len()));
        }
        sql.push_str(" ORDER BY created_at DESC");
        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(crate::models::proof::Problem {
                    id: row.get(0)?,
                    statement: row.get(1)?,
                    formal_statement: row.get(2)?,
                    domain: row.get(3)?,
                    source: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                    solved_at: row.get(7)?,
                    total_attempts: row.get(8)?,
                    total_steps: row.get(9)?,
                    known_answer: row.get(10)?,
                    title: row.get(11)?,
                    difficulty: row.get(12)?,
                    metadata: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Update a problem's title.
    pub fn update_problem_title(&self, id: &str, title: &str) -> Result<(), DbError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "UPDATE problems SET title = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![title, now, id],
        )?;
        Ok(())
    }

    /// Update a problem's difficulty.
    pub fn update_problem_difficulty(&self, id: &str, difficulty: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE problems SET difficulty = ?1 WHERE id = ?2",
            rusqlite::params![difficulty, id],
        )?;
        Ok(())
    }
}
