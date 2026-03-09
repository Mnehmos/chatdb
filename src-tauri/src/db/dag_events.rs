use super::{Database, DbError};
use crate::models::dag::DagEvent;

impl Database {
    pub fn append_dag_event(
        &self,
        attempt_id: &str,
        event_type: &str,
        payload: &str,
        agent_role: &str,
    ) -> Result<i64, DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();

        // Atomic: compute next sequence number and insert in one statement
        // to prevent race conditions with concurrent appends.
        conn.execute(
            "INSERT INTO dag_events (attempt_id, event_type, payload, agent_role, sequence_number, created_at)
             VALUES (?1, ?2, ?3, ?4,
                     (SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM dag_events WHERE attempt_id = ?1),
                     ?5)",
            rusqlite::params![attempt_id, event_type, payload, agent_role, now],
        )?;

        let seq: i64 = conn
            .query_row(
                "SELECT MAX(sequence_number) FROM dag_events WHERE attempt_id = ?1",
                [attempt_id],
                |row| row.get(0),
            )
            .unwrap_or(1);

        Ok(seq)
    }

    pub fn get_dag_events(
        &self,
        attempt_id: &str,
        since_sequence: i64,
    ) -> Result<Vec<DagEvent>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, event_type, payload, agent_role, sequence_number, created_at
             FROM dag_events WHERE attempt_id = ?1 AND sequence_number > ?2
             ORDER BY sequence_number",
        )?;
        let events = stmt
            .query_map(rusqlite::params![attempt_id, since_sequence], |row| {
                Ok(DagEvent {
                    id: row.get(0)?,
                    attempt_id: row.get(1)?,
                    event_type: row.get(2)?,
                    payload: row.get(3)?,
                    agent_role: row.get(4)?,
                    sequence_number: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(events)
    }
}
