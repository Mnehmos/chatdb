"""Red-phase tests for MCP tools.

These tests verify the expected behavior of all MCP tool functions.
They import from src.mcp.* which does not exist yet — all tests fail on import.
Run: pytest tests/test_mcp_tools.py -v
"""
import json
import os
import sqlite3
import uuid
from datetime import datetime, timezone
from pathlib import Path

import pytest

# ---------------------------------------------------------------------------
# Schema fixture — mirrors src-tauri/src/db/schema.rs exactly
# ---------------------------------------------------------------------------

_SCHEMA = """
CREATE TABLE IF NOT EXISTS problems (
    id TEXT PRIMARY KEY, statement TEXT NOT NULL, formal_statement TEXT,
    domain TEXT, source TEXT, status TEXT DEFAULT 'open',
    created_at TEXT NOT NULL, solved_at TEXT,
    total_attempts INTEGER DEFAULT 0, total_steps INTEGER DEFAULT 0,
    known_answer TEXT
);
CREATE TABLE IF NOT EXISTS attempts (
    id TEXT PRIMARY KEY, problem_id TEXT NOT NULL REFERENCES problems(id),
    strategy TEXT, status TEXT DEFAULT 'active', started_at TEXT NOT NULL,
    ended_at TEXT, step_count INTEGER DEFAULT 0, backtrack_count INTEGER DEFAULT 0,
    cost_tokens_in INTEGER DEFAULT 0, cost_tokens_out INTEGER DEFAULT 0,
    models_used TEXT
);
CREATE TABLE IF NOT EXISTS steps (
    id TEXT PRIMARY KEY, attempt_id TEXT NOT NULL REFERENCES attempts(id),
    parent_step_id TEXT, step_number INTEGER NOT NULL, model TEXT NOT NULL,
    context_refs TEXT, goal_state TEXT NOT NULL, context_provided TEXT,
    proposal_type TEXT NOT NULL, proposal_natural TEXT NOT NULL,
    proposal_formal TEXT, proposal_reasoning TEXT,
    sympy_result TEXT, sympy_passed INTEGER, pint_result TEXT, pint_passed INTEGER,
    lean_result TEXT, lean_passed INTEGER, critic_prediction TEXT, critic_reasoning TEXT,
    verified INTEGER NOT NULL, rejection_reason TEXT,
    model_tokens_in INTEGER, model_tokens_out INTEGER, wall_time_ms INTEGER,
    created_at TEXT NOT NULL,
    step_type TEXT DEFAULT 'proof', branch_id INTEGER DEFAULT 0, technique_class TEXT,
    challenge_model TEXT, challenge_flaw_found INTEGER, challenge_attack TEXT,
    challenge_confidence REAL, challenge_fatal INTEGER
);
CREATE TABLE IF NOT EXISTS patterns (
    id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL,
    domain TEXT, trigger_text TEXT NOT NULL, strategy TEXT NOT NULL,
    source_steps TEXT NOT NULL, success_count INTEGER DEFAULT 1,
    failure_count INTEGER DEFAULT 0, avg_steps REAL, technique_class TEXT,
    deprecated INTEGER DEFAULT 0, deprecated_by TEXT,
    created_at TEXT NOT NULL, last_used_at TEXT
);
CREATE TABLE IF NOT EXISTS agent_profiles (
    id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, description TEXT,
    config_json TEXT NOT NULL, is_default INTEGER DEFAULT 0,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS proof_nodes (
    id TEXT PRIMARY KEY, attempt_id TEXT NOT NULL, branch_id INTEGER DEFAULT 0,
    node_type TEXT NOT NULL, parent_ids TEXT, content TEXT NOT NULL,
    formal_content TEXT, technique_class TEXT, construction_family TEXT,
    status TEXT DEFAULT 'proposed', validator_used TEXT, validator_result TEXT,
    model_id TEXT, obligation_ref TEXT, opens_obligations TEXT,
    step_id TEXT, token_cost INTEGER, sequence_number INTEGER NOT NULL,
    created_at TEXT NOT NULL, verified_at TEXT
);
CREATE TABLE IF NOT EXISTS obligations (
    id TEXT PRIMARY KEY, attempt_id TEXT NOT NULL, branch_id INTEGER DEFAULT 0,
    parent_node_id TEXT NOT NULL, description TEXT NOT NULL,
    obligation_type TEXT NOT NULL, priority REAL DEFAULT 0.5,
    confidence REAL DEFAULT 0.7, source_layer INTEGER,
    status TEXT DEFAULT 'open', assigned_model TEXT, closure_node_id TEXT,
    closure_type TEXT, escalation_level INTEGER DEFAULT 0,
    steps_spent INTEGER DEFAULT 0, max_steps INTEGER DEFAULT 20,
    search_space TEXT, superseded_by TEXT, retraction_reason TEXT,
    created_at TEXT NOT NULL, closed_at TEXT
);
CREATE TABLE IF NOT EXISTS dag_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT, attempt_id TEXT NOT NULL,
    event_type TEXT NOT NULL, payload TEXT NOT NULL, agent_role TEXT NOT NULL,
    sequence_number INTEGER NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS technique_registry (
    id INTEGER PRIMARY KEY AUTOINCREMENT, problem_class TEXT NOT NULL,
    technique_family TEXT NOT NULL, description TEXT NOT NULL,
    source TEXT DEFAULT 'seed', success_count INTEGER DEFAULT 0,
    failure_count INTEGER DEFAULT 0, last_used_at TEXT, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS orchestrator_decisions (
    id TEXT PRIMARY KEY, attempt_id TEXT, problem_id TEXT,
    decision_type TEXT NOT NULL, proof_state TEXT, worker_states TEXT,
    resource_state TEXT, decision TEXT NOT NULL, reasoning TEXT, outcome TEXT,
    steps_to_next_verify INTEGER, cost_after INTEGER, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS critic_evaluations (
    id TEXT PRIMARY KEY, step_id TEXT NOT NULL, attempt_id TEXT NOT NULL,
    prediction TEXT NOT NULL, confidence REAL, reasoning TEXT NOT NULL,
    actual_outcome INTEGER, prediction_correct INTEGER, cost_saved INTEGER,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS council_sessions (
    id TEXT PRIMARY KEY, trigger_type TEXT NOT NULL, problem_id TEXT NOT NULL,
    attempt_id TEXT, council_models TEXT NOT NULL, moderator_model TEXT,
    transcript TEXT NOT NULL, findings_count INTEGER DEFAULT 0, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS council_findings (
    id TEXT PRIMARY KEY, session_id TEXT NOT NULL, finding_type TEXT NOT NULL,
    summary TEXT NOT NULL, detail TEXT NOT NULL, step_refs TEXT,
    consensus TEXT NOT NULL, dissent TEXT, target_agent TEXT,
    acted_on INTEGER DEFAULT 0, impact_notes TEXT, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS scout_queries (
    id TEXT PRIMARY KEY, trigger_type TEXT NOT NULL, trigger_ref TEXT,
    query_text TEXT NOT NULL, sources_queried TEXT NOT NULL,
    results_count INTEGER DEFAULT 0, results_summary TEXT, techniques_found TEXT,
    injected_into TEXT, helped INTEGER, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS librarian_actions (
    id TEXT PRIMARY KEY, action_type TEXT NOT NULL, trigger_ref TEXT,
    pattern_id TEXT, reasoning TEXT NOT NULL, impact_measured INTEGER DEFAULT 0,
    solver_performance_delta REAL, created_at TEXT NOT NULL
);
"""


