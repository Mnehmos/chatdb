"""Deterministic proof-to-Lean formalization helpers.

This is a first-pass pipeline: it extracts load-bearing verified steps,
generates standalone Lean theorem candidates, and tries a small tactic
family against the persistent REPL. It does not call an LLM.
"""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Iterable, Sequence

from src.validation.lean_validator import _extract_variables, _sympy_to_lean

from .session import _MATHLIB_IMPORTS


@dataclass(frozen=True)
class CandidateSource:
    lean_source: str
    compile_source: str
    tactic_label: str


def extract_spine(verified_chain: Sequence[object], obligations: Sequence[object]) -> list[object]:
    closed_obligations = {
        getattr(obligation, "id", None)
        for obligation in obligations
        if getattr(obligation, "status", "") not in {"open", "assigned"}
    }

    spine: list[object] = []
    seen_formals: set[str] = set()
    for step in verified_chain:
        formal = getattr(step, "formal", None)
        if not formal:
            continue

        is_conclusion = getattr(step, "proposal_type", "") == "conclusion"
        obligation_id = getattr(step, "obligation_id", None)
        is_load_bearing = is_conclusion or obligation_id in closed_obligations
        if not is_load_bearing:
            continue

        normalized = formal.strip()
        if normalized in seen_formals and not is_conclusion:
            continue

        seen_formals.add(normalized)
        spine.append(step)

    if spine:
        return spine

    for step in verified_chain:
        formal = getattr(step, "formal", None)
        if not formal:
            continue
        normalized = formal.strip()
        if normalized in seen_formals:
            continue
        seen_formals.add(normalized)
        spine.append(step)
    return spine


def generate_candidate_sources(request: object, spine: Sequence[object]) -> list[CandidateSource]:
    theorem_name = _theorem_name(getattr(request, "problem_id", "proof"))
    goal = _choose_goal(request, spine)
    if goal is None:
        return []

    translated_goal = _translate_formula(goal)
    if translated_goal is None:
        return []

    theorem_params = _theorem_params(translated_goal, spine)
    theorem_prefix = f"theorem {theorem_name}"
    if theorem_params:
        theorem_prefix = f"{theorem_prefix} {theorem_params}"
    theorem_header = f"{theorem_prefix} : {translated_goal} := by"

    spine_comments = [
        f"  -- {step.step_number}. {getattr(step, 'natural', '').strip()}"
        for step in spine
        if getattr(step, "natural", None)
    ]

    translated_spine = []
    for step in spine:
        formal = getattr(step, "formal", None)
        if not formal:
            continue
        translated = _translate_formula(formal)
        if translated is None:
            continue
        translated_spine.append((step.step_number, translated))

    candidates: list[CandidateSource] = []
    for tactic_label in _goal_tactic_sequences(translated_goal):
        body_lines = [theorem_header, *spine_comments]
        last_have_name: str | None = None
        for index, (_, translated) in enumerate(translated_spine, start=1):
            last_have_name = f"h{index}"
            body_lines.append(f"  have {last_have_name} : {translated} := by")
            body_lines.append(f"    {_first_tactic(_goal_tactic_sequences(translated)[0])}")

        if last_have_name and translated_spine[-1][1] == translated_goal:
            body_lines.append(f"  exact {last_have_name}")
        else:
            body_lines.append(f"  {_first_tactic(tactic_label)}")

        compile_source = "\n".join(body_lines) + "\n"
        lean_source = f"{_MATHLIB_IMPORTS}\n\n{compile_source}"
        candidates.append(
            CandidateSource(
                lean_source=lean_source,
                compile_source=compile_source,
                tactic_label=tactic_label,
            )
        )
    return candidates


