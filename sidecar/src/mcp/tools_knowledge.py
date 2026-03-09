"""MCP tools: Knowledge — pattern search and expression validation."""
import os
from typing import Optional
import httpx
from .db import get_db, rows_to_list

_SIDECAR_URL = os.environ.get("CHATDB_SIDECAR_URL", "http://localhost:9743")


def chatdb_search_patterns(query: str) -> list:
    """Find learned proof strategies matching a query string."""
    conn = get_db()
    try:
        rows = conn.execute(
            "SELECT id, name, description, domain, trigger_text, strategy, "
            "source_steps, success_count, failure_count, technique_class, "
            "deprecated, created_at, last_used_at "
            "FROM patterns WHERE deprecated = 0 "
            "AND (name LIKE ? OR trigger_text LIKE ?) "
            "ORDER BY success_count DESC LIMIT 20",
            (f"%{query}%", f"%{query}%"),
        ).fetchall()
        return rows_to_list(rows)
    finally:
        conn.close()


def chatdb_validate_expression(
    proposal_type: str,
    proposal_natural: str,
    proposal_formal: Optional[str],
    goal_state: str,
) -> dict:
    """Validate a formal expression against SymPy/Pint/Lean validators.

    Calls the sidecar /validate/step endpoint. Returns all_passed + per-validator results.
    """
    try:
        resp = httpx.post(
            f"{_SIDECAR_URL}/validate/step",
            json={
                "proposal_type": proposal_type,
                "proposal_natural": proposal_natural,
                "proposal_formal": proposal_formal,
                "goal_state": goal_state,
            },
            timeout=45.0,
        )
        resp.raise_for_status()
        return resp.json()
    except httpx.ConnectError:
        return {
            "error": "Sidecar not reachable at :9743. Ensure the sidecar is running.",
            "all_passed": False,
        }
    except Exception as e:
        return {"error": str(e), "all_passed": False}
