"""MCP tools: Strategic intervention — inject obligations, trigger council, query registry."""
import os
from typing import Optional
import httpx
from .db import get_db, new_id, now_iso, rows_to_list

_SIDECAR_URL = os.environ.get("CHATDB_SIDECAR_URL", "http://localhost:9743")


def chatdb_inject_obligation(
    attempt_id: str,
    description: str,
    obligation_type: str = "construction",
    priority: float = 0.8,
    parent_node_id: Optional[str] = None,
) -> dict:
    """Inject a new obligation the solver must address.

    If parent_node_id is not provided, uses the most recent proof_node for the attempt.
    Returns the created obligation dict.
    """
    conn = get_db()
    try:
        if parent_node_id is None:
            row = conn.execute(
                "SELECT id FROM proof_nodes WHERE attempt_id = ? "
                "ORDER BY sequence_number DESC LIMIT 1",
                (attempt_id,),
            ).fetchone()
            if row is None:
                raise ValueError(
                    f"No proof nodes found for attempt {attempt_id!r}. "
                    "Cannot inject obligation without a parent_node_id."
                )
            parent_node_id = row["id"]

        oid = new_id()
        ts = now_iso()
        conn.execute(
            "INSERT INTO obligations (id, attempt_id, branch_id, parent_node_id, "
            "description, obligation_type, priority, confidence, status, created_at) "
            "VALUES (?, ?, 0, ?, ?, ?, ?, 0.7, 'open', ?)",
            (oid, attempt_id, parent_node_id, description, obligation_type, priority, ts),
        )
        conn.commit()
        return {
            "id": oid,
            "attempt_id": attempt_id,
            "parent_node_id": parent_node_id,
            "description": description,
            "obligation_type": obligation_type,
            "priority": priority,
            "status": "open",
            "created_at": ts,
        }
    finally:
        conn.close()


def chatdb_trigger_council(
    problem_id: str,
    attempt_id: str,
    council_models: Optional[list] = None,
    trigger_type: str = "mcp_requested",
) -> dict:
    """Force a multi-model council deliberation.

    Calls the sidecar /agents/council/deliberate endpoint with the current
    problem steps, then records the session and findings to SQLite.
    """
    if council_models is None:
        council_models = ["claude-sonnet-4-6"]

    conn = get_db()
    try:
        steps_rows = conn.execute(
            "SELECT id, step_number, model, goal_state, proposal_natural, "
            "proposal_formal, verified, rejection_reason "
            "FROM steps WHERE attempt_id = ? ORDER BY step_number",
            (attempt_id,),
        ).fetchall()
        steps = [dict(r) for r in steps_rows]
    finally:
        conn.close()

    try:
        resp = httpx.post(
            f"{_SIDECAR_URL}/agents/council/deliberate",
            json={
                "trigger_type": trigger_type,
                "problem_id": problem_id,
                "attempt_id": attempt_id,
                "steps": steps,
                "council_models": council_models,
            },
            timeout=60.0,
        )
        resp.raise_for_status()
        return resp.json()
    except httpx.ConnectError:
        return {
            "error": "Sidecar not reachable at :9743. Ensure the sidecar is running.",
            "council_models": council_models,
        }
    except Exception as e:
        return {"error": str(e), "council_models": council_models}


def chatdb_query_registry(problem_class: str) -> list:
    """Search the technique registry for approaches in a problem class."""
    conn = get_db()
    try:
        rows = conn.execute(
            "SELECT id, problem_class, technique_family, description, source, "
            "success_count, failure_count, last_used_at, created_at "
            "FROM technique_registry WHERE problem_class LIKE ? "
            "ORDER BY success_count DESC LIMIT 20",
            (f"%{problem_class}%",),
        ).fetchall()
        return rows_to_list(rows)
    finally:
        conn.close()
