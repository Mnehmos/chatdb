use super::{Database, DbError};
use crate::models::dag::Branch;

impl Database {
    /// Create a new branch for an attempt. Returns the auto-increment branch ID.
    pub fn create_branch(
        &self,
        attempt_id: &str,
        parent_branch: i32,
        fork_step: Option<i32>,
        fork_reason: Option<&str>,
        direction: Option<&str>,
    ) -> Result<i64, DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO branches (attempt_id, parent_branch, fork_step, fork_reason, direction, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
            rusqlite::params![attempt_id, parent_branch, fork_step, fork_reason, direction, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }

    /// Get a branch by ID.
    pub fn get_branch(&self, id: i64) -> Result<Branch, DbError> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, attempt_id, parent_branch, fork_step, fork_reason, direction,
                    status, conclusion, conclusion_value, step_count, created_at
             FROM branches WHERE id = ?1",
            [id],
            |row| Ok(Self::row_to_branch(row)),
        )
        .map_err(|_| DbError::NotFound(format!("branch {}", id)))
    }

    /// Get all branches for an attempt.
    pub fn get_branches(&self, attempt_id: &str) -> Result<Vec<Branch>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, parent_branch, fork_step, fork_reason, direction,
                    status, conclusion, conclusion_value, step_count, created_at
             FROM branches WHERE attempt_id = ?1
             ORDER BY id ASC",
        )?;
        let branches = stmt
            .query_map([attempt_id], |row| Ok(Self::row_to_branch(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(branches)
    }

    /// Get active branches for an attempt.
    pub fn get_active_branches(&self, attempt_id: &str) -> Result<Vec<Branch>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, parent_branch, fork_step, fork_reason, direction,
                    status, conclusion, conclusion_value, step_count, created_at
             FROM branches WHERE attempt_id = ?1 AND status = 'active'
             ORDER BY id ASC",
        )?;
        let branches = stmt
            .query_map([attempt_id], |row| Ok(Self::row_to_branch(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(branches)
    }

    /// Update branch status (active → completed | abandoned | merged).
    pub fn update_branch_status(&self, id: i64, status: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE branches SET status = ?1 WHERE id = ?2",
            rusqlite::params![status, id],
        )?;
        Ok(())
    }

    /// Close a branch with conclusion details.
    pub fn close_branch(
        &self,
        id: i64,
        status: &str,
        conclusion: Option<&str>,
        conclusion_value: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE branches SET status = ?1, conclusion = ?2, conclusion_value = ?3 WHERE id = ?4",
            rusqlite::params![status, conclusion, conclusion_value, id],
        )?;
        Ok(())
    }

    /// Increment step count for a branch.
    pub fn increment_branch_steps(&self, id: i64) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE branches SET step_count = step_count + 1 WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    /// Get verified chain for a specific branch.
    /// Includes inherited nodes from the parent branch up to the fork point,
    /// plus all nodes created on this branch.
    pub fn get_branch_verified_chain(
        &self,
        attempt_id: &str,
        branch_id: i64,
    ) -> Result<Vec<(String, u32, String, String, String)>, DbError> {
        let conn = self.conn();
        // For the main branch (or when no branching data exists), fall back to
        // the legacy attempt-wide query via proof_nodes.
        // For child branches, include parent chain up to fork_step + own nodes.
        let mut stmt = conn.prepare(
            "SELECT s.id, s.step_number, s.model, s.proposal_natural, COALESCE(s.proposal_formal, '')
             FROM steps s
             JOIN proof_nodes pn ON pn.step_id = s.id
             WHERE pn.attempt_id = ?1
               AND s.verified = 1
               AND (
                   pn.branch_id = ?2
                   OR (
                       pn.branch_id = (SELECT parent_branch FROM branches WHERE id = ?2)
                       AND pn.sequence_number <= COALESCE((SELECT fork_step FROM branches WHERE id = ?2), 999999)
                   )
               )
             ORDER BY s.step_number"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![attempt_id, branch_id], |row| {
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

    fn row_to_branch(row: &rusqlite::Row) -> Branch {
        Branch {
            id: row.get(0).unwrap_or(0),
            attempt_id: row.get(1).unwrap_or_default(),
            parent_branch: row.get(2).unwrap_or(0),
            fork_step: row.get(3).unwrap_or(None),
            fork_reason: row.get(4).unwrap_or(None),
            direction: row.get(5).unwrap_or(None),
            status: row.get(6).unwrap_or_default(),
            conclusion: row.get(7).unwrap_or(None),
            conclusion_value: row.get(8).unwrap_or(None),
            step_count: row.get(9).unwrap_or(0),
            created_at: row.get(10).unwrap_or_default(),
        }
    }
}
