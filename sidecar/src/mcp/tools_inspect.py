"""MCP tools: Proof inspection — reads DAG state from SQLite."""
from typing import Optional
from .db import get_db, rows_to_list


def chatdb_get_proof_chain(attempt_id: str) -> list:
    """Get the verified step chain for an attempt (verified=1 only, ordered by step_number)."""
    conn = get_db()
    try:
        rows = conn.execute(
            "SELECT id, attempt_id, step_number, model, goal_state, proposal_type, "
            "proposal_natural, proposal_formal, proposal_reasoning, verified, "
            "sympy_passed, pint_passed, lean_passed, created_at "
            "FROM steps WHERE attempt_id = ? AND verified = 1 ORDER BY step_number",
            (attempt_id,),
        ).fetchall()
        result = []
        for r in rows:
            d = dict(r)
            d["verified"] = bool(d["verified"])
            d["sympy_passed"] = bool(d["sympy_passed"]) if d["sympy_passed"] is not None else None
            d["pint_passed"] = bool(d["pint_passed"]) if d["pint_passed"] is not None else None
            d["lean_passed"] = bool(d["lean_passed"]) if d["lean_passed"] is not None else None
            result.append(d)
        return result
    finally:
        conn.close()


def chatdb_get_all_steps(attempt_id: str) -> list:
    """Get all steps (verified + rejected) for an attempt with challenge/adversary data."""
    conn = get_db()
    try:
        rows = conn.execute(
            "SELECT id, attempt_id, parent_step_id, step_number, model, goal_state, "
            "proposal_type, proposal_natural, proposal_formal, proposal_reasoning, "
            "verified, rejection_reason, sympy_passed, pint_passed, lean_passed, "
            "challenge_model, challenge_flaw_found, challenge_attack, "
            "challenge_confidence, challenge_fatal, created_at "
            "FROM steps WHERE attempt_id = ? ORDER BY step_number",
            (attempt_id,),
        ).fetchall()
        result = []
        for r in rows:
            d = dict(r)
            d["verified"] = bool(d["verified"])
            for col in ("sympy_passed", "pint_passed", "lean_passed",
                        "challenge_flaw_found", "challenge_fatal"):
                if d[col] is not None:
                    d[col] = bool(d[col])
            result.append(d)
        return result
    finally:
        conn.close()


def chatdb_get_obligations(attempt_id: str, status: Optional[str] = None) -> list:
    """Get obligations for an attempt. Optional status filter (open, closed_proved, etc.)."""
    conn = get_db()
    try:
        if status:
            rows = conn.execute(
                "SELECT id, attempt_id, branch_id, parent_node_id, description, "
                "obligation_type, priority, confidence, source_layer, status, "
                "assigned_model, closure_node_id, closure_type, escalation_level, "
                "steps_spent, max_steps, search_space, superseded_by, retraction_reason, "
                "created_at, closed_at "
                "FROM obligations WHERE attempt_id = ? AND status = ? ORDER BY priority DESC",
                (attempt_id, status),
            ).fetchall()
        else:
            rows = conn.execute(
                "SELECT id, attempt_id, branch_id, parent_node_id, description, "
                "obligation_type, priority, confidence, source_layer, status, "
                "assigned_model, closure_node_id, closure_type, escalation_level, "
                "steps_spent, max_steps, search_space, superseded_by, retraction_reason, "
                "created_at, closed_at "
                "FROM obligations WHERE attempt_id = ? ORDER BY priority DESC",
                (attempt_id,),
            ).fetchall()
        return rows_to_list(rows)
    finally:
        conn.close()


def chatdb_get_dag_events(attempt_id: str, since_sequence: int = 0) -> list:
    """Get DAG event stream for an attempt since a sequence number.

    Use since_sequence=0 for all events. Pass the last sequence_number you saw
    to poll for new events.
    """
    conn = get_db()
    try:
        rows = conn.execute(
            "SELECT id, attempt_id, event_type, payload, agent_role, "
            "sequence_number, created_at "
            "FROM dag_events WHERE attempt_id = ? AND sequence_number > ? "
            "ORDER BY sequence_number",
            (attempt_id, since_sequence),
        ).fetchall()
        return rows_to_list(rows)
    finally:
        conn.close()


def chatdb_get_proof_nodes(attempt_id: str) -> list:
    """Get DAG proof nodes for an attempt with parent linkage."""
    conn = get_db()
    try:
        rows = conn.execute(
            "SELECT id, attempt_id, branch_id, node_type, parent_ids, content, "
            "formal_content, technique_class, construction_family, status, "
            "validator_used, validator_result, model_id, obligation_ref, "
            "opens_obligations, step_id, token_cost, sequence_number, "
            "created_at, verified_at "
            "FROM proof_nodes WHERE attempt_id = ? ORDER BY sequence_number",
            (attempt_id,),
        ).fetchall()
        return rows_to_list(rows)
    finally:
        conn.close()
