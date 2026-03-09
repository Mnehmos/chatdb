"""ChatDB MCP server — exposes ChatDB capabilities to Claude via Model Context Protocol.

The LLM operates ChatDB like a user: creating problems, configuring agents,
starting proof runs, monitoring progress, and iterating on strategy.
"""
from mcp.server.fastmcp import FastMCP

from .tools_problem import chatdb_create_problem, chatdb_list_problems, chatdb_get_problem
from .tools_profile import chatdb_list_profiles, chatdb_save_profile, chatdb_delete_profile
from .tools_loop import (
    chatdb_start_solve, chatdb_stop_solve, chatdb_pause_solve, chatdb_get_loop_status
)
from .tools_inspect import (
    chatdb_get_proof_chain, chatdb_get_all_steps, chatdb_get_obligations,
    chatdb_get_dag_events, chatdb_get_proof_nodes,
)
from .tools_diagnose import (
    chatdb_get_diagnostics, chatdb_get_system_health, chatdb_get_after_action_report,
    chatdb_get_training_stats, chatdb_analyze_failures,
)
from .tools_intervene import (
    chatdb_inject_obligation, chatdb_trigger_council, chatdb_query_registry,
)
from .tools_knowledge import chatdb_search_patterns, chatdb_validate_expression

chatdb_mcp = FastMCP(
    "chatdb",
    instructions=(
        "You are an intelligence orchestrator for ChatDB, an autonomous proof engine. "
        "Use these tools to: create math problems, configure multi-model agents, "
        "start/monitor proof solving, inject strategic obligations, and analyze failures. "
        "The LLM decides which models do math, when to intervene, and what to try next. "
        "The database is the single source of truth — all state persists to SQLite.\n\n"
        "Workflow: create_problem → configure agents → start_solve → poll loop_status + "
        "diagnostics → intervene if needed → analyze results → iterate."
    ),
)

# Register all tools
for _fn in [
    chatdb_create_problem, chatdb_list_problems, chatdb_get_problem,
    chatdb_list_profiles, chatdb_save_profile, chatdb_delete_profile,
    chatdb_start_solve, chatdb_stop_solve, chatdb_pause_solve, chatdb_get_loop_status,
    chatdb_get_proof_chain, chatdb_get_all_steps, chatdb_get_obligations,
    chatdb_get_dag_events, chatdb_get_proof_nodes,
    chatdb_get_diagnostics, chatdb_get_system_health, chatdb_get_after_action_report,
    chatdb_get_training_stats, chatdb_analyze_failures,
    chatdb_inject_obligation, chatdb_trigger_council, chatdb_query_registry,
    chatdb_search_patterns, chatdb_validate_expression,
]:
    chatdb_mcp.tool()(_fn)
