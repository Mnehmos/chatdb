use super::{Database, DbError};
use crate::models::dag::ProofNode;

impl Database {
    pub fn create_node(
        &self,
        attempt_id: &str,
        branch_id: i32,
        node_type: &str,
        parent_ids: Option<&str>,
        content: &str,
        formal_content: Option<&str>,
        technique_class: Option<&str>,
        construction_family: Option<&str>,
        status: &str,
        validator_used: Option<&str>,
        validator_result: Option<&str>,
        model_id: Option<&str>,
        obligation_ref: Option<&str>,
        step_id: Option<&str>,
        token_cost: Option<u32>,
        sequence_number: u32,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let verified_at: Option<&str> = if status == "verified" {
            Some(&now)
        } else {
            None
        };

        conn.execute(
            "INSERT INTO proof_nodes (id, attempt_id, branch_id, node_type, parent_ids,
             content, formal_content, technique_class, construction_family, status,
             validator_used, validator_result, model_id, obligation_ref, step_id,
             token_cost, sequence_number, created_at, verified_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
            rusqlite::params![
                id,
                attempt_id,
                branch_id,
                node_type,
                parent_ids,
                content,
                formal_content,
                technique_class,
                construction_family,
                status,
                validator_used,
                validator_result,
                model_id,
                obligation_ref,
                step_id,
                token_cost,
                sequence_number,
                now,
                verified_at,
            ],
        )?;
        Ok(id)
    }

    pub fn get_node(&self, id: &str) -> Result<ProofNode, DbError> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, attempt_id, branch_id, node_type, parent_ids,
                    content, formal_content, technique_class, construction_family, status,
                    validator_used, validator_result, model_id, obligation_ref, opens_obligations,
                    step_id, token_cost, sequence_number, created_at, verified_at
             FROM proof_nodes WHERE id = ?1",
            [id],
            |row| Ok(Self::row_to_node(row)),
        )
        .map_err(|_| DbError::NotFound(format!("node {}", id)))
    }

    pub fn get_nodes_for_attempt(&self, attempt_id: &str) -> Result<Vec<ProofNode>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, node_type, parent_ids,
                    content, formal_content, technique_class, construction_family, status,
                    validator_used, validator_result, model_id, obligation_ref, opens_obligations,
                    step_id, token_cost, sequence_number, created_at, verified_at
             FROM proof_nodes WHERE attempt_id = ?1 ORDER BY sequence_number",
        )?;
        let nodes = stmt
            .query_map([attempt_id], |row| Ok(Self::row_to_node(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(nodes)
    }

    pub fn get_verified_nodes(&self, attempt_id: &str) -> Result<Vec<ProofNode>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, node_type, parent_ids,
                    content, formal_content, technique_class, construction_family, status,
                    validator_used, validator_result, model_id, obligation_ref, opens_obligations,
                    step_id, token_cost, sequence_number, created_at, verified_at
             FROM proof_nodes WHERE attempt_id = ?1 AND status = 'verified' ORDER BY sequence_number"
        )?;
        let nodes = stmt
            .query_map([attempt_id], |row| Ok(Self::row_to_node(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(nodes)
    }

    /// Look up a proof_node by its linked step_id (from the legacy steps table).
    /// Returns None if no node exists for this step_id.
    pub fn get_node_by_step_id(&self, step_id: &str) -> Result<Option<ProofNode>, DbError> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, attempt_id, branch_id, node_type, parent_ids,
                    content, formal_content, technique_class, construction_family, status,
                    validator_used, validator_result, model_id, obligation_ref, opens_obligations,
                    step_id, token_cost, sequence_number, created_at, verified_at
             FROM proof_nodes WHERE step_id = ?1",
            [step_id],
            |row| Ok(Self::row_to_node(row)),
        );
        match result {
            Ok(node) => Ok(Some(node)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }

    /// Get all proof_nodes that targeted a specific obligation.
    /// Returns both verified and rejected nodes, ordered by creation time.
    pub fn get_nodes_for_obligation(&self, obligation_id: &str) -> Result<Vec<ProofNode>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, attempt_id, branch_id, node_type, parent_ids,
                    content, formal_content, technique_class, construction_family, status,
                    validator_used, validator_result, model_id, obligation_ref, opens_obligations,
                    step_id, token_cost, sequence_number, created_at, verified_at
             FROM proof_nodes WHERE obligation_ref = ?1 ORDER BY sequence_number ASC",
        )?;
        let nodes = stmt
            .query_map([obligation_id], |row| Ok(Self::row_to_node(row)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(nodes)
    }

    pub fn update_node_status(&self, id: &str, status: &str) -> Result<(), DbError> {
        let conn = self.conn();
        let verified_at: Option<String> = if status == "verified" {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            None
        };
        conn.execute(
            "UPDATE proof_nodes SET status = ?1, verified_at = ?2 WHERE id = ?3",
            rusqlite::params![status, verified_at, id],
        )?;
        Ok(())
    }

    pub fn update_node_obligations(
        &self,
        id: &str,
        opens_obligations: &str,
    ) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "UPDATE proof_nodes SET opens_obligations = ?1 WHERE id = ?2",
            rusqlite::params![opens_obligations, id],
        )?;
        Ok(())
    }

    fn row_to_node(row: &rusqlite::Row) -> ProofNode {
        ProofNode {
            id: row.get(0).unwrap_or_default(),
            attempt_id: row.get(1).unwrap_or_default(),
            branch_id: row.get(2).unwrap_or(0),
            node_type: row.get(3).unwrap_or_default(),
            parent_ids: row.get(4).unwrap_or(None),
            content: row.get(5).unwrap_or_default(),
            formal_content: row.get(6).unwrap_or(None),
            technique_class: row.get(7).unwrap_or(None),
            construction_family: row.get(8).unwrap_or(None),
            status: row.get(9).unwrap_or_default(),
            validator_used: row.get(10).unwrap_or(None),
            validator_result: row.get(11).unwrap_or(None),
            model_id: row.get(12).unwrap_or(None),
            obligation_ref: row.get(13).unwrap_or(None),
            opens_obligations: row.get(14).unwrap_or(None),
            step_id: row.get(15).unwrap_or(None),
            token_cost: row.get(16).unwrap_or(None),
            sequence_number: row.get(17).unwrap_or(0),
            created_at: row.get(18).unwrap_or_default(),
            verified_at: row.get(19).unwrap_or(None),
        }
    }
}