@pytest.fixture
def db_path(tmp_path: Path) -> str:
    """Create a temporary SQLite DB with full schema and return its path."""
    path = tmp_path / "test_chatdb.sqlite"
    conn = sqlite3.connect(str(path))
    conn.executescript(_SCHEMA)
    conn.commit()
    conn.close()
    return str(path)


@pytest.fixture(autouse=True)
def set_db_env(db_path: str, monkeypatch: pytest.MonkeyPatch):
    """Point all MCP tools at the test DB."""
    monkeypatch.setenv("CHATDB_DB_PATH", db_path)


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _uid() -> str:
    return str(uuid.uuid4())


def _seed_problem(db_path: str) -> str:
    """Insert a test problem and return its id."""
    pid = _uid()
    conn = sqlite3.connect(db_path)
    conn.execute(
        "INSERT INTO problems (id, statement, domain, source, status, created_at) "
        "VALUES (?, ?, ?, ?, 'open', ?)",
        (pid, "Prove that 2+2=4", "arithmetic", "test", _now()),
    )
    conn.commit()
    conn.close()
    return pid


def _seed_attempt(db_path: str, problem_id: str) -> str:
    aid = _uid()
    conn = sqlite3.connect(db_path)
    conn.execute(
        "INSERT INTO attempts (id, problem_id, status, started_at) VALUES (?, ?, 'active', ?)",
        (aid, problem_id, _now()),
    )
    conn.commit()
    conn.close()
    return aid


