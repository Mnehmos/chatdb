use super::{Database, DbError};
use crate::models::agents::AgentProfile;

impl Database {
    /// Save a new profile or update an existing one (upsert by name).
    pub fn save_profile(
        &self,
        name: &str,
        description: Option<&str>,
        config_json: &str,
        is_default: bool,
    ) -> Result<AgentProfile, DbError> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();

        // Check for existing profile with same name
        let existing_id: Option<String> = conn
            .query_row(
                "SELECT id FROM agent_profiles WHERE name = ?1",
                rusqlite::params![name],
                |row| row.get(0),
            )
            .ok();

        let id = if let Some(eid) = existing_id {
            conn.execute(
                "UPDATE agent_profiles SET description = ?1, config_json = ?2, is_default = ?3, updated_at = ?4 WHERE id = ?5",
                rusqlite::params![description, config_json, is_default as i32, now, eid],
            )?;
            eid
        } else {
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO agent_profiles (id, name, description, config_json, is_default, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![new_id, name, description, config_json, is_default as i32, now, now],
            )?;
            new_id
        };

        // If setting as default, clear other defaults
        if is_default {
            conn.execute(
                "UPDATE agent_profiles SET is_default = 0 WHERE id != ?1",
                rusqlite::params![id],
            )?;
        }

        drop(conn);
        self.get_profile(&id)
    }

    pub fn get_profile(&self, id: &str) -> Result<AgentProfile, DbError> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, name, description, config_json, is_default, created_at, updated_at FROM agent_profiles WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok(AgentProfile {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    config_json: row.get(3)?,
                    is_default: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .map_err(|_| DbError::NotFound(format!("Profile {}", id)))
    }

    pub fn list_profiles(&self) -> Result<Vec<AgentProfile>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, config_json, is_default, created_at, updated_at FROM agent_profiles ORDER BY is_default DESC, updated_at DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(AgentProfile {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    config_json: row.get(3)?,
                    is_default: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_default_profile(&self) -> Result<Option<AgentProfile>, DbError> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, name, description, config_json, is_default, created_at, updated_at FROM agent_profiles WHERE is_default = 1",
            [],
            |row| {
                Ok(AgentProfile {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    config_json: row.get(3)?,
                    is_default: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        );
        match result {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }

    pub fn delete_profile(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn();
        let affected = conn.execute(
            "DELETE FROM agent_profiles WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(format!("Profile {}", id)));
        }
        Ok(())
    }

    pub fn set_default_profile(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute("UPDATE agent_profiles SET is_default = 0", [])?;
        let affected = conn.execute(
            "UPDATE agent_profiles SET is_default = 1 WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(format!("Profile {}", id)));
        }
        Ok(())
    }
}
