"""Red-phase tests for the Discerner failure classifier.

The Discerner is a pure function: given a list of diagnostic event dicts,
it classifies failures as model errors vs mechanical errors and produces
actionable recommendations.

Run: pytest tests/test_discerner.py -v
"""
import pytest

from src.mcp.discerner import classify_failures


def _diag(category: str, message: str, severity: str = "error",
           source: str = "solver") -> dict:
    """Build a synthetic diagnostic event payload dict."""
    return {
        "event_type": "loop:diagnostic",
        "payload": {
            "category": category,
            "severity": severity,
            "source": source,
            "message": message,
            "step_number": 1,
        },
        "sequence_number": 1,
    }


class TestClassifyFailures:
    def test_empty_events_returns_zero_counts(self):
        result = classify_failures([])
        assert result["model_count"] == 0
        assert result["mechanical_count"] == 0
        assert result["dominant_type"] == "none"
        assert isinstance(result["recommendations"], list)

    def test_model_errors_counted(self):
        events = [
            _diag("model", "Wrong mathematical step"),
            _diag("model", "Hallucination detected"),
            _diag("model", "Incorrect reasoning"),
        ]
        result = classify_failures(events)
        assert result["model_count"] == 3
        assert result["mechanical_count"] == 0
        assert result["dominant_type"] == "model"

    def test_mechanical_errors_counted(self):
        events = [
            _diag("mechanical", "API timeout after 30s"),
            _diag("mechanical", "Sidecar unreachable: connection refused"),
            _diag("mechanical", "JSON parse failure"),
        ]
        result = classify_failures(events)
        assert result["mechanical_count"] == 3
        assert result["model_count"] == 0
        assert result["dominant_type"] == "mechanical"

    def test_mixed_errors_dominant_determined_by_majority(self):
        events = [
            _diag("model", "Wrong step"),
            _diag("mechanical", "Timeout"),
            _diag("mechanical", "Timeout"),
            _diag("mechanical", "Parse failure"),
        ]
        result = classify_failures(events)
        assert result["dominant_type"] == "mechanical"
        assert result["mechanical_count"] == 3
        assert result["model_count"] == 1

    def test_model_errors_list_contains_event_data(self):
        events = [_diag("model", "Incorrect factorization")]
        result = classify_failures(events)
        assert len(result["model_errors"]) == 1
        assert "Incorrect factorization" in result["model_errors"][0]["message"]

    def test_mechanical_errors_list_contains_event_data(self):
        events = [_diag("mechanical", "Lean warmup not complete")]
        result = classify_failures(events)
        assert len(result["mechanical_errors"]) == 1

    def test_model_dominant_recommendation_mentions_model(self):
        events = [_diag("model", "Wrong step")] * 5
        result = classify_failures(events)
        recs = " ".join(result["recommendations"]).lower()
        assert any(kw in recs for kw in ("model", "temperature", "solver", "approach"))

    def test_mechanical_dominant_recommendation_mentions_infrastructure(self):
        events = [_diag("mechanical", "Timeout")] * 5
        result = classify_failures(events)
        recs = " ".join(result["recommendations"]).lower()
        assert any(kw in recs for kw in ("sidecar", "api", "timeout", "connection", "health"))

    def test_events_with_string_payload_parsed(self):
        """Events from SQLite have payload as JSON string, not dict."""
        import json
        events = [
            {
                "event_type": "loop:diagnostic",
                "payload": json.dumps({
                    "category": "model",
                    "severity": "error",
                    "message": "Serialized payload test",
                }),
                "sequence_number": 1,
            }
        ]
        result = classify_failures(events)
        assert result["model_count"] == 1

    def test_non_diagnostic_events_ignored(self):
        """step_verified and other non-diagnostic event types are skipped."""
        events = [
            {"event_type": "step_verified", "payload": {"step": 1}, "sequence_number": 1},
            {"event_type": "node_created", "payload": {"node_id": "abc"}, "sequence_number": 2},
        ]
        result = classify_failures(events)
        assert result["model_count"] == 0
        assert result["mechanical_count"] == 0

    def test_gate_category_neither_model_nor_mechanical(self):
        """Gate events (obligation not met) are their own category — not counted."""
        events = [_diag("gate", "Obligation not satisfied")]
        result = classify_failures(events)
        assert result["model_count"] == 0
        assert result["mechanical_count"] == 0
        assert "gate_count" in result or result["dominant_type"] == "none"

    def test_info_severity_not_counted_as_error(self):
        """Info-severity events should not inflate error counts."""
        events = [_diag("model", "Step attempt started", severity="info")]
        result = classify_failures(events)
        assert result["model_count"] == 0

    def test_returns_required_keys(self):
        result = classify_failures([])
        required = {"model_errors", "mechanical_errors", "model_count",
                    "mechanical_count", "dominant_type", "recommendations"}
        assert required.issubset(result.keys())
