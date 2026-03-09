"""MCP tools: Diagnostics and analysis."""
import json
import os
import sqlite3
from typing import Optional
import httpx
from .db import get_db, rows_to_list
from .discerner import classify_failures

_SIDECAR_URL = os.environ.get("CHATDB_SIDECAR_URL", "http://localhost:9743")
_CONTROL_URL = os.environ.get("CHATDB_CONTROL_URL", "http://localhost:9744")


def _safe_count(conn, table: str) -> int:
    try:
        return conn.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
    except sqlite3.OperationalError:
        return 0


def chatdb_get_diagnostics(attempt_id: str, since_sequence: int = 0) -> list:
    """Get diagnostic events for an attempt, classified by category.

    Returns only loop:diagnostic events (not step_verified, node_created, etc.).
    Each event includes parsed payload fields: category, severity, source, message.
    """
    conn = get_db()
    try:
        rows = conn.execute(
            "SELECT id, attempt_id, event_type, payload, agent_role, "
            "sequence_number, created_at "
            "FROM dag_events WHERE attempt_id = ? AND sequence_number > ? "
            "AND event_type LIKE '%diagnostic%' "
            "ORDER BY sequence_number",
            (attempt_id, since_sequence),
        ).fetchall()
    finally:
        conn.close()

    result = []
    for r in rows:
        d = dict(r)
        try:
            parsed = json.loads(d["payload"]) if isinstance(d["payload"], str) else d["payload"]
            d["category"] = parsed.get("category", "unknown")
            d["severity"] = parsed.get("severity", "info")
            d["source"] = parsed.get("source", "")
            d["message"] = parsed.get("message", "")
        except (json.JSONDecodeError, AttributeError):
            d["category"] = "unknown"
            d["severity"] = "info"
            d["source"] = ""
            d["message"] = ""
        result.append(d)
    return result


def chatdb_get_training_stats() -> dict:
    """Get aggregate training data statistics."""
    conn = get_db()
    try:
        total = conn.execute("SELECT COUNT(*) FROM steps").fetchone()[0]
        verified = conn.execute("SELECT COUNT(*) FROM steps WHERE verified = 1").fetchone()[0]
        rejected = conn.execute("SELECT COUNT(*) FROM steps WHERE verified = 0").fetchone()[0]
        council_sessions = conn.execute("SELECT COUNT(*) FROM council_sessions").fetchone()[0]
        council_findings = conn.execute("SELECT COUNT(*) FROM council_findings").fetchone()[0]
        critic_evals = conn.execute("SELECT COUNT(*) FROM critic_evaluations").fetchone()[0]
        scout_queries = conn.execute("SELECT COUNT(*) FROM scout_queries").fetchone()[0]
        librarian_actions = conn.execute("SELECT COUNT(*) FROM librarian_actions").fetchone()[0]
        orch_decisions = conn.execute("SELECT COUNT(*) FROM orchestrator_decisions").fetchone()[0]
        contrastive_pairs = _safe_count(conn, "contrastive_pairs")
        return {
            "total_steps": total,
            "verified_steps": verified,
            "rejected_steps": rejected,
            "contrastive_pairs": contrastive_pairs,
            "orchestrator_decisions": orch_decisions,
            "council_sessions": council_sessions,
            "council_findings": council_findings,
            "critic_evaluations": critic_evals,
            "scout_queries": scout_queries,
            "librarian_actions": librarian_actions,
        }
    finally:
        conn.close()