def _seed_step(db_path: str, attempt_id: str, step_number: int = 1, verified: int = 1) -> str:
    sid = _uid()
    conn = sqlite3.connect(db_path)
    conn.execute(
        "INSERT INTO steps (id, attempt_id, step_number, model, goal_state, "
        "proposal_type, proposal_natural, verified, created_at) "
        "VALUES (?, ?, ?, 'claude-test', 'prove x', 'lemma', 'x equals x', ?, ?)",
        (sid, attempt_id, step_number, verified, _now()),
    )
    conn.commit()
    conn.close()
    return sid


def _seed_node(db_path: str, attempt_id: str) -> str:
    nid = _uid()
    conn = sqlite3.connect(db_path)
    conn.execute(
        "INSERT INTO proof_nodes (id, attempt_id, node_type, content, sequence_number, created_at) "
        "VALUES (?, ?, 'lemma', 'test content', 1, ?)",
        (nid, attempt_id, _now()),
    )
    conn.commit()
    conn.close()
    return nid


# ---------------------------------------------------------------------------
# Problem lifecycle tests
# ---------------------------------------------------------------------------

class TestProblemTools:
    def test_create_problem_returns_dict_with_id(self):
        from src.mcp.tools_problem import chatdb_create_problem
        result = chatdb_create_problem("Prove P≠NP", "complexity")
        assert isinstance(result, dict)
        assert "id" in result
        assert result["statement"] == "Prove P≠NP"
        assert result["domain"] == "complexity"
        assert result["status"] == "open"

    def test_create_problem_deduplicates_on_same_statement(self):
        from src.mcp.tools_problem import chatdb_create_problem
        r1 = chatdb_create_problem("Prove X", "algebra")
        r2 = chatdb_create_problem("Prove X", "algebra")
        assert r1["id"] == r2["id"]

    def test_list_problems_returns_list(self, db_path):
        from src.mcp.tools_problem import chatdb_list_problems
        _seed_problem(db_path)
        result = chatdb_list_problems()
        assert isinstance(result, list)
        assert len(result) >= 1

    def test_list_problems_status_filter(self, db_path):
        from src.mcp.tools_problem import chatdb_list_problems, chatdb_create_problem
        chatdb_create_problem("Open problem", "algebra")
        open_results = chatdb_list_problems(status="open")
        solved_results = chatdb_list_problems(status="solved")
        assert len(open_results) >= 1
        assert all(r["status"] == "open" for r in open_results)
        assert all(r["status"] == "solved" for r in solved_results)

    def test_get_problem_returns_problem(self, db_path):
        from src.mcp.tools_problem import chatdb_get_problem
        pid = _seed_problem(db_path)
        result = chatdb_get_problem(pid)
        assert result["id"] == pid
        assert result["statement"] == "Prove that 2+2=4"

    def test_get_problem_raises_on_missing(self):
        from src.mcp.tools_problem import chatdb_get_problem
        with pytest.raises(Exception, match="not found|NotFound|no such"):
            chatdb_get_problem("nonexistent-id")


