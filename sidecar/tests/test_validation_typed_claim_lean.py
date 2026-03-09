import pytest

from src.validation import router as validation_router


@pytest.mark.asyncio
async def test_typed_claims_request_lean_advisory_when_enabled(monkeypatch):
    async def fake_lean_validate_claim(claim, formal, formal_lean):
        assert claim["type"] == "inequality"
        assert formal == "n + 1 <= n + 2"
        assert formal_lean is None
        return validation_router.ValidatorResult(
            passed=True,
            message="Lean verified typed claim",
            raw_output="theorem check : n + 1 <= n + 2 := by omega",
            wall_time_ms=4,
        )

    monkeypatch.setattr(validation_router, "_lean_available", lambda: True)
    monkeypatch.setattr(
        validation_router,
        "_lean_validate_claim",
        fake_lean_validate_claim,
        raising=False,
    )

    response = await validation_router.validate_step(
        validation_router.ValidateStepRequest(
            proposal_type="lemma",
            proposal_natural="Since n + 1 is at most n + 2, the bound holds.",
            proposal_formal="n + 1 <= n + 2",
            goal_state="prove bound",
            run_lean=True,
            problem_domain="number_theory",
            proposal_claim={
                "type": "inequality",
                "lhs": "n + 1",
                "rhs": "n + 2",
                "relation": "<=",
            },
        )
    )

    assert response.all_passed is True
    assert response.sympy is not None
    assert response.sympy.passed is True
    assert response.lean is not None
    assert response.lean.passed is True
    assert "typed claim" in response.lean.message
