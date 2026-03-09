use super::{Database, DbError};
use crate::models::management::AfterActionReportRecord;

impl Database {
    pub fn create_after_action_report(
        &self,
        attempt_id: &str,
        problem_id: &str,
        coverage: Option<f64>,
        soundness: Option<f64>,
        budget_efficiency: Option<f64>,
        death_spirals: i32,
        contradictions: i32,
        obligations_total: i32,
        obligations_closed: i32,
        findings_json: Option<&str>,
        recommendations: Option<&str>,
        training_label: Option<&str>,
    ) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "INSERT OR REPLACE INTO after_action_reports
             (id, attempt_id, problem_id, coverage, soundness, budget_efficiency, death_spirals, contradictions, obligations_total, obligations_closed, findings_json, recommendations, training_label, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![
                id, attempt_id, problem_id, coverage, soundness, budget_efficiency,
                death_spirals, contradictions, obligations_total, obligations_closed,
                findings_json, recommendations, training_label, now
            ],
        )?;
        Ok(id)
    }

    pub fn get_report_for_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<AfterActionReportRecord, DbError> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, attempt_id, problem_id, coverage, soundness, budget_efficiency, death_spirals, contradictions, obligations_total, obligations_closed, findings_json, recommendations, training_label, created_at
             FROM after_action_reports WHERE attempt_id = ?1",
            rusqlite::params![attempt_id],
            Self::row_to_aar,
        )
        .map_err(|_| DbError::NotFound(format!("AAR for attempt {}", attempt_id)))
    }

    pub fn get_reports_for_problem(
        &self,
        problem_id: &str,
    ) -> Result<Vec<AfterActionReportRecord>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, problem_id, coverage, soundness, budget_efficiency, death_spirals, contradictions, obligations_total, obligations_closed, findings_json, recommendations, training_label, created_at
             FROM after_action_reports WHERE problem_id = ?1 ORDER BY created_at DESC"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![problem_id], Self::row_to_aar)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn row_to_aar(row: &rusqlite::Row) -> rusqlite::Result<AfterActionReportRecord> {
        Ok(AfterActionReportRecord {
            id: row.get(0)?,
            attempt_id: row.get(1)?,
            problem_id: row.get(2)?,
            coverage: row.get(3)?,
            soundness: row.get(4)?,
            budget_efficiency: row.get(5)?,
            death_spirals: row.get(6)?,
            contradictions: row.get(7)?,
            obligations_total: row.get(8)?,
            obligations_closed: row.get(9)?,
            findings_json: row.get(10)?,
            recommendations: row.get(11)?,
            training_label: row.get(12)?,
            created_at: row.get(13)?,
        })
    }
}
