use super::{Database, DbError};
use crate::models::tool_use::{CorpusUsage, ResolutionCorpusEntry};

impl Database {
    // ---- resolution_corpus ----

    pub fn register_corpus_entry(
        &self,
        canonical_statement: &str,
        canonical_hash: &str,
        obligation_type: &str,
        problem_domain: Option<&str>,
        signature_json: &str,
        embedding_json: Option<&str>,
        source_tier: i32,
        source_provider: &str,
        source_reference: &str,
        source_url: Option<&str>,
        source_formalism: &str,
        mapping_text: &str,
        translation_json: &str,
        mechanical_verification: bool,
        verification_method: Option<&str>,
        lean_proof_term: Option<&str>,
        tags_json: Option<&str>,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO resolution_corpus (
                id, canonical_statement, canonical_hash, obligation_type,
                problem_domain, signature_json, embedding_json,
                source_tier, source_provider, source_reference, source_url,
                source_formalism, mapping_text, translation_json,
                mechanical_verification, verification_method, lean_proof_term,
                tags_json, created_at, updated_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
            rusqlite::params![
                id,
                canonical_statement,
                canonical_hash,
                obligation_type,
                problem_domain,
                signature_json,
                embedding_json,
                source_tier,
                source_provider,
                source_reference,
                source_url,
                source_formalism,
                mapping_text,
                translation_json,
                mechanical_verification as i32,
                verification_method,
                lean_proof_term,
                tags_json,
                now,
                now,
            ],
        )?;
        Ok(id)
    }

    pub fn find_corpus_by_hash(
        &self,
        canonical_hash: &str,
    ) -> Result<Option<ResolutionCorpusEntry>, DbError> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, canonical_statement, canonical_hash, obligation_type,
                    problem_domain, signature_json, embedding_json,
                    source_tier, source_provider, source_reference, source_url,
                    source_formalism, mapping_text, translation_json,
                    mechanical_verification, verification_method, lean_proof_term,
                    tags_json, reuse_count, last_reused_at, active,
                    created_at, updated_at
             FROM resolution_corpus WHERE canonical_hash = ?1 AND active = 1 LIMIT 1",
            [canonical_hash],
            |row| Ok(Self::row_to_corpus_entry(row)),
        );
        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }

    pub fn increment_corpus_reuse(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE resolution_corpus SET reuse_count = reuse_count + 1,
             last_reused_at = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )?;
        Ok(())
    }

    pub fn get_top_corpus_entries(
        &self,
        limit: u32,
    ) -> Result<Vec<ResolutionCorpusEntry>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, canonical_statement, canonical_hash, obligation_type,
                    problem_domain, signature_json, embedding_json,
                    source_tier, source_provider, source_reference, source_url,
                    source_formalism, mapping_text, translation_json,
                    mechanical_verification, verification_method, lean_proof_term,
                    tags_json, reuse_count, last_reused_at, active,
                    created_at, updated_at
             FROM resolution_corpus WHERE active = 1
             ORDER BY reuse_count DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![limit], |row| {
                Ok(Self::row_to_corpus_entry(row))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn row_to_corpus_entry(row: &rusqlite::Row) -> ResolutionCorpusEntry {
        ResolutionCorpusEntry {
            id: row.get(0).unwrap_or_default(),
            canonical_statement: row.get(1).unwrap_or_default(),
            canonical_hash: row.get(2).unwrap_or_default(),
            obligation_type: row.get(3).unwrap_or_default(),
            problem_domain: row.get(4).unwrap_or(None),
            signature_json: row.get(5).unwrap_or_default(),
            embedding_json: row.get(6).unwrap_or(None),
            source_tier: row.get(7).unwrap_or(0),
            source_provider: row.get(8).unwrap_or_default(),
            source_reference: row.get(9).unwrap_or_default(),
            source_url: row.get(10).unwrap_or(None),
            source_formalism: row.get(11).unwrap_or_default(),
            mapping_text: row.get(12).unwrap_or_default(),
            translation_json: row.get(13).unwrap_or_default(),
            mechanical_verification: row.get::<_, i32>(14).unwrap_or(0) != 0,
            verification_method: row.get(15).unwrap_or(None),
            lean_proof_term: row.get(16).unwrap_or(None),
            tags_json: row.get(17).unwrap_or(None),
            reuse_count: row.get(18).unwrap_or(0),
            last_reused_at: row.get(19).unwrap_or(None),
            active: row.get::<_, i32>(20).unwrap_or(1) != 0,
            created_at: row.get(21).unwrap_or_default(),
            updated_at: row.get(22).unwrap_or_default(),
        }
    }

    // ---- corpus_usages ----

    pub fn record_corpus_usage(
        &self,
        corpus_entry_id: &str,
        attempt_id: &str,
        problem_id: &str,
        obligation_id: &str,
        scout_session_id: &str,
        resolution_id: Option<&str>,
    ) -> Result<String, DbError> {
        let conn = self.conn();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO corpus_usages (
                id, corpus_entry_id, attempt_id, problem_id,
                obligation_id, scout_session_id, resolution_id, created_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![
                id,
                corpus_entry_id,
                attempt_id,
                problem_id,
                obligation_id,
                scout_session_id,
                resolution_id,
                now,
            ],
        )?;
        Ok(id)
    }

    pub fn get_usages_for_corpus_entry(
        &self,
        corpus_entry_id: &str,
    ) -> Result<Vec<CorpusUsage>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, corpus_entry_id, attempt_id, problem_id,
                    obligation_id, scout_session_id, resolution_id, created_at
             FROM corpus_usages WHERE corpus_entry_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt
            .query_map([corpus_entry_id], |row| {
                Ok(CorpusUsage {
                    id: row.get(0).unwrap_or_default(),
                    corpus_entry_id: row.get(1).unwrap_or_default(),
                    attempt_id: row.get(2).unwrap_or_default(),
                    problem_id: row.get(3).unwrap_or_default(),
                    obligation_id: row.get(4).unwrap_or_default(),
                    scout_session_id: row.get(5).unwrap_or_default(),
                    resolution_id: row.get(6).unwrap_or(None),
                    created_at: row.get(7).unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }
}
