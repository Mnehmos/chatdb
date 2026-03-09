import pytest

from src.lean_repl import router as lean_router
from src.lean_repl.session import REPLResult


def make_request() -> "lean_router.LeanFormalizeRequest":
    return lean_router.LeanFormalizeRequest(
        problem_id="problem-1",
        problem_statement="Prove x^2 >= 0",
        problem_domain="algebra",
        problem_formal_statement="x**2 >= 0",
        attempt_id="attempt-1",
        final_answer="Therefore x^2 >= 0.",
        verified_chain=[
            lean_router.LeanFormalizeStep(
                step_number=1,
                proposal_type="lemma",
                natural="Squares are nonnegative.",
                formal="x**2 >= 0",
                model="solver-a",
                obligation_id="ob-1",
                obligation_desc="show the square is nonnegative",
                obligation_type="BOUND",
            ),
            lean_router.LeanFormalizeStep(
                step_number=2,
                proposal_type="observation",
                natural="Pure prose note",
                formal=None,
                model="solver-a",
            ),
            lean_router.LeanFormalizeStep(
                step_number=3,
                proposal_type="conclusion",
                natural="Thus the claim follows.",
                formal="x**2 >= 0",
                model="solver-a",
            ),
        ],
        obligations=[
            lean_router.LeanFormalizeObligation(
                id="ob-1",
                description="show the square is nonnegative",
                obligation_type="BOUND",
                status="closed_proved",
            ),
            lean_router.LeanFormalizeObligation(
                id="ob-2",
                description="unused open branch",
                obligation_type="CASE_CHECK",
                status="open",
            ),
        ],
    )


def test_extract_spine_prefers_closed_obligation_steps_and_conclusion():
    from src.lean_repl import formalize

    request = make_request()
    spine = formalize.extract_spine(request.verified_chain, request.obligations)

    assert [step.step_number for step in spine] == [1, 3]


def test_generate_candidate_sources_targets_problem_formal_statement():
    from src.lean_repl import formalize

    request = make_request()
    spine = formalize.extract_spine(request.verified_chain, request.obligations)
    candidates = formalize.generate_candidate_sources(request, spine)

    assert candidates
    assert "theorem chatdb_problem_1" in candidates[0].lean_source
    assert ": x^2 >= 0 := by" in candidates[0].lean_source
    assert "Squares are nonnegative." in candidates[0].lean_source


@pytest.mark.asyncio
async def test_lean_formalize_returns_first_successful_candidate(monkeypatch):
    class FakeRepl:
        def __init__(self):
            self.commands = []

        async def cmd(self, command, env=None):
            self.commands.append(command)
            return REPLResult(env=7, messages=[], sorries=[])

    fake_repl = FakeRepl()

    async def fake_ready():
        return fake_repl

    monkeypatch.setattr(lean_router, "_ensure_repl_ready", fake_ready)

    response = await lean_router.lean_formalize(make_request())

    assert response.success is True
    assert response.attempts == 1
    assert "theorem chatdb_problem_1" in response.lean_source
    assert fake_repl.commands
