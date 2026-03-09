use super::{Database, DbError};
use serde::{Deserialize, Serialize};

/// A stored research API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchApiKey {
    pub id: String,
    pub service: String,
    pub key_value: String,
    pub label: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl Database {
    /// Upsert a research API key — insert or update if service already exists.
    pub fn set_research_api_key(
        &self,
        service: &str,
        key_value: &str,
        label: Option<&str>,
    ) -> Result<ResearchApiKey, DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();

        // Check if service already has a key
        let existing_id: Option<String> = conn
            .query_row(
                "SELECT id FROM research_api_keys WHERE service = ?1",
                rusqlite::params![service],
                |row| row.get(0),
            )
            .ok();

        let id = if let Some(eid) = existing_id {
            conn.execute(
                "UPDATE research_api_keys SET key_value = ?1, label = ?2, active = 1, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![key_value, label, now, eid],
            )?;
            eid
        } else {
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO research_api_keys (id, service, key_value, label, active, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
                rusqlite::params![new_id, service, key_value, label, now],
            )?;
            new_id
        };

        Ok(ResearchApiKey {
            id,
            service: service.to_string(),
            key_value: key_value.to_string(),
            label: label.map(|s| s.to_string()),
            active: true,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Get a research API key by service name.
    pub fn get_research_api_key(&self, service: &str) -> Result<Option<ResearchApiKey>, DbError> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, service, key_value, label, active, created_at, updated_at FROM research_api_keys WHERE service = ?1",
            rusqlite::params![service],
            |row| {
                Ok(ResearchApiKey {
                    id: row.get(0)?,
                    service: row.get(1)?,
                    key_value: row.get(2)?,
                    label: row.get(3)?,
                    active: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        );

        match result {
            Ok(key) => Ok(Some(key)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::from(e)),
        }
    }

    /// List all research API keys (masks key_value for display).
    pub fn list_research_api_keys(&self) -> Result<Vec<ResearchApiKey>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, service, key_value, label, active, created_at, updated_at FROM research_api_keys ORDER BY service",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ResearchApiKey {
                    id: row.get(0)?,
                    service: row.get(1)?,
                    key_value: row.get(2)?,
                    label: row.get(3)?,
                    active: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Delete a research API key.
    pub fn delete_research_api_key(&self, service: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM research_api_keys WHERE service = ?1",
            rusqlite::params![service],
        )?;
        Ok(())
    }

    /// Toggle a research API key active/inactive.
    pub fn toggle_research_api_key(&self, service: &str, active: bool) -> Result<(), DbError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "UPDATE research_api_keys SET active = ?1, updated_at = ?2 WHERE service = ?3",
            rusqlite::params![active as i32, now, service],
        )?;
        Ok(())
    }
}
