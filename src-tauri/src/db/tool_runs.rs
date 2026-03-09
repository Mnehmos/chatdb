use super::{Database, DbError};
use crate::models::tool_use::{ToolArtifact, ToolRun};

impl Database {
    // ---- tool_runs ----

    pub fn create_tool_run(
        &self,
        attempt_id: &str,
        branch_id: Option<i32>,
        step_id: Option<&str>,
        step_number: Option<u32>,
        obligation_id: Option<&str>,
        parent_tool_run_id: Option<&str>,
        session_id: Option<&str>,
        agent_role: &str,
        trigger_kind: &str,
        tool_name: &str,
        provider: &str,
        tier: Option<i32>,
        query_json: &str,
        input_hash: Option<&str>,
        required_by_policy: bool,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO tool_runs (
                id, attempt_id, branch_id, step_id, step_number, obligation_id,
                parent_tool_run_id, session_id, agent_role, trigger_kind,
                tool_name, provider, tier, query_json, input_hash,
                required_by_policy, status, created_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,'requested',?17)",
            rusqlite::params![
                id,
                attempt_id,
                branch_id,
                step_id,
                step_number,
                obligation_id,
                parent_tool_run_id,
                session_id,
                agent_role,
                trigger_kind,
                tool_name,
                provider,
                tier,
                query_json,
                input_hash,
                required_by_policy as i32,
                now,
            ],
        )?;
        if let Some(step_id) = step_id {
            conn.execute(
                "UPDATE steps SET tool_call_count = COALESCE(tool_call_count, 0) + 1 WHERE id = ?1",
                rusqlite::params![step_id],
            )?;
        }
        Ok(id)
    }

    pub fn complete_tool_run(
        &self,
        id: &str,
        status: &str,
        hit: Option<bool>,
        latency_ms: Option<u32>,
        error_message: Option<&str>,
        raw_result_json: Option<&str>,
        result_summary: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE tool_runs SET status = ?1, hit = ?2, latency_ms = ?3,
             error_message = ?4, raw_result_json = ?5, result_summary = ?6,
             completed_at = ?7 WHERE id = ?8",
            rusqlite::params![
                status,
                hit.map(|b| b as i32),
                latency_ms,
                error_message,
                raw_result_json,
                result_summary,
                now,
                id,
            ],
        )?;
        Ok(())
    }

    pub fn backfill_tool_runs_step_id(
        &self,
        tool_run_ids: &[String],
        step_id: &str,
    ) -> Result<(), DbError> {
        if tool_run_ids.is_empty() {
            return Ok(());
        }
        let conn = self.conn();
        for rid in tool_run_ids {
            conn.execute(
                "UPDATE tool_runs SET step_id = ?1 WHERE id = ?2",
                rusqlite::params![step_id, rid],
            )?;
        }
        let tool_call_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tool_runs WHERE step_id = ?1",
            rusqlite::params![step_id],
            |row| row.get(0),
        )?;
        conn.execute(
            "UPDATE steps SET tool_call_count = ?1 WHERE id = ?2",
            rusqlite::params![tool_call_count, step_id],
        )?;
        Ok(())
    }

    pub fn get_tool_runs_for_step(&self, step_id: &str) -> Result<Vec<ToolRun>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, step_id, step_number, obligation_id,
                    parent_tool_run_id, session_id, agent_role, trigger_kind,
                    tool_name, provider, tier, query_json, input_hash,
                    required_by_policy, status, hit, latency_ms, error_message,
                    raw_result_json, result_summary, created_at, completed_at
             FROM tool_runs WHERE step_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt
            .query_map([step_id], |row| Ok(Self::row_to_tool_run(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn get_tool_runs_for_session(&self, session_id: &str) -> Result<Vec<ToolRun>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, step_id, step_number, obligation_id,
                    parent_tool_run_id, session_id, agent_role, trigger_kind,
                    tool_name, provider, tier, query_json, input_hash,
                    required_by_policy, status, hit, latency_ms, error_message,
                    raw_result_json, result_summary, created_at, completed_at
             FROM tool_runs WHERE session_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt
            .query_map([session_id], |row| Ok(Self::row_to_tool_run(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn get_tool_runs_for_obligation(
        &self,
        obligation_id: &str,
    ) -> Result<Vec<ToolRun>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, step_id, step_number, obligation_id,
                    parent_tool_run_id, session_id, agent_role, trigger_kind,
                    tool_name, provider, tier, query_json, input_hash,
                    required_by_policy, status, hit, latency_ms, error_message,
                    raw_result_json, result_summary, created_at, completed_at
             FROM tool_runs WHERE obligation_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt
            .query_map([obligation_id], |row| Ok(Self::row_to_tool_run(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn row_to_tool_run(row: &rusqlite::Row) -> ToolRun {
        ToolRun {
            id: row.get(0).unwrap_or_default(),
            attempt_id: row.get(1).unwrap_or_default(),
            branch_id: row.get(2).unwrap_or(None),
            step_id: row.get(3).unwrap_or(None),
            step_number: row.get(4).unwrap_or(None),
            obligation_id: row.get(5).unwrap_or(None),
            parent_tool_run_id: row.get(6).unwrap_or(None),
            session_id: row.get(7).unwrap_or(None),
            agent_role: row.get(8).unwrap_or_default(),
            trigger_kind: row.get(9).unwrap_or_default(),
            tool_name: row.get(10).unwrap_or_default(),
            provider: row.get(11).unwrap_or_default(),
            tier: row.get(12).unwrap_or(None),
            query_json: row.get(13).unwrap_or_default(),
            input_hash: row.get(14).unwrap_or(None),
            required_by_policy: row.get::<_, i32>(15).unwrap_or(0) != 0,
            status: row.get(16).unwrap_or_default(),
            hit: row
                .get::<_, Option<i32>>(17)
                .unwrap_or(None)
                .map(|v| v != 0),
            latency_ms: row.get(18).unwrap_or(None),
            error_message: row.get(19).unwrap_or(None),
            raw_result_json: row.get(20).unwrap_or(None),
            result_summary: row.get(21).unwrap_or(None),
            created_at: row.get(22).unwrap_or_default(),
            completed_at: row.get(23).unwrap_or(None),
        }
    }

    // ---- tool_artifacts ----

    pub fn insert_tool_artifact(
        &self,
        tool_run_id: &str,
        attempt_id: &str,
        obligation_id: Option<&str>,
        artifact_index: i32,
        artifact_kind: &str,
        external_ref: Option<&str>,
        title: Option<&str>,
        statement: Option<&str>,
        formalism: Option<&str>,
        url: Option<&str>,
        relevance_score: Option<f64>,
        raw_json: &str,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO tool_artifacts (
                id, tool_run_id, attempt_id, obligation_id, artifact_index,
                artifact_kind, external_ref, title, statement, formalism,
                url, relevance_score, raw_json, created_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            rusqlite::params![
                id,
                tool_run_id,
                attempt_id,
                obligation_id,
                artifact_index,
                artifact_kind,
                external_ref,
                title,
                statement,
                formalism,
                url,
                relevance_score,
                raw_json,
                now,
            ],
        )?;
        Ok(id)
    }

    pub fn get_artifacts_for_tool_run(
        &self,
        tool_run_id: &str,
    ) -> Result<Vec<ToolArtifact>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, tool_run_id, attempt_id, obligation_id, artifact_index,
                    artifact_kind, external_ref, title, statement, formalism,
                    url, relevance_score, matched, raw_json, created_at
             FROM tool_artifacts WHERE tool_run_id = ?1 ORDER BY artifact_index",
        )?;
        let rows = stmt
            .query_map([tool_run_id], |row| {
                Ok(ToolArtifact {
                    id: row.get(0).unwrap_or_default(),
                    tool_run_id: row.get(1).unwrap_or_default(),
                    attempt_id: row.get(2).unwrap_or_default(),
                    obligation_id: row.get(3).unwrap_or(None),
                    artifact_index: row.get(4).unwrap_or(0),
                    artifact_kind: row.get(5).unwrap_or_default(),
                    external_ref: row.get(6).unwrap_or(None),
                    title: row.get(7).unwrap_or(None),
                    statement: row.get(8).unwrap_or(None),
                    formalism: row.get(9).unwrap_or(None),
                    url: row.get(10).unwrap_or(None),
                    relevance_score: row.get(11).unwrap_or(None),
                    matched: row.get::<_, i32>(12).unwrap_or(0) != 0,
                    raw_json: row.get(13).unwrap_or_default(),
                    created_at: row.get(14).unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }
}
