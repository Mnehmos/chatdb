use super::{Database, DbError};
use crate::models::management::Claim;

impl Database {
    pub fn create_claim(
        &self,
        step_id: &str,
        attempt_id: &str,
        claim_type: &str,
        object: &str,
        scope_type: Option<&str>,
        scope_param: Option<&str>,
        scope_constraint: Option<&str>,
        direction: Option<&str>,
        value: Option<&str>,
        natural_text: &str,
        confidence: f64,
    ) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO claims (id, step_id, attempt_id, claim_type, object, scope_type, scope_param, scope_constraint, direction, value, natural_text, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                id, step_id, attempt_id, claim_type, object,
                scope_type, scope_param, scope_constraint,
                direction, value, natural_text, confidence, now
            ],
        )?;
        Ok(id)
    }

    pub fn get_claims_for_step(&self, step_id: &str) -> Result<Vec<Claim>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, step_id, attempt_id, claim_type, object, scope_type, scope_param, scope_constraint, direction, value, natural_text, confidence, verified, superseded_by, created_at
             FROM claims WHERE step_id = ?1 ORDER BY created_at"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![step_id], Self::row_to_claim)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_claims_for_attempt(&self, attempt_id: &str) -> Result<Vec<Claim>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, step_id, attempt_id, claim_type, object, scope_type, scope_param, scope_constraint, direction, value, natural_text, confidence, verified, superseded_by, created_at
             FROM claims WHERE attempt_id = ?1 ORDER BY created_at"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![attempt_id], Self::row_to_claim)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn supersede_claim(&self, id: &str, superseded_by: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE claims SET superseded_by = ?1 WHERE id = ?2",
            rusqlite::params![superseded_by, id],
        )?;
        Ok(())
    }

    pub fn verify_claim(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE claims SET verified = 1 WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    fn row_to_claim(row: &rusqlite::Row) -> rusqlite::Result<Claim> {
        Ok(Claim {
            id: row.get(0)?,
            step_id: row.get(1)?,
            attempt_id: row.get(2)?,
            claim_type: row.get(3)?,
            object: row.get(4)?,
            scope_type: row.get(5)?,
            scope_param: row.get(6)?,
            scope_constraint: row.get(7)?,
            direction: row.get(8)?,
            value: row.get(9)?,
            natural_text: row.get(10)?,
            confidence: row.get(11)?,
            verified: row.get::<_, i32>(12)? != 0,
            superseded_by: row.get(13)?,
            created_at: row.get(14)?,
        })
    }
}
