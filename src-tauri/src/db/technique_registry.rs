use super::{Database, DbError};
use crate::models::dag::TechniqueEntry;

impl Database {
    pub fn seed_techniques(&self, entries: &[(&str, &str, &str)]) -> Result<u32, DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        let mut inserted = 0u32;
        for (problem_class, technique_family, description) in entries {
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM technique_registry WHERE problem_class = ?1 AND technique_family = ?2",
                rusqlite::params![problem_class, technique_family],
                |row| row.get(0),
            ).unwrap_or(false);
            if !exists {
                conn.execute(
                    "INSERT INTO technique_registry (problem_class, technique_family, description, source, created_at)
                     VALUES (?1, ?2, ?3, 'seed', ?4)",
                    rusqlite::params![problem_class, technique_family, description, now],
                )?;
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    pub fn get_techniques_for_class(
        &self,
        problem_class: &str,
    ) -> Result<Vec<TechniqueEntry>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, problem_class, technique_family, description, source, success_count, failure_count, last_used_at, created_at
             FROM technique_registry WHERE problem_class = ?1
             ORDER BY success_count DESC"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![problem_class], |row| {
                Ok(TechniqueEntry {
                    id: row.get(0)?,
                    problem_class: row.get(1)?,
                    technique_family: row.get(2)?,
                    description: row.get(3)?,
                    source: row.get(4)?,
                    success_count: row.get(5)?,
                    failure_count: row.get(6)?,
                    last_used_at: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn get_all_techniques(&self) -> Result<Vec<TechniqueEntry>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, problem_class, technique_family, description, source, success_count, failure_count, last_used_at, created_at
             FROM technique_registry ORDER BY problem_class, success_count DESC"
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(TechniqueEntry {
                    id: row.get(0)?,
                    problem_class: row.get(1)?,
                    technique_family: row.get(2)?,
                    description: row.get(3)?,
                    source: row.get(4)?,
                    success_count: row.get(5)?,
                    failure_count: row.get(6)?,
                    last_used_at: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn record_technique_use(&self, id: i64, success: bool) -> Result<(), DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        if success {
            conn.execute(
                "UPDATE technique_registry SET success_count = success_count + 1, last_used_at = ?2 WHERE id = ?1",
                rusqlite::params![id, now],
            )?;
        } else {
            conn.execute(
                "UPDATE technique_registry SET failure_count = failure_count + 1, last_used_at = ?2 WHERE id = ?1",
                rusqlite::params![id, now],
            )?;
        }
        Ok(())
    }
}