# ---------------------------------------------------------------------------
# Agent profile tests
# ---------------------------------------------------------------------------

_MINIMAL_CONFIG = {
    "models": [{"provider": "anthropic", "model": "claude-sonnet-4-6",
                "api_key_ref": "ANTHROPIC_API_KEY", "temperature": 0.3,
                "max_budget_tokens": 50000}],
    "max_total_cost": 100000,
    "failure_threshold": 5,
    "use_critic": True,
    "critic_skip_threshold": 0.8,
    "use_council": True,
    "council_models": [],
    "use_scout": False,
    "use_patterns": True,
    "allow_self_modify": False,
    "max_attempts": 5,
    "min_exploration_coverage": 0.6,
    "min_conclusion_confidence": 0.7,
}


class TestProfileTools:
    def test_save_profile_returns_profile(self):
        from src.mcp.tools_profile import chatdb_save_profile
        result = chatdb_save_profile("test-profile", "A test config", _MINIMAL_CONFIG)
        assert "id" in result
        assert result["name"] == "test-profile"

    def test_list_profiles_returns_list(self):
        from src.mcp.tools_profile import chatdb_save_profile, chatdb_list_profiles
        chatdb_save_profile("profile-a", None, _MINIMAL_CONFIG)
        result = chatdb_list_profiles()
        assert isinstance(result, list)
        assert any(p["name"] == "profile-a" for p in result)

    def test_delete_profile_removes_it(self):
        from src.mcp.tools_profile import chatdb_save_profile, chatdb_list_profiles, chatdb_delete_profile
        p = chatdb_save_profile("to-delete", None, _MINIMAL_CONFIG)
        chatdb_delete_profile(p["id"])
        remaining = chatdb_list_profiles()
        assert not any(r["id"] == p["id"] for r in remaining)

    def test_save_profile_upserts_by_name(self):
        from src.mcp.tools_profile import chatdb_save_profile, chatdb_list_profiles
        chatdb_save_profile("upsert-me", "v1", _MINIMAL_CONFIG)
        chatdb_save_profile("upsert-me", "v2", _MINIMAL_CONFIG)
        profiles = [p for p in chatdb_list_profiles() if p["name"] == "upsert-me"]
        assert len(profiles) == 1
        assert profiles[0]["description"] == "v2"


# ---------------------------------------------------------------------------
# Proof inspection tests
# ---------------------------------------------------------------------------

