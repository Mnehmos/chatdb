"""Tests for sidecar app bootstrap and health endpoint."""

from fastapi.testclient import TestClient

from src import main as sidecar_main


def test_health_reports_current_lean_status(monkeypatch):
    monkeypatch.setattr(sidecar_main, "_lean_available", lambda: False)
    monkeypatch.setattr(sidecar_main.lean_validator, "lean_warming_up", False)
    monkeypatch.setattr(sidecar_main.lean_validator, "lean_warm", False)
    monkeypatch.setattr(sidecar_main.lean_validator, "warmup_attempts", 2)

    with TestClient(sidecar_main.app) as client:
        response = client.get("/health")

    assert response.status_code == 200
    data = response.json()
    assert data["status"] == "ok"
    assert data["service"] == "chatdb-sidecar"
    assert data["lean_available"] is False
    assert data["lean_warming_up"] is False
    assert data["lean_ready"] is False
    assert data["lean_warmup_attempts"] == 2
    assert "lean_repl_state" in data


def test_app_mounts_core_route_families() -> None:
    paths = {route.path for route in sidecar_main.app.routes}

    assert "/health" in paths
    assert "/validate/step" in paths
    assert "/patterns/extract" in paths
    assert "/agents/council/deliberate" in paths
    assert "/research/search" in paths
    assert "/export/training_data/stats" in paths
    assert "/lean/formalize" in paths