async def run_formalization(request: object, repl: object) -> tuple[bool, str, list[str], int]:
    spine = extract_spine(
        getattr(request, "verified_chain", []),
        getattr(request, "obligations", []),
    )
    candidates = generate_candidate_sources(request, spine)
    if not candidates:
        return (
            False,
            _skeleton_only_source(request, spine),
            ["No formal goal could be extracted from the completed proof."],
            0,
        )

    errors: list[str] = []
    for attempts, candidate in enumerate(candidates, start=1):
        result = await repl.cmd(candidate.compile_source)
        candidate_errors = [
            str(message.get("data", message.get("message", "Lean error")))
            for message in result.messages
            if message.get("severity") == "error"
        ]
        if getattr(result, "sorries", None):
            candidate_errors.append("Lean reported sorry placeholders.")

        if not candidate_errors:
            return True, candidate.lean_source, [], attempts

        errors.append(f"{candidate.tactic_label}: {'; '.join(candidate_errors)}")

    return False, candidates[-1].lean_source, errors, len(candidates)


def _skeleton_only_source(request: object, spine: Sequence[object]) -> str:
    theorem_name = _theorem_name(getattr(request, "problem_id", "proof"))
    comments = "\n".join(
        f"-- {step.step_number}. {getattr(step, 'natural', '').strip()}"
        for step in spine
        if getattr(step, "natural", None)
    )
    if comments:
        comments = f"{comments}\n"
    return f"{_MATHLIB_IMPORTS}\n\n{comments}theorem {theorem_name} : Prop := by\n  sorry\n"


def _theorem_name(problem_id: str) -> str:
    slug = re.sub(r"[^a-zA-Z0-9]+", "_", problem_id).strip("_").lower()
    return f"chatdb_{slug or 'proof'}"


def _choose_goal(request: object, spine: Sequence[object]) -> str | None:
    candidates: list[str | None] = [
        getattr(request, "problem_formal_statement", None),
    ]
    verified_chain = list(getattr(request, "verified_chain", []))
    for step in reversed(verified_chain):
        if getattr(step, "proposal_type", "") == "conclusion" and getattr(step, "formal", None):
            candidates.append(getattr(step, "formal"))
            break
    for step in reversed(spine):
        candidates.append(getattr(step, "formal", None))

    final_answer = getattr(request, "final_answer", None)
    if isinstance(final_answer, str) and any(op in final_answer for op in ("<=", ">=", "<", ">", "=")):
        candidates.append(final_answer)

    for candidate in candidates:
        if candidate and _translate_formula(candidate) is not None:
            return candidate
    return None


def _theorem_params(goal: str, spine: Sequence[object]) -> str:
    variables: set[str] = set(_extract_variables(goal))
    for step in spine:
        formal = getattr(step, "formal", None)
        if formal:
            variables.update(_extract_variables(formal))
    if not variables:
        return ""
    return " ".join(f"({name} : Int)" for name in sorted(variables))


def _translate_formula(formal: str) -> str | None:
    relation = _split_relation(formal)
    if relation is None:
        return None

    lhs, operator, rhs = relation
    lean_lhs = _sympy_to_lean(lhs)
    lean_rhs = _sympy_to_lean(rhs)
    if lean_lhs is None or lean_rhs is None:
        return None
    return f"{lean_lhs} {operator} {lean_rhs}"


def _split_relation(formal: str) -> tuple[str, str, str] | None:
    text = formal.strip()
    if not text or "==" in text:
        return None

    for operator in ("<=", ">=", "<", ">", "="):
        if operator not in text:
            continue
        lhs, rhs = text.split(operator, 1)
        lhs = lhs.strip()
        rhs = rhs.strip()
        if lhs and rhs:
            return lhs, operator, rhs
    return None


def _goal_tactic_sequences(goal: str) -> list[str]:
    if any(token in goal for token in ("<=", ">=", "<", ">")):
        return [
            "omega | norm_num | simp | decide",
            "norm_num | simp | decide",
            "simp | decide",
        ]
    return [
        "ring | omega | norm_num | simp | decide",
        "norm_num | simp | decide",
        "simp | decide",
    ]


def _first_tactic(tactic_chain: str) -> str:
    return f"first | {tactic_chain}"