class TestInspectTools:
    def test_get_proof_chain_returns_verified_only(self, db_path):
        from src.mcp.tools_inspect import chatdb_get_proof_chain
        pid = _seed_problem(db_path)
        aid = _seed_attempt(db_path, pid)
        _seed_step(db_path, aid, step_number=1, verified=1)
        _seed_step(db_path, aid, step_number=2, verified=0)  # rejected
        chain = chatdb_get_proof_chain(aid)
        assert isinstance(chain, list)
        assert all(s["verified"] for s in chain)
        assert len(chain) == 1

    def test_get_all_steps_includes_rejected(self, db_path):
        from src.mcp.tools_inspect import chatdb_get_all_steps
        pid = _seed_problem(db_path)
        aid = _seed_attempt(db_path, pid)
        _seed_step(db_path, aid, step_number=1, verified=1)
        _seed_step(db_path, aid, step_number=2, verified=0)
        steps = chatdb_get_all_steps(aid)
        assert len(steps) == 2

    def test_get_obligations_returns_list(self, db_path):
        from src.mcp.tools_inspect import chatdb_get_obligations
        pid = _seed_problem(db_path)
        aid = _seed_attempt(db_path, pid)
        nid = _seed_node(db_path, aid)
        conn = sqlite3.connect(db_path)
        oid = _uid()
        conn.execute(
            "INSERT INTO obligations (id, attempt_id, parent_node_id, description, "
            "obligation_type, status, created_at) VALUES (?, ?, ?, 'test', 'construction', 'open', ?)",
            (oid, aid, nid, _now()),
        )
        conn.commit()
        conn.close()
        obs = chatdb_get_obligations(aid)
        assert isinstance(obs, list)
        assert len(obs) >= 1
        assert obs[0]["status"] == "open"

    def test_get_obligations_status_filter(self, db_path):
        from src.mcp.tools_inspect import chatdb_get_obligations
        pid = _seed_problem(db_path)
        aid = _seed_attempt(db_path, pid)
        nid = _seed_node(db_path, aid)
        conn = sqlite3.connect(db_path)
        conn.execute(
            "INSERT INTO obligations (id, attempt_id, parent_node_id, description, "
            "obligation_type, status, created_at) VALUES (?, ?, ?, 'open ob', 'construction', 'open', ?)",
            (_uid(), aid, nid, _now()),
        )
        conn.execute(
            "INSERT INTO obligations (id, attempt_id, parent_node_id, description, "
            "obligation_type, status, created_at) VALUES (?, ?, ?, 'closed ob', 'construction', 'closed_proved', ?)",
            (_uid(), aid, nid, _now()),
        )
        conn.commit()
        conn.close()
        open_obs = chatdb_get_obligations(aid, status="open")
        assert all(o["status"] == "open" for o in open_obs)

    def test_get_dag_events_respects_since(self, db_path):
        from src.mcp.tools_inspect import chatdb_get_dag_events
        pid = _seed_problem(db_path)
        aid = _seed_attempt(db_path, pid)
        conn = sqlite3.connect(db_path)
        for i in range(1, 4):
            conn.execute(
                "INSERT INTO dag_events (attempt_id, event_type, payload, agent_role, "
                "sequence_number, created_at) VALUES (?, 'step_verified', '{}', 'solver', ?, ?)",
                (aid, i, _now()),
            )
        conn.commit()
        conn.close()
        all_events = chatdb_get_dag_events(aid, since_sequence=0)
        assert len(all_events) == 3
        later = chatdb_get_dag_events(aid, since_sequence=2)
        assert len(later) == 1
        assert later[0]["sequence_number"] == 3

    def test_get_proof_nodes_returns_list(self, db_path):
        from src.mcp.tools_inspect import chatdb_get_proof_nodes
        pid = _seed_problem(db_path)
        aid = _seed_attempt(db_path, pid)
        _seed_node(db_path, aid)
        nodes = chatdb_get_proof_nodes(aid)
        assert isinstance(nodes, list)
        assert len(nodes) >= 1


# ---------------------------------------------------------------------------
# Diagnostics tests
# ---------------------------------------------------------------------------

