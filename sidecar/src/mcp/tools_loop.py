"""MCP tools: Loop control — start, stop, pause, status.

All tools call the Axum control server embedded in the Tauri app on :9744.
Returns a clear error if Tauri is not running.
"""
import os
from typing import Optional
import httpx

_CONTROL_URL = os.environ.get("CHATDB_CONTROL_URL", "http://localhost:9744")

_TAURI_NOT_RUNNING = (
    "Tauri app is not running (could not reach :9744). "
    "Open the ChatDB desktop app, then retry."
)


def chatdb_start_solve(problem_id: str, config: Optional[dict] = None) -> dict:
    """Start autonomous proof solving for a problem.

    Returns attempt_id on success. Requires the ChatDB Tauri app to be running.
    Pass config dict (MultiAgentConfig shape) to override the default profile.
    """
    try:
        resp = httpx.post(
            f"{_CONTROL_URL}/loop/start",
            json={"problem_id": problem_id, "config": config},
            timeout=10.0,
        )
        resp.raise_for_status()
        return resp.json()
    except httpx.ConnectError:
        return {"error": _TAURI_NOT_RUNNING}
    except httpx.HTTPStatusError as e:
        return {"error": f"Loop start failed: {e.response.text}"}
    except Exception as e:
        return {"error": str(e)}


def chatdb_stop_solve() -> dict:
    """Stop the currently running proof loop."""
    try:
        resp = httpx.post(f"{_CONTROL_URL}/loop/stop", timeout=5.0)
        resp.raise_for_status()
        return resp.json()
    except httpx.ConnectError:
        return {"error": _TAURI_NOT_RUNNING}
    except Exception as e:
        return {"error": str(e)}


def chatdb_pause_solve() -> dict:
    """Pause the proof loop (can be resumed with chatdb_start_solve using continue)."""
    try:
        resp = httpx.post(f"{_CONTROL_URL}/loop/pause", timeout=5.0)
        resp.raise_for_status()
        return resp.json()
    except httpx.ConnectError:
        return {"error": _TAURI_NOT_RUNNING}
    except Exception as e:
        return {"error": str(e)}


def chatdb_get_loop_status() -> dict:
    """Check if the proof loop is running and the current step number.

    Returns: { running: bool, attempt_id: str|null, step_number: int }
    """
    try:
        resp = httpx.get(f"{_CONTROL_URL}/loop/status", timeout=3.0)
        resp.raise_for_status()
        return resp.json()
    except httpx.ConnectError:
        return {
            "running": False,
            "attempt_id": None,
            "step_number": 0,
            "note": "Tauri app not running",
        }
    except Exception as e:
        return {"error": str(e), "running": False}
