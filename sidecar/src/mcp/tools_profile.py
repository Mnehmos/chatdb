"""MCP tools: Agent configuration profiles."""
import json
from typing import Optional
from .db import get_db, new_id, now_iso, rows_to_list


def chatdb_list_profiles() -> list:
    """List all saved agent configuration profiles."""
    conn = get_db()
    try:
        rows = conn.execute(
            "SELECT id, name, description, config_json, is_default, created_at, updated_at "
            "FROM agent_profiles ORDER BY name"
        ).fetchall()
        return rows_to_list(rows)
    finally:
        conn.close()


def chatdb_save_profile(name: str, description: Optional[str], config: dict) -> dict:
    """Save a named agent configuration profile (upsert by name).

    config must be a dict matching MultiAgentConfig shape. The Discerner role
    is supported via the optional discerner_model key.
    """
    config_json = json.dumps(config)
    conn = get_db()
    try:
        ts = now_iso()
        existing = conn.execute(
            "SELECT id FROM agent_profiles WHERE name = ?", (name,)
        ).fetchone()
        if existing:
            conn.execute(
                "UPDATE agent_profiles SET description = ?, config_json = ?, updated_at = ? "
                "WHERE name = ?",
                (description, config_json, ts, name),
            )
            profile_id = existing["id"]
        else:
            profile_id = new_id()
            conn.execute(
                "INSERT INTO agent_profiles (id, name, description, config_json, is_default, "
                "created_at, updated_at) VALUES (?, ?, ?, ?, 0, ?, ?)",
                (profile_id, name, description, config_json, ts, ts),
            )
        conn.commit()
        row = conn.execute(
            "SELECT id, name, description, config_json, is_default, created_at, updated_at "
            "FROM agent_profiles WHERE id = ?",
            (profile_id,),
        ).fetchone()
        return dict(row)
    finally:
        conn.close()


def chatdb_delete_profile(profile_id: str) -> dict:
    """Delete a profile by id. Returns confirmation."""
    conn = get_db()
    try:
        conn.execute("DELETE FROM agent_profiles WHERE id = ?", (profile_id,))
        conn.commit()
        return {"deleted": True, "id": profile_id}
    finally:
        conn.close()