class TestDiagnoseTools:
    def test_get_diagnostics_filters_to_diagnostic_events(self, db_path):
        from src.mcp.tools_diagnose import chatdb_get_diagnostics
        pid = _seed_problem(db_path)
        aid = _seed_attempt(db_path, pid)
        conn = sqlite3.connect(db_path)
        diagnostic_payload = json.dumps({"category": "model", "severity": "error",
                                          "message": "Wrong reasoning"})
        other_payload = json.dumps({"step": 1})
        conn.execute(
            "INSERT INTO dag_events (attempt_id, event_type, payload, agent_role, "
            "sequence_number, created_at) VALUES (?, 'loop:diagnostic', ?, 'solver', 1, ?)",
            (aid, diagnostic_payload, _now()),
        )
        conn.execute(
            "INSERT INTO dag_events (attempt_id, event_type, payload, agent_role, "
            "sequence_number, created_at) VALUES (?, 'step_verified', ?, 'solver', 2, ?)",
            (aid, other_payload, _now()),
        )
        conn.commit()
        conn.close()
        result = chatdb_get_diagnostics(aid, since_sequence=0)
        assert isinstance(result, list)
        assert len(result) == 1
        assert result[0]["category"] == "model"

    def test_get_training_stats_returns_counts(self, db_path):
        from src.mcp.tools_diagnose import chatdb_get_training_stats
        pid = _seed_problem(db_path)
        aid = _seed_attempt(db_path, pid)
        _seed_step(db_path, aid, 1, verified=1)
        _seed_step(db_path, aid, 2, verified=0)
        stats = chatdb_get_training_stats()
        assert stats["total_steps"] >= 2
        assert stats["verified_steps"] >= 1
        assert stats["rejected_steps"] >= 1

    def test_get_after_action_report_structure(self, db_path):
        from src.mcp.tools_diagnose import chatdb_get_after_action_report
        pid = _seed_problem(db_path)
        aid = _seed_attempt(db_path, pid)
        _seed_step(db_path, aid, 1, verified=1)
        _seed_step(db_path, aid, 2, verified=0)
        report = chatdb_get_after_action_report(pid)
        assert report["problem_id"] == pid
        assert "total_steps" in report
        assert "verified_steps" in report
        assert "accuracy_pct" in report
        assert "verified_chain" in report
        assert "failure_modes" in report


# ---------------------------------------------------------------------------
# Strategic intervention tests
# ---------------------------------------------------------------------------

class TestInterveneTools:
    def test_inject_obligation_inserts_to_db(self, db_path):
        from src.mcp.tools_intervene import chatdb_inject_obligation
        pid = _seed_problem(db_path)
        aid = _seed_attempt(db_path, pid)
        nid = _seed_node(db_path, aid)
        result = chatdb_inject_obligation(
            attempt_id=aid,
            description="Consider Fermat's Little Theorem",
            obligation_type="construction",
            priority=0.9,
            parent_node_id=nid,
        )
        assert "id" in result
        conn = sqlite3.connect(db_path)
        row = conn.execute("SELECT description, priority FROM obligations WHERE id = ?",
                           (result["id"],)).fetchone()
        conn.close()
        assert row is not None
        assert "Fermat" in row[0]
        assert abs(row[1] - 0.9) < 0.01

    def test_query_registry_returns_techniques(self, db_path):
        from src.mcp.tools_intervene import chatdb_query_registry
        conn = sqlite3.connect(db_path)
        conn.execute(
            "INSERT INTO technique_registry (problem_class, technique_family, description, created_at) "
            "VALUES ('number_theory', 'modular_arithmetic', 'Use mod properties', ?)",
            (_now(),),
        )
        conn.commit()
        conn.close()
        results = chatdb_query_registry("number_theory")
        assert isinstance(results, list)
        assert len(results) >= 1
        assert results[0]["technique_family"] == "modular_arithmetic"


# ---------------------------------------------------------------------------
# Knowledge tools tests
# ---------------------------------------------------------------------------

class TestKnowledgeTools:
    def test_search_patterns_returns_matching(self, db_path):
        from src.mcp.tools_knowledge import chatdb_search_patterns
        conn = sqlite3.connect(db_path)
        conn.execute(
            "INSERT INTO patterns (id, name, description, trigger_text, strategy, "
            "source_steps, created_at) VALUES (?, 'induction', 'Mathematical induction', "
            "'prove for all n', 'base + inductive step', '[]', ?)",
            (_uid(), _now()),
        )
        conn.commit()
        conn.close()
        results = chatdb_search_patterns("induction")
        assert isinstance(results, list)
        assert any("induction" in r["name"].lower() for r in results)

    def test_search_patterns_returns_empty_on_no_match(self):
        from src.mcp.tools_knowledge import chatdb_search_patterns
        results = chatdb_search_patterns("xyznonexistentpattern999")
        assert results == []