def chatdb_get_after_action_report(problem_id: str) -> dict:
    """Get post-attempt analysis for the latest attempt on a problem."""
    conn = get_db()
    try:
        problem = conn.execute(
            "SELECT statement, domain FROM problems WHERE id = ?", (problem_id,)
        ).fetchone()
        if not problem:
            raise ValueError(f"Problem {problem_id!r} not found")

        attempt = conn.execute(
            "SELECT id, started_at FROM attempts WHERE problem_id = ? "
            "ORDER BY started_at DESC LIMIT 1",
            (problem_id,),
        ).fetchone()
        if not attempt:
            raise ValueError(f"No attempts found for problem {problem_id!r}")
        attempt_id = attempt["id"]

        total = conn.execute(
            "SELECT COUNT(*) FROM steps WHERE attempt_id = ?", (attempt_id,)
        ).fetchone()[0]
        verified = conn.execute(
            "SELECT COUNT(*) FROM steps WHERE attempt_id = ? AND verified = 1", (attempt_id,)
        ).fetchone()[0]
        rejected = total - verified
        accuracy_pct = (verified / total * 100.0) if total > 0 else 0.0

        tokens_in = conn.execute(
            "SELECT COALESCE(SUM(model_tokens_in), 0) FROM steps WHERE attempt_id = ?",
            (attempt_id,),
        ).fetchone()[0]
        tokens_out = conn.execute(
            "SELECT COALESCE(SUM(model_tokens_out), 0) FROM steps WHERE attempt_id = ?",
            (attempt_id,),
        ).fetchone()[0]
        wall_ms = conn.execute(
            "SELECT COALESCE(SUM(wall_time_ms), 0) FROM steps WHERE attempt_id = ?",
            (attempt_id,),
        ).fetchone()[0]

        models_used = [
            r[0] for r in conn.execute(
                "SELECT DISTINCT model FROM steps WHERE attempt_id = ?", (attempt_id,)
            ).fetchall()
        ]

        chain_rows = conn.execute(
            "SELECT step_number, proposal_natural, proposal_formal, model "
            "FROM steps WHERE attempt_id = ? AND verified = 1 ORDER BY step_number",
            (attempt_id,),
        ).fetchall()
        verified_chain = [
            {"step_number": r[0], "natural": r[1], "formal": r[2], "model": r[3]}
            for r in chain_rows
        ]

        fail_rows = conn.execute(
            "SELECT COALESCE(rejection_reason, 'unknown'), COUNT(*) "
            "FROM steps WHERE attempt_id = ? AND verified = 0 "
            "GROUP BY rejection_reason ORDER BY COUNT(*) DESC",
            (attempt_id,),
        ).fetchall()
        failure_modes = [{"reason": r[0], "count": r[1]} for r in fail_rows]
        proof_complete = conn.execute(
            "SELECT COALESCE(status, 'unknown') FROM attempts WHERE id = ?",
            (attempt_id,),
        ).fetchone()[0] == "completed"
        open_obligations = conn.execute(
            "SELECT COUNT(*) FROM obligations WHERE attempt_id = ? AND status IN ('open', 'assigned')",
            (attempt_id,),
        ).fetchone()[0]
        final_answer_row = conn.execute(
            "SELECT proposal_natural FROM steps WHERE attempt_id = ? "
            "AND proposal_type = 'conclusion' AND verified = 1 "
            "ORDER BY step_number DESC LIMIT 1",
            (attempt_id,),
        ).fetchone()

        return {
            "problem_id": problem_id,
            "problem_statement": problem["statement"],
            "problem_domain": problem["domain"],
            "attempt_id": attempt_id,
            "total_steps": total,
            "verified_steps": verified,
            "rejected_steps": rejected,
            "accuracy_pct": accuracy_pct,
            "total_tokens_in": tokens_in,
            "total_tokens_out": tokens_out,
            "total_wall_ms": wall_ms,
            "models_used": models_used,
            "verified_chain": verified_chain,
            "failure_modes": failure_modes,
            "started_at": attempt["started_at"],
            "proof_complete": proof_complete,
            "open_obligations": open_obligations,
            "final_answer": final_answer_row[0] if final_answer_row else None,
        }
    finally:
        conn.close()


def chatdb_get_system_health() -> dict:
    """Get sidecar health (validators, Lean) and loop status from Tauri control plane."""
    sidecar_health = {}
    try:
        resp = httpx.get(f"{_SIDECAR_URL}/health", timeout=3.0)
        sidecar_health = resp.json()
        sidecar_reachable = True
    except Exception:
        sidecar_reachable = False

    loop_status = {}
    tauri_reachable = False
    try:
        resp = httpx.get(f"{_CONTROL_URL}/loop/status", timeout=2.0)
        loop_status = resp.json()
        tauri_reachable = True
    except Exception:
        pass

    return {
        "sidecar_reachable": sidecar_reachable,
        "lean_available": sidecar_health.get("lean_available", False),
        "lean_ready": sidecar_health.get("lean_ready", False),
        "lean_warming_up": sidecar_health.get("lean_warming_up", False),
        "lean_warmup_attempts": sidecar_health.get("lean_warmup_attempts", 0),
        "tauri_reachable": tauri_reachable,
        "loop_running": loop_status.get("running", False),
        "active_attempt": loop_status.get("attempt_id"),
        "current_step": loop_status.get("step_number"),
    }


def chatdb_analyze_failures(attempt_id: str) -> dict:
    """Discerner: classify failures as model errors vs mechanical errors.

    Reads diagnostic events for the attempt and returns:
    - model_errors / mechanical_errors lists
    - dominant_type: which category is causing the most failures
    - recommendations: actionable next steps for the orchestrator
    """
    conn = get_db()
    try:
        rows = conn.execute(
            "SELECT event_type, payload, sequence_number, created_at "
            "FROM dag_events WHERE attempt_id = ? ORDER BY sequence_number",
            (attempt_id,),
        ).fetchall()
    finally:
        conn.close()

    events = [dict(r) for r in rows]
    return classify_failures(events)
