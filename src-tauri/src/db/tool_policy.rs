use super::{Database, DbError};
use crate::models::tool_use::ToolPolicyViolation;

impl Database {
    pub fn record_tool_policy_violation(
        &self,
        attempt_id: &str,
        branch_id: Option<i32>,
        step_id: Option<&str>,
        step_number: Option<u32>,
        obligation_id: Option<&str>,
        agent_role: &str,
        policy_name: &str,
        required_tool: &str,
        observed_tool_runs_json: Option<&str>,
        action_taken: &str,
        notes: Option<&str>,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO tool_policy_violations (
                id, attempt_id, branch_id, step_id, step_number, obligation_id,
                agent_role, policy_name, required_tool, observed_tool_runs_json,
                action_taken, notes, created_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            rusqlite::params![
                id,
                attempt_id,
                branch_id,
                step_id,
                step_number,
                obligation_id,
                agent_role,
                policy_name,
                required_tool,
                observed_tool_runs_json,
                action_taken,
                notes,
                now,
            ],
        )?;
        Ok(id)
    }

    pub fn get_violations_for_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Vec<ToolPolicyViolation>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, step_id, step_number, obligation_id,
                    agent_role, policy_name, required_tool, observed_tool_runs_json,
                    action_taken, notes, created_at
             FROM tool_policy_violations WHERE attempt_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt
            .query_map([attempt_id], |row| {
                Ok(ToolPolicyViolation {
                    id: row.get(0).unwrap_or_default(),
                    attempt_id: row.get(1).unwrap_or_default(),
                    branch_id: row.get(2).unwrap_or(None),
                    step_id: row.get(3).unwrap_or(None),
                    step_number: row.get(4).unwrap_or(None),
                    obligation_id: row.get(5).unwrap_or(None),
                    agent_role: row.get(6).unwrap_or_default(),
                    policy_name: row.get(7).unwrap_or_default(),
                    required_tool: row.get(8).unwrap_or_default(),
                    observed_tool_runs_json: row.get(9).unwrap_or(None),
                    action_taken: row.get(10).unwrap_or_default(),
                    notes: row.get(11).unwrap_or(None),
                    created_at: row.get(12).unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }
}
