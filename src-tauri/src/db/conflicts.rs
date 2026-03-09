use super::{Database, DbError};
use crate::models::management::Conflict;

impl Database {
    pub fn create_conflict(
        &self,
        attempt_id: &str,
        claim_a_id: &str,
        claim_b_id: &str,
        conflict_type: &str,
        severity: &str,
        description: &str,
    ) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO conflicts (id, attempt_id, claim_a_id, claim_b_id, conflict_type, severity, description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, attempt_id, claim_a_id, claim_b_id, conflict_type, severity, description, now],
        )?;
        Ok(id)
    }

    pub fn get_conflicts_for_attempt(&self, attempt_id: &str) -> Result<Vec<Conflict>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, claim_a_id, claim_b_id, conflict_type, severity, description, resolution, resolution_step, resolved_at, created_at
             FROM conflicts WHERE attempt_id = ?1 ORDER BY created_at"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![attempt_id], Self::row_to_conflict)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_unresolved_conflicts(&self, attempt_id: &str) -> Result<Vec<Conflict>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, claim_a_id, claim_b_id, conflict_type, severity, description, resolution, resolution_step, resolved_at, created_at
             FROM conflicts WHERE attempt_id = ?1 AND resolution IS NULL ORDER BY created_at"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![attempt_id], Self::row_to_conflict)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn resolve_conflict(
        &self,
        id: &str,
        resolution: &str,
        resolution_step: Option<&str>,
    ) -> Result<(), DbError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "UPDATE conflicts SET resolution = ?1, resolution_step = ?2, resolved_at = ?3 WHERE id = ?4",
            rusqlite::params![resolution, resolution_step, now, id],
        )?;
        Ok(())
    }

    fn row_to_conflict(row: &rusqlite::Row) -> rusqlite::Result<Conflict> {
        Ok(Conflict {
            id: row.get(0)?,
            attempt_id: row.get(1)?,
            claim_a_id: row.get(2)?,
            claim_b_id: row.get(3)?,
            conflict_type: row.get(4)?,
            severity: row.get(5)?,
            description: row.get(6)?,
            resolution: row.get(7)?,
            resolution_step: row.get(8)?,
            resolved_at: row.get(9)?,
            created_at: row.get(10)?,
        })
    }
}
