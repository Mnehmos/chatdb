use super::{Database, DbError};
use crate::models::dag::Obligation;

impl Database {
    pub fn create_obligation(
        &self,
        attempt_id: &str,
        branch_id: i32,
        parent_node_id: &str,
        description: &str,
        obligation_type: &str,
        priority: f64,
        confidence: f64,
        source_layer: Option<i32>,
        max_steps: Option<i32>,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        if let Some(ms) = max_steps {
            conn.execute(
                "INSERT INTO obligations (id, attempt_id, branch_id, parent_node_id,
                 description, obligation_type, priority, confidence, source_layer,
                 status, max_steps, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'open',?10,?11)",
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
                    ms,
                    now,
                ],
            )?;
        } else {
            conn.execute(
                "INSERT INTO obligations (id, attempt_id, branch_id, parent_node_id,
                 description, obligation_type, priority, confidence, source_layer,
                 status, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'open',?10)",
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
                    now,
                ],
            )?;
        }
        Ok(id)
    }

    pub fn get_obligation(&self, id: &str) -> Result<Obligation, DbError> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, attempt_id, branch_id, parent_node_id, description,
                    obligation_type, priority, confidence, source_layer, status,
                    assigned_model, closure_node_id, closure_type, escalation_level,
                    steps_spent, max_steps, search_space, superseded_by, retraction_reason,
                    depends_on, decomposition_id, satisfaction_criteria,
                    signature_json, embedding_json, scout_status,
                    last_scout_session_id, last_scout_confidence,
                    resolved_externally, resolved_by_corpus_id,
                    external_reference, scout_last_checked_at,
                    assigned_models_json, active_solver_round_id,
                    created_at, closed_at
             FROM obligations WHERE id = ?1",
            [id],
            |row| Ok(Self::row_to_obligation(row)),
        )
        .map_err(|_| DbError::NotFound(format!("obligation {}", id)))
    }

    pub fn get_open_obligations(&self, attempt_id: &str) -> Result<Vec<Obligation>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, parent_node_id, description,
                    obligation_type, priority, confidence, source_layer, status,
                    assigned_model, closure_node_id, closure_type, escalation_level,
                    steps_spent, max_steps, search_space, superseded_by, retraction_reason,
                    depends_on, decomposition_id, satisfaction_criteria,
                    signature_json, embedding_json, scout_status,
                    last_scout_session_id, last_scout_confidence,
                    resolved_externally, resolved_by_corpus_id,
                    external_reference, scout_last_checked_at,
                    assigned_models_json, active_solver_round_id,
                    created_at, closed_at
             FROM obligations WHERE attempt_id = ?1 AND status IN ('open', 'assigned')
             ORDER BY priority DESC",
        )?;
        let obs = stmt
            .query_map([attempt_id], |row| Ok(Self::row_to_obligation(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(obs)
    }

    pub fn get_all_obligations(&self, attempt_id: &str) -> Result<Vec<Obligation>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, parent_node_id, description,
                    obligation_type, priority, confidence, source_layer, status,
                    assigned_model, closure_node_id, closure_type, escalation_level,
                    steps_spent, max_steps, search_space, superseded_by, retraction_reason,
                    depends_on, decomposition_id, satisfaction_criteria,
                    signature_json, embedding_json, scout_status,
                    last_scout_session_id, last_scout_confidence,
                    resolved_externally, resolved_by_corpus_id,
                    external_reference, scout_last_checked_at,
                    assigned_models_json, active_solver_round_id,
                    created_at, closed_at
             FROM obligations WHERE attempt_id = ?1
             ORDER BY priority DESC",
        )?;
        let obs = stmt
            .query_map([attempt_id], |row| Ok(Self::row_to_obligation(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(obs)
    }

    pub fn count_open_obligations(&self, attempt_id: &str) -> Result<u32, DbError> {
        let conn = self.conn();
        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM obligations WHERE attempt_id = ?1 AND status IN ('open', 'assigned')",
            [attempt_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Get open obligations for a specific branch within an attempt.
    pub fn get_branch_open_obligations(
        &self,
        attempt_id: &str,
        branch_id: i32,
    ) -> Result<Vec<Obligation>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, parent_node_id, description,
                    obligation_type, priority, confidence, source_layer, status,
                    assigned_model, closure_node_id, closure_type, escalation_level,
                    steps_spent, max_steps, search_space, superseded_by, retraction_reason,
                    depends_on, decomposition_id, satisfaction_criteria,
                    signature_json, embedding_json, scout_status,
                    last_scout_session_id, last_scout_confidence,
                    resolved_externally, resolved_by_corpus_id,
                    external_reference, scout_last_checked_at,
                    assigned_models_json, active_solver_round_id,
                    created_at, closed_at
             FROM obligations WHERE attempt_id = ?1 AND branch_id = ?2 AND status IN ('open', 'assigned')
             ORDER BY priority DESC"
        )?;
        let obs = stmt
            .query_map(rusqlite::params![attempt_id, branch_id], |row| {
                Ok(Self::row_to_obligation(row))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(obs)
    }

    /// Count open obligations for a specific branch.
    pub fn count_branch_open_obligations(
        &self,
        attempt_id: &str,
        branch_id: i32,
    ) -> Result<u32, DbError> {
        let conn = self.conn();
        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM obligations WHERE attempt_id = ?1 AND branch_id = ?2 AND status IN ('open', 'assigned')",
            rusqlite::params![attempt_id, branch_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Assign an obligation to multiple solver workers for collaborative fan-in solving.
    pub fn assign_obligation_collaborative(
        &self,
        id: &str,
        primary_model: &str,
        all_models_json: &str,
        solver_round_id: &str,
    ) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE obligations SET status = 'assigned', assigned_model = ?1,
             assigned_models_json = ?2, active_solver_round_id = ?3 WHERE id = ?4",
            rusqlite::params![primary_model, all_models_json, solver_round_id, id],
        )?;
        Ok(())
    }

    /// Clear the active solver round after fan-in completes.
    pub fn clear_obligation_active_round(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE obligations SET active_solver_round_id = NULL, assigned_models_json = NULL WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    pub fn assign_obligation(&self, id: &str, model: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE obligations SET status = 'assigned', assigned_model = ?1 WHERE id = ?2",
            rusqlite::params![model, id],
        )?;
        Ok(())
    }

    pub fn unassign_obligations_except(
        &self,
        attempt_id: &str,
        keep_ids: &std::collections::HashSet<String>,
    ) -> Result<(), DbError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT id FROM obligations WHERE attempt_id = ?1 AND status = 'assigned'")?;
        let assigned_ids: Vec<String> = stmt
            .query_map([attempt_id], |row| row.get(0))?
            .filter_map(|row| row.ok())
            .collect();

        for id in assigned_ids {
            if keep_ids.contains(&id) {
                continue;
            }
            conn.execute(
                "UPDATE obligations
                 SET status = 'open',
                     assigned_model = NULL,
                     assigned_models_json = NULL,
                     active_solver_round_id = NULL
                 WHERE id = ?1",
                [id],
            )?;
        }
        Ok(())
    }

    pub fn close_obligation(
        &self,
        id: &str,
        closure_node_id: &str,
        closure_type: &str,
        closure_note: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        let status = match closure_type {
            "refuted" => "closed_refuted",
            "invalidated_by_evidence" => "closed_invalidated",
            _ => "closed_proved",
        };
        conn.execute(
            "UPDATE obligations SET status = ?1, closure_node_id = ?2, closure_type = ?3, closure_note = ?4, closed_at = ?5 WHERE id = ?6",
            rusqlite::params![status, closure_node_id, closure_type, closure_note, now, id],
        )?;
        Ok(())
    }

    pub fn supersede_obligation(&self, id: &str, superseded_by: &str) -> Result<(), DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE obligations SET status = 'superseded', superseded_by = ?1, closed_at = ?2 WHERE id = ?3",
            rusqlite::params![superseded_by, now, id],
        )?;
        Ok(())
    }

    pub fn retract_obligation(&self, id: &str, reason: &str) -> Result<(), DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE obligations SET status = 'retracted', retraction_reason = ?1, closed_at = ?2 WHERE id = ?3",
            rusqlite::params![reason, now, id],
        )?;
        Ok(())
    }

    pub fn demote_obligation(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE obligations SET status = 'demoted', closed_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )?;
        Ok(())
    }

    pub fn increment_obligation_steps(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE obligations SET steps_spent = steps_spent + 1 WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    /// Increment steps_spent for ALL open obligations in an attempt,
    /// then demote any that have exceeded their max_steps budget.
    /// Returns the list of (id, description) pairs that were demoted.
    pub fn tick_and_expire_stale_obligations(
        &self,
        attempt_id: &str,
    ) -> Result<Vec<(String, String)>, DbError> {
        let conn = self.conn();
        // Increment all open obligations
        conn.execute(
            "UPDATE obligations SET steps_spent = steps_spent + 1
             WHERE attempt_id = ?1 AND status IN ('open', 'assigned')",
            [attempt_id],
        )?;
        // Find obligations that exceeded their budget
        let mut stmt = conn.prepare(
            "SELECT id, description FROM obligations
             WHERE attempt_id = ?1 AND status IN ('open', 'assigned')
             AND steps_spent >= max_steps",
        )?;
        let stale: Vec<(String, String)> = stmt
            .query_map([attempt_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        // Demote stale obligations
        let now = chrono::Utc::now().to_rfc3339();
        for (id, _) in &stale {
            conn.execute(
                "UPDATE obligations SET status = 'demoted', retraction_reason = 'stale: exceeded max_steps budget', closed_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            )?;
        }
        Ok(stale)
    }

    /// Like tick_and_expire_stale_obligations, but skips a specific obligation ID.
    /// Used to avoid double-counting the targeted obligation (which gets its own
    /// increment_obligation_steps call from the solver loop).
    pub fn tick_and_expire_stale_obligations_except(
        &self,
        attempt_id: &str,
        except_id: Option<&str>,
    ) -> Result<Vec<(String, String)>, DbError> {
        let conn = self.conn();
        // Increment all open obligations EXCEPT the targeted one
        // (targeted obligation gets per-step increments separately)
        if let Some(skip_id) = except_id {
            conn.execute(
                "UPDATE obligations SET steps_spent = steps_spent + 1
                 WHERE attempt_id = ?1 AND status IN ('open', 'assigned') AND id != ?2",
                rusqlite::params![attempt_id, skip_id],
            )?;
        } else {
            conn.execute(
                "UPDATE obligations SET steps_spent = steps_spent + 1
                 WHERE attempt_id = ?1 AND status IN ('open', 'assigned')",
                [attempt_id],
            )?;
        }
        // Find obligations that exceeded their budget.
        // IMPORTANT: Also exclude the targeted obligation from demotion —
        // the solver is actively working on it and it should not be demoted
        // mid-solve. It will be checked for staleness when it is no longer targeted.
        let stale: Vec<(String, String)> = if let Some(skip_id) = except_id {
            let mut stmt = conn.prepare(
                "SELECT id, description FROM obligations
                 WHERE attempt_id = ?1 AND status IN ('open', 'assigned')
                 AND steps_spent >= max_steps AND id != ?2",
            )?;
            let rows: Vec<(String, String)> = stmt
                .query_map(rusqlite::params![attempt_id, skip_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, description FROM obligations
                 WHERE attempt_id = ?1 AND status IN ('open', 'assigned')
                 AND steps_spent >= max_steps",
            )?;
            let rows: Vec<(String, String)> = stmt
                .query_map([attempt_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };
        // Demote stale obligations
        let now = chrono::Utc::now().to_rfc3339();
        for (id, _) in &stale {
            conn.execute(
                "UPDATE obligations SET status = 'demoted', retraction_reason = 'stale: exceeded max_steps budget', closed_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            )?;
        }
        Ok(stale)
    }

    pub fn escalate_obligation(&self, id: &str) -> Result<i32, DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE obligations SET escalation_level = escalation_level + 1, steps_spent = 0 WHERE id = ?1",
            [id],
        )?;
        let level: i32 = conn.query_row(
            "SELECT escalation_level FROM obligations WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(level)
    }

    fn row_to_obligation(row: &rusqlite::Row) -> Obligation {
        Obligation {
            id: row.get(0).unwrap_or_default(),
            attempt_id: row.get(1).unwrap_or_default(),
            branch_id: row.get(2).unwrap_or(0),
            parent_node_id: row.get(3).unwrap_or_default(),
            description: row.get(4).unwrap_or_default(),
            obligation_type: row.get(5).unwrap_or_default(),
            priority: row.get(6).unwrap_or(0.5),
            confidence: row.get(7).unwrap_or(0.7),
            source_layer: row.get(8).unwrap_or(None),
            status: row.get(9).unwrap_or_default(),
            assigned_model: row.get(10).unwrap_or(None),
            closure_node_id: row.get(11).unwrap_or(None),
            closure_type: row.get(12).unwrap_or(None),
            escalation_level: row.get(13).unwrap_or(0),
            steps_spent: row.get(14).unwrap_or(0),
            max_steps: row.get(15).unwrap_or(20),
            search_space: row.get(16).unwrap_or(None),
            superseded_by: row.get(17).unwrap_or(None),
            retraction_reason: row.get(18).unwrap_or(None),
            depends_on: row.get(19).unwrap_or(None),
            decomposition_id: row.get(20).unwrap_or(None),
            satisfaction_criteria: row.get(21).unwrap_or(None),
            signature_json: row.get(22).unwrap_or(None),
            embedding_json: row.get(23).unwrap_or(None),
            scout_status: row.get(24).unwrap_or(None),
            last_scout_session_id: row.get(25).unwrap_or(None),
            last_scout_confidence: row.get(26).unwrap_or(None),
            resolved_externally: row.get::<_, i32>(27).unwrap_or(0) != 0,
            resolved_by_corpus_id: row.get(28).unwrap_or(None),
            external_reference: row.get(29).unwrap_or(None),
            scout_last_checked_at: row.get(30).unwrap_or(None),
            assigned_models_json: row.get(31).unwrap_or(None),
            active_solver_round_id: row.get(32).unwrap_or(None),
            created_at: row.get(33).unwrap_or_default(),
            closed_at: row.get(34).unwrap_or(None),
        }
    }

    // ---- Scout metadata updates ----

    pub fn update_obligation_scout_status(
        &self,
        id: &str,
        scout_status: &str,
        last_scout_session_id: &str,
        last_scout_confidence: f64,
    ) -> Result<(), DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE obligations SET scout_status = ?1, last_scout_session_id = ?2,
             last_scout_confidence = ?3, scout_last_checked_at = ?4 WHERE id = ?5",
            rusqlite::params![
                scout_status,
                last_scout_session_id,
                last_scout_confidence,
                now,
                id
            ],
        )?;
        Ok(())
    }

    pub fn mark_obligation_externally_resolved(
        &self,
        id: &str,
        corpus_id: Option<&str>,
        external_reference: &str,
    ) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE obligations SET resolved_externally = 1,
             resolved_by_corpus_id = ?1, external_reference = ?2 WHERE id = ?3",
            rusqlite::params![corpus_id, external_reference, id],
        )?;
        Ok(())
    }
}
