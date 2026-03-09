use super::{Database, DbError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatisfactionSignal {
    pub id: String,
    pub obligation_id: String,
    pub step_id: Option<String>,
    pub source: String, // "mechanical", "solver", "reviewer", "adversary"
    pub model_id: Option<String>,
    pub satisfies: bool,
    pub confidence: f64,
    pub note: Option<String>,
    pub created_at: String,
}

impl Database {
    pub fn record_satisfaction_signal(
        &self,
        obligation_id: &str,
        step_id: Option<&str>,
        source: &str,
        model_id: Option<&str>,
        satisfies: bool,
        confidence: f64,
        note: Option<&str>,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO obligation_signals \
             (id, obligation_id, step_id, source, model_id, satisfies, confidence, note, created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            rusqlite::params![id, obligation_id, step_id, source, model_id, satisfies as i32, confidence, note, now],
        )?;
        Ok(id)
    }

    /// Returns (yes_count, total_count) for majority calculation.
    pub fn get_obligation_tally(&self, obligation_id: &str) -> Result<(i64, i64), DbError> {
        let conn = self.conn();
        let (yes, total) = conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN satisfies = 1 THEN 1 ELSE 0 END), 0), COUNT(*) \
             FROM obligation_signals WHERE obligation_id = ?1",
            [obligation_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        Ok((yes, total))
    }

    /// Get all signals for an obligation (for display/debugging).
    pub fn get_obligation_signals(
        &self,
        obligation_id: &str,
    ) -> Result<Vec<SatisfactionSignal>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, obligation_id, step_id, source, model_id, satisfies, confidence, note, created_at \
             FROM obligation_signals WHERE obligation_id = ?1 ORDER BY created_at ASC"
        )?;
        let rows: Vec<SatisfactionSignal> = stmt
            .query_map([obligation_id], |row| {
                Ok(SatisfactionSignal {
                    id: row.get(0)?,
                    obligation_id: row.get(1)?,
                    step_id: row.get(2)?,
                    source: row.get(3)?,
                    model_id: row.get(4)?,
                    satisfies: row.get::<_, i32>(5)? != 0,
                    confidence: row.get(6)?,
                    note: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }
}
