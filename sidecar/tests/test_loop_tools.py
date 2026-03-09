"""Tests for MCP loop-control tools."""

import httpx

from src.mcp.tools_loop import (
    chatdb_get_loop_status,
    chatdb_pause_solve,
    chatdb_start_solve,
    chatdb_stop_solve,
)


def _json_response(method: str, url: str, payload: dict, status_code: int = 200) -> httpx.Response:
    request = httpx.Request(method, url)
    return httpx.Response(status_code, request=request, json=payload)


def _text_response(method: str, url: str, text: str, status_code: int) -> httpx.Response:
    request = httpx.Request(method, url)
    return httpx.Response(status_code, request=request, text=text)


def test_start_solve_posts_problem_id_and_config(monkeypatch):
    captured: dict[str, object] = {}

    def fake_post(url: str, json: dict, timeout: float) -> httpx.Response:
        captured["url"] = url
        captured["json"] = json
        captured["timeout"] = timeout
        return _json_response("POST", url, {"attempt_id": "attempt-123", "running": True})

    monkeypatch.setattr(httpx, "post", fake_post)

    result = chatdb_start_solve("problem-1", {"max_attempts": 2})

    assert result == {"attempt_id": "attempt-123", "running": True}
    assert captured["json"] == {"problem_id": "problem-1", "config": {"max_attempts": 2}}
    assert captured["timeout"] == 10.0
    assert str(captured["url"]).endswith("/loop/start")


def test_start_solve_returns_user_facing_http_error(monkeypatch):
    def fake_post(url: str, json: dict, timeout: float) -> httpx.Response:
        return _text_response("POST", url, "bad config", 400)

    monkeypatch.setattr(httpx, "post", fake_post)

    result = chatdb_start_solve("problem-1")

    assert result == {"error": "Loop start failed: bad config"}


def test_stop_solve_reports_tauri_not_running_on_connect_error(monkeypatch):
    def fake_post(url: str, timeout: float) -> httpx.Response:
        raise httpx.ConnectError("connection refused", request=httpx.Request("POST", url))

    monkeypatch.setattr(httpx, "post", fake_post)

    result = chatdb_stop_solve()

    assert "Tauri app is not running" in result["error"]


def test_pause_solve_returns_json_payload(monkeypatch):
    def fake_post(url: str, timeout: float) -> httpx.Response:
        return _json_response("POST", url, {"paused": True})

    monkeypatch.setattr(httpx, "post", fake_post)

    result = chatdb_pause_solve()

    assert result == {"paused": True}


def test_get_loop_status_returns_fallback_when_tauri_is_down(monkeypatch):
    def fake_get(url: str, timeout: float) -> httpx.Response:
        raise httpx.ConnectError("connection refused", request=httpx.Request("GET", url))

    monkeypatch.setattr(httpx, "get", fake_get)

    result = chatdb_get_loop_status()

    assert result["running"] is False
    assert result["attempt_id"] is None
    assert result["step_number"] == 0
    assert result["note"] == "Tauri app not running"


def test_get_loop_status_surfaces_unexpected_errors(monkeypatch):
    def fake_get(url: str, timeout: float) -> httpx.Response:
        raise RuntimeError("boom")

    monkeypatch.setattr(httpx, "get", fake_get)

    result = chatdb_get_loop_status()

    assert result == {"error": "boom", "running": False}
