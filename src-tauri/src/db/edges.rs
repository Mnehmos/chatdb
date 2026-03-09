use super::{Database, DbError};
use crate::models::management::DagEdge;

impl Database {
    pub fn create_edge(
        &self,
        source_id: &str,
        target_id: &str,
        source_type: &str,
        target_type: &str,
        edge_type: &str,
        metadata: Option<&str>,
    ) -> Result<i64, DbError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn();
        conn.execute(
            "INSERT OR IGNORE INTO dag_edges (source_id, target_id, source_type, target_type, edge_type, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![source_id, target_id, source_type, target_type, edge_type, metadata, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_edges_from(&self, source_id: &str) -> Result<Vec<DagEdge>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, source_type, target_type, edge_type, metadata, created_at
             FROM dag_edges WHERE source_id = ?1 ORDER BY created_at"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![source_id], Self::row_to_edge)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_edges_to(&self, target_id: &str) -> Result<Vec<DagEdge>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, source_type, target_type, edge_type, metadata, created_at
             FROM dag_edges WHERE target_id = ?1 ORDER BY created_at"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![target_id], Self::row_to_edge)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_edges_by_type(&self, edge_type: &str) -> Result<Vec<DagEdge>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, source_type, target_type, edge_type, metadata, created_at
             FROM dag_edges WHERE edge_type = ?1 ORDER BY created_at"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![edge_type], Self::row_to_edge)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_neighbors(&self, node_id: &str) -> Result<Vec<DagEdge>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, source_type, target_type, edge_type, metadata, created_at
             FROM dag_edges WHERE source_id = ?1 OR target_id = ?1 ORDER BY created_at"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![node_id], Self::row_to_edge)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_edge(&self, id: i64) -> Result<(), DbError> {
        let conn = self.conn();
        conn.execute("DELETE FROM dag_edges WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    fn row_to_edge(row: &rusqlite::Row) -> rusqlite::Result<DagEdge> {
        Ok(DagEdge {
            id: row.get(0)?,
            source_id: row.get(1)?,
            target_id: row.get(2)?,
            source_type: row.get(3)?,
            target_type: row.get(4)?,
            edge_type: row.get(5)?,
            metadata: row.get(6)?,
            created_at: row.get(7)?,
        })
    }
}
