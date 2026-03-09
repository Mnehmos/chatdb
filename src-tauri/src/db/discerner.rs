use super::{Database, DbError};
use serde::{Deserialize, Serialize};

/// Mid-run discerner finding — persisted from an in-flight failure streak classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscernerFinding {
    pub id: String,
    pub attempt_id: String,
    pub step_number: u32,
    pub failure_streak: u32,
    /// JSON array of FailureEntry (serialized failure window).
    pub failure_window: String,
    /// "mechanical" | "model" | "gate" | "validator"
    pub classification: String,
    pub root_cause: String,
    pub recommendation: String,
    pub confidence: f64,
    /// "switch_model" | "add_backoff" | "retry" | "rephrase_prompt" | "continue"
    pub suggested_action: String,
    pub discerner_model: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscernerDecision {
    pub id: String,
    pub attempt_id: String,
    /// "mechanical" | "logical" | "mixed"
    pub kind: String,
    pub confidence: f32,
    pub mechanical_score: f32,
    pub logical_score: f32,
    pub reasoning: String,
    /// "same" | "new_approach" | "restructure"
    pub retry_rec: String,
    pub model: String,
    /// JSON array of failure summaries analyzed
    pub failures_json: String,
    pub created_at: String,
}

impl Database {
    /// Persist a mid-run Discerner finding (in-flight failure streak classification).
    pub fn record_discerner_finding(&self, f: &DiscernerFinding) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO discerner_findings \
             (id, attempt_id, step_number, failure_streak, failure_window, \
              classification, root_cause, recommendation, confidence, \
              suggested_action, discerner_model, created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            rusqlite::params![
                f.id,
                f.attempt_id,
                f.step_number,
                f.failure_streak,
                f.failure_window,
                f.classification,
                f.root_cause,
                f.recommendation,
                f.confidence,
                f.suggested_action,
                f.discerner_model,
                f.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn save_discerner_decision(&self, d: &DiscernerDecision) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO discerner_decisions
             (id, attempt_id, kind, confidence, mechanical_score, logical_score,
              reasoning, retry_rec, model, failures_json, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            rusqlite::params![
                d.id,
                d.attempt_id,
                d.kind,
                d.confidence,
                d.mechanical_score,
                d.logical_score,
                d.reasoning,
                d.retry_rec,
                d.model,
                d.failures_json,
                d.created_at,
            ],
        )?;
        Ok(())
    }
}
