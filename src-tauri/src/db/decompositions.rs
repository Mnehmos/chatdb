use super::{Database, DbError};

/// A recorded decomposition session — the decomposer's analysis of a problem
/// and the obligation graph it generated.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Decomposition {
    pub id: String,
    pub attempt_id: String,
    pub trigger: String,
    pub trigger_node_id: Option<String>,
    pub problem_profile: String,
    pub obligation_graph: String,
    pub obligations_created: i32,
    pub model_used: String,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub created_at: String,
}

impl Database {
    pub fn record_decomposition(
        &self,
        attempt_id: &str,
        trigger: &str,
        trigger_node_id: Option<&str>,
        problem_profile: &str,
        obligation_graph: &str,
        obligations_created: i32,
        model_used: &str,
        tokens_in: Option<u32>,
        tokens_out: Option<u32>,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO decompositions (id, attempt_id, trigger, trigger_node_id,
             problem_profile, obligation_graph, obligations_created, model_used,
             tokens_in, tokens_out, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            rusqlite::params![
                id,
                attempt_id,
                trigger,
                trigger_node_id,
                problem_profile,
                obligation_graph,
                obligations_created,
                model_used,
                tokens_in,
                tokens_out,
                now,
            ],
        )?;
        Ok(id)
    }

    pub fn get_decompositions(&self, attempt_id: &str) -> Result<Vec<Decomposition>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, trigger, trigger_node_id, problem_profile,
                    obligation_graph, obligations_created, model_used,
                    tokens_in, tokens_out, created_at
             FROM decompositions WHERE attempt_id = ?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt
            .query_map([attempt_id], |row| {
                Ok(Decomposition {
                    id: row.get(0)?,
                    attempt_id: row.get(1)?,
                    trigger: row.get(2)?,
                    trigger_node_id: row.get(3)?,
                    problem_profile: row.get(4)?,
                    obligation_graph: row.get(5)?,
                    obligations_created: row.get(6)?,
                    model_used: row.get(7)?,
                    tokens_in: row.get(8)?,
                    tokens_out: row.get(9)?,
                    created_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Create an obligation with dependency tracking and decomposition linkage.
    pub fn create_obligation_with_deps(
        &self,
        attempt_id: &str,
        branch_id: i32,
        parent_node_id: &str,
        description: &str,
        obligation_type: &str,
        priority: f64,
        confidence: f64,
        source_layer: Option<i32>,
        depends_on: Option<&str>,
        decomposition_id: Option<&str>,
        satisfaction_criteria: Option<&str>,
        max_steps: i32,
    ) -> Result<String, DbError> {
        // Clamp max_steps: floor of 20 ensures start-of-proof obligations get
        // enough budget to be worked through; ceiling of 50 prevents absurd budgets.
        let capped_max_steps = max_steps.clamp(20, 50);
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO obligations (id, attempt_id, branch_id, parent_node_id,
             description, obligation_type, priority, confidence, source_layer,
             status, depends_on, decomposition_id, satisfaction_criteria, max_steps, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'open',?10,?11,?12,?13,?14)",
            rusqlite::params![
                id,
                attempt_id,
                branch_id,
                parent_node_id,
                description,
                obligation_type,
                priority,
                confidence,
                source_layer,
                depends_on,
                decomposition_id,
                satisfaction_criteria,
                capped_max_steps,
                now,
            ],
        )?;
        Ok(id)
    }

    /// Supersede all obligations from a specific decomposition (used during re-decomposition).
    pub fn supersede_decomposition_obligations(
        &self,
        decomposition_id: &str,
        superseded_by: &str,
    ) -> Result<u32, DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        let count = conn.execute(
            "UPDATE obligations SET status = 'superseded', superseded_by = ?1, closed_at = ?2
             WHERE decomposition_id = ?3 AND status IN ('open', 'assigned')",
            rusqlite::params![superseded_by, now, decomposition_id],
        )?;
        Ok(count as u32)
    }
}
