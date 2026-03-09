use super::{Database, DbError};
use crate::models::tool_use::{ObligationResolution, ScoutSession};

impl Database {
    // ---- scout_sessions ----

    pub fn create_scout_session(
        &self,
        attempt_id: &str,
        branch_id: Option<i32>,
        problem_id: &str,
        obligation_id: Option<&str>,
        scope: &str,
        trigger_type: &str,
        routing_plan_json: &str,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scout_sessions (
                id, attempt_id, branch_id, problem_id, obligation_id,
                scope, trigger_type, routing_plan_json, status, created_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'running',?9)",
            rusqlite::params![
                id,
                attempt_id,
                branch_id,
                problem_id,
                obligation_id,
                scope,
                trigger_type,
                routing_plan_json,
                now,
            ],
        )?;
        Ok(id)
    }

    pub fn complete_scout_session(
        &self,
        id: &str,
        status: &str,
        best_resolution_status: Option<&str>,
        best_tool_run_id: Option<&str>,
        best_artifact_id: Option<&str>,
        confidence: Option<f64>,
    ) -> Result<(), DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE scout_sessions SET status = ?1, best_resolution_status = ?2,
             best_tool_run_id = ?3, best_artifact_id = ?4, confidence = ?5,
             completed_at = ?6 WHERE id = ?7",
            rusqlite::params![
                status,
                best_resolution_status,
                best_tool_run_id,
                best_artifact_id,
                confidence,
                now,
                id,
            ],
        )?;
        Ok(())
    }

    pub fn get_scout_session(&self, id: &str) -> Result<ScoutSession, DbError> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, attempt_id, branch_id, problem_id, obligation_id,
                    scope, trigger_type, routing_plan_json, status,
                    best_resolution_status, best_tool_run_id, best_artifact_id,
                    confidence, created_at, completed_at
             FROM scout_sessions WHERE id = ?1",
            [id],
            |row| Ok(Self::row_to_scout_session(row)),
        )
        .map_err(|_| DbError::NotFound(format!("scout_session {}", id)))
    }

    pub fn get_scout_sessions_for_obligation(
        &self,
        obligation_id: &str,
    ) -> Result<Vec<ScoutSession>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, problem_id, obligation_id,
                    scope, trigger_type, routing_plan_json, status,
                    best_resolution_status, best_tool_run_id, best_artifact_id,
                    confidence, created_at, completed_at
             FROM scout_sessions WHERE obligation_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map([obligation_id], |row| Ok(Self::row_to_scout_session(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn row_to_scout_session(row: &rusqlite::Row) -> ScoutSession {
        ScoutSession {
            id: row.get(0).unwrap_or_default(),
            attempt_id: row.get(1).unwrap_or_default(),
            branch_id: row.get(2).unwrap_or(None),
            problem_id: row.get(3).unwrap_or_default(),
            obligation_id: row.get(4).unwrap_or(None),
            scope: row.get(5).unwrap_or_default(),
            trigger_type: row.get(6).unwrap_or_default(),
            routing_plan_json: row.get(7).unwrap_or_default(),
            status: row.get(8).unwrap_or_default(),
            best_resolution_status: row.get(9).unwrap_or(None),
            best_tool_run_id: row.get(10).unwrap_or(None),
            best_artifact_id: row.get(11).unwrap_or(None),
            confidence: row.get(12).unwrap_or(None),
            created_at: row.get(13).unwrap_or_default(),
            completed_at: row.get(14).unwrap_or(None),
        }
    }

    // ---- obligation_resolutions ----

    pub fn create_obligation_resolution(
        &self,
        attempt_id: &str,
        obligation_id: &str,
        scout_session_id: &str,
        status: &str,
        source_tier: i32,
        source_provider: &str,
        source_reference: &str,
        source_url: Option<&str>,
        source_formalism: &str,
        mapping_text: &str,
        translation_json: Option<&str>,
        mechanical_verification: bool,
        confidence: f64,
        corpus_entry_id: Option<&str>,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO obligation_resolutions (
                id, attempt_id, obligation_id, scout_session_id, status,
                source_tier, source_provider, source_reference, source_url,
                source_formalism, mapping_text, translation_json,
                mechanical_verification, confidence, corpus_entry_id, created_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            rusqlite::params![
                id,
                attempt_id,
                obligation_id,
                scout_session_id,
                status,
                source_tier,
                source_provider,
                source_reference,
                source_url,
                source_formalism,
                mapping_text,
                translation_json,
                mechanical_verification as i32,
                confidence,
                corpus_entry_id,
                now,
            ],
        )?;
        Ok(id)
    }

    pub fn get_resolutions_for_obligation(
        &self,
        obligation_id: &str,
    ) -> Result<Vec<ObligationResolution>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, obligation_id, scout_session_id, status,
                    source_tier, source_provider, source_reference, source_url,
                    source_formalism, mapping_text, translation_json,
                    mechanical_verification, confidence, corpus_entry_id, created_at
             FROM obligation_resolutions WHERE obligation_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map([obligation_id], |row| {
                Ok(ObligationResolution {
                    id: row.get(0).unwrap_or_default(),
                    attempt_id: row.get(1).unwrap_or_default(),
                    obligation_id: row.get(2).unwrap_or_default(),
                    scout_session_id: row.get(3).unwrap_or_default(),
                    status: row.get(4).unwrap_or_default(),
                    source_tier: row.get(5).unwrap_or(0),
                    source_provider: row.get(6).unwrap_or_default(),
                    source_reference: row.get(7).unwrap_or_default(),
                    source_url: row.get(8).unwrap_or(None),
                    source_formalism: row.get(9).unwrap_or_default(),
                    mapping_text: row.get(10).unwrap_or_default(),
                    translation_json: row.get(11).unwrap_or(None),
                    mechanical_verification: row.get::<_, i32>(12).unwrap_or(0) != 0,
                    confidence: row.get(13).unwrap_or(0.0),
                    corpus_entry_id: row.get(14).unwrap_or(None),
                    created_at: row.get(15).unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }
}
