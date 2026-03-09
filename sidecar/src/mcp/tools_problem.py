"""MCP tools: Problem lifecycle."""
from typing import Optional
from .db import get_db, new_id, now_iso, rows_to_list


def chatdb_create_problem(statement: str, domain: str) -> dict:
    """Create a new math problem to solve. Deduplicates on identical statement."""
    conn = get_db()
    try:
        # Dedup: return existing if statement matches
        row = conn.execute(
            "SELECT id, statement, formal_statement, domain, source, status, "
            "created_at, solved_at, total_attempts, total_steps, known_answer "
            "FROM problems WHERE statement = ? LIMIT 1",
            (statement,),
        ).fetchone()
        if row:
            return dict(row)

        pid = new_id()
        ts = now_iso()
        conn.execute(
            "INSERT INTO problems (id, statement, domain, source, status, created_at) "
            "VALUES (?, ?, ?, 'mcp', 'open', ?)",
            (pid, statement, domain, ts),
        )
        conn.commit()
        return {
            "id": pid, "statement": statement, "formal_statement": None,
            "domain": domain, "source": "mcp", "status": "open",
            "created_at": ts, "solved_at": None,
            "total_attempts": 0, "total_steps": 0, "known_answer": None,
        }
    finally:
        conn.close()


def chatdb_list_problems(status: Optional[str] = None) -> list:
    """List all problems. Optional status filter: open, solved, failed."""
    conn = get_db()
    try:
        if status:
            rows = conn.execute(
                "SELECT id, statement, formal_statement, domain, source, status, "
                "created_at, solved_at, total_attempts, total_steps, known_answer "
                "FROM problems WHERE status = ? ORDER BY created_at DESC",
                (status,),
            ).fetchall()
        else:
            rows = conn.execute(
                "SELECT id, statement, formal_statement, domain, source, status, "
                "created_at, solved_at, total_attempts, total_steps, known_answer "
                "FROM problems ORDER BY created_at DESC",
            ).fetchall()
        return rows_to_list(rows)
    finally:
        conn.close()


def chatdb_get_problem(problem_id: str) -> dict:
    """Get problem details and attempt stats. Raises ValueError if not found."""
    conn = get_db()
    try:
        row = conn.execute(
            "SELECT id, statement, formal_statement, domain, source, status, "
            "created_at, solved_at, total_attempts, total_steps, known_answer "
            "FROM problems WHERE id = ?",
            (problem_id,),
        ).fetchone()
        if row is None:
            raise ValueError(f"Problem {problem_id!r} not found")
        return dict(row)
    finally:
        conn.close()
