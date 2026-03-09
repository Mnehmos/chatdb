//! Contrastive pair storage — rejected vs accepted steps at the same proof slot.
//!
//! When the solver produces a wrong step (rejected) at a step_number where
//! a verified step already exists, we record the pair. This creates clean
//! DPO/RLHF-style training examples: (rejected, accepted) at the same position.

use crate::db::{Database, DbError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContrastivePair {
    pub id: String,
    pub attempt_id: String,
    pub step_number: u32,
    pub accepted_step_id: String,
    pub rejected_step_id: String,
    pub rejection_reason: Option<String>,
    pub created_at: String,
}

impl Database {
    /// Record a (accepted, rejected) contrastive pair at the same step slot.
    pub fn record_contrastive_pair(
        &self,
        accepted_step_id: &str,
        rejected_step_id: &str,
        step_number: u32,
        attempt_id: &str,
        rejection_reason: Option<&str>,
    ) -> Result<String, DbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO contrastive_pairs
             (id, attempt_id, step_number, accepted_step_id, rejected_step_id, rejection_reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id, attempt_id, step_number,
                accepted_step_id, rejected_step_id,
                rejection_reason, now,
            ],
        )?;
        Ok(id)
    }

    /// List all contrastive pairs for an attempt — for training data export.
    pub fn list_contrastive_pairs(
        &self,
        attempt_id: &str,
    ) -> Result<Vec<ContrastivePair>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, step_number, accepted_step_id, rejected_step_id,
                    rejection_reason, created_at
             FROM contrastive_pairs WHERE attempt_id = ?1
             ORDER BY step_number ASC",
        )?;
        let pairs = stmt
            .query_map(rusqlite::params![attempt_id], |row| {
                Ok(ContrastivePair {
                    id: row.get(0)?,
                    attempt_id: row.get(1)?,
                    step_number: row.get(2)?,
                    accepted_step_id: row.get(3)?,
                    rejected_step_id: row.get(4)?,
                    rejection_reason: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(pairs)
    }

    /// Find the most recent verified step ID at a given step_number in an attempt.
    /// Returns None if no verified step exists at that slot yet.
    pub fn get_verified_step_id_at(&self, step_number: u32, attempt_id: &str) -> Option<String> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id FROM steps WHERE attempt_id = ?1 AND step_number = ?2 AND verified = 1
             ORDER BY created_at DESC LIMIT 1",
            rusqlite::params![attempt_id, step_number],
            |row| row.get(0),
        )
        .ok()
    }

    /// Find all rejected step IDs at a given step_number in an attempt.
    /// Used to backfill contrastive pairs when a step is verified (the accepted
    /// step arrives after the rejections in a linear chain).
    pub fn get_rejected_step_ids_at(
        &self,
        step_number: u32,
        attempt_id: &str,
    ) -> Vec<(String, Option<String>)> {
        let conn = self.conn();
        let mut stmt = match conn.prepare(
            "SELECT id, rejection_reason FROM steps
             WHERE attempt_id = ?1 AND step_number = ?2 AND verified = 0
             ORDER BY created_at ASC",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(rusqlite::params![attempt_id, step_number], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }
}
