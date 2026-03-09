"""Discerner: failure classification logic.

classify_failures(events) is a pure function over a list of dag_event dicts.
It classifies failures as model errors vs mechanical errors and returns
actionable recommendations.

Model errors: bad math, wrong reasoning, hallucination, incorrect proof step.
Mechanical errors: API timeouts, parse failures, sidecar unreachable, Lean warmup.
"""
import json
from typing import Any


def _parse_payload(payload: Any) -> dict:
    """Parse payload from either a dict (in-memory) or JSON string (from SQLite)."""
    if isinstance(payload, str):
        try:
            return json.loads(payload)
        except (json.JSONDecodeError, TypeError):
            return {}
    if isinstance(payload, dict):
        return payload
    return {}


def classify_failures(events: list[dict]) -> dict:
    """Classify diagnostic events into model vs mechanical failure categories.

    Args:
        events: List of dag_event dicts. Each has event_type, payload,
                sequence_number. Payload may be a dict or JSON string.

    Returns:
        {
            model_errors: list of event dicts for model-category errors,
            mechanical_errors: list of event dicts for mechanical-category errors,
            model_count: int,
            mechanical_count: int,
            gate_count: int,
            dominant_type: "model" | "mechanical" | "none",
            recommendations: list[str],
        }
    """
    model_errors: list[dict] = []
    mechanical_errors: list[dict] = []
    gate_count = 0

    for event in events:
        event_type = event.get("event_type", "")
        payload = _parse_payload(event.get("payload", {}))

        # Only process diagnostic events
        if "diagnostic" not in event_type and payload.get("category") is None:
            continue

        category = payload.get("category", "")
        severity = payload.get("severity", "error")
        message = payload.get("message", "")

        # Skip info-severity events — not errors
        if severity == "info":
            continue

        enriched = {
            "category": category,
            "severity": severity,
            "message": message,
            "source": payload.get("source", ""),
            "step_number": payload.get("step_number"),
            "sequence_number": event.get("sequence_number"),
        }

        if category == "model":
            model_errors.append(enriched)
        elif category == "mechanical":
            mechanical_errors.append(enriched)
        elif category == "gate":
            gate_count += 1

    model_count = len(model_errors)
    mechanical_count = len(mechanical_errors)

    if model_count == 0 and mechanical_count == 0:
        dominant_type = "none"
    elif mechanical_count > model_count:
        dominant_type = "mechanical"
    elif model_count > mechanical_count:
        dominant_type = "model"
    else:
        dominant_type = "model"  # tie goes to model (the harder problem to fix)

    recommendations = _build_recommendations(dominant_type, model_errors, mechanical_errors)

    return {
        "model_errors": model_errors,
        "mechanical_errors": mechanical_errors,
        "model_count": model_count,
        "mechanical_count": mechanical_count,
        "gate_count": gate_count,
        "dominant_type": dominant_type,
        "recommendations": recommendations,
    }


def _build_recommendations(
    dominant_type: str,
    model_errors: list[dict],
    mechanical_errors: list[dict],
) -> list[str]:
    if dominant_type == "none":
        return []

    recs: list[str] = []

    if dominant_type == "model":
        recs.append(
            "Model errors dominate. Consider switching the solver model or lowering temperature."
        )
        # Check for recurring patterns
        messages = [e["message"].lower() for e in model_errors]
        if any("hallucination" in m or "incorrect" in m or "wrong" in m for m in messages):
            recs.append(
                "Multiple incorrect reasoning steps detected. Try injecting an obligation "
                "to constrain the approach, or trigger a council deliberation."
            )
        if any("formal" in m or "parse" in m for m in messages):
            recs.append(
                "Formal expression failures detected. Verify the solver's formal output "
                "format matches the validator expectations."
            )
        recs.append("Run chatdb_trigger_council() to get multi-model diagnostic advice.")

    elif dominant_type == "mechanical":
        recs.append(
            "Mechanical errors dominate. Check infrastructure before retrying."
        )
        messages = [e["message"].lower() for e in mechanical_errors]
        if any("timeout" in m for m in messages):
            recs.append(
                "API timeouts detected. Check network connectivity and API key rate limits."
            )
        if any("sidecar" in m or "connection" in m or "unreachable" in m for m in messages):
            recs.append(
                "Sidecar unreachable. Run chatdb_get_system_health() and ensure the sidecar "
                "is running on :9743."
            )
        if any("lean" in m or "warmup" in m for m in messages):
            recs.append(
                "Lean warmup not complete. Wait 60-150s after sidecar start, or disable "
                "Lean validation temporarily."
            )
        if any("parse" in m or "json" in m for m in messages):
            recs.append(
                "JSON parse failures detected. This is usually transient — retry the solve."
            )

    return recs
