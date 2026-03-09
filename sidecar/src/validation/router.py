"""Validation endpoints."""
import re
import asyncio
from fastapi import APIRouter
from pydantic import BaseModel
from typing import Optional
from .sympy_validator import validate_sympy, sympy_check
from .pint_validator import validate_pint
from .lean_validator import (
    validate_lean,
    validate_lean_native,
    _lean_available,
    _sympy_to_lean,
    _extract_variables,
    _needs_rationals,
    _run_lean,
    _MATHLIB_IMPORT,
)
from .typed_claims import verify_typed_claim

router = APIRouter()

# Math function/variable names that are NOT units
_MATH_WORDS = frozenset({
    'sin', 'cos', 'tan', 'asin', 'acos', 'atan', 'atan2',
    'sinh', 'cosh', 'tanh', 'exp', 'log', 'ln', 'sqrt', 'cbrt',
    'abs', 'pi', 'factorial', 'gamma', 'binomial', 'comb', 'perm',
    'floor', 'ceil', 'mod', 'gcd', 'lcm', 'min', 'max',
    'sum', 'prod', 'inf', 'nan', 'true', 'false',
    'Eq', 'Ne', 'Lt', 'Gt', 'Le', 'Ge',
    'Rational', 'Integer', 'Float', 'Symbol',
    'oo', 'zoo', 'I', 'E', 'S', 'N', 'O',
})


def _likely_has_units(expression: str) -> bool:
    """Heuristic: return True only if expression likely contains physical units.

    Pure math expressions use single-letter variables (x, n, k) and math
    function names (sin, sqrt). Multi-character alphabetic tokens that
    aren't recognized math words are likely units (meter, kg, second).
    """
    tokens = re.findall(r'[a-zA-Z_]\w*', expression)
    for t in tokens:
        # Single letters are variables, not units
        if len(t) <= 1:
            continue
        if t not in _MATH_WORDS:
            return True
    return False


class ValidateStepRequest(BaseModel):
    proposal_type: str
    proposal_natural: str
    proposal_formal: Optional[str] = None
    proposal_formal_lean: Optional[str] = None
    goal_state: str
    # When True, Lean validation is requested (council check).
    # When False/omitted, Lean is skipped for speed.
    run_lean: bool = False
    # Problem domain for verifier selection (e.g., "algebra", "number_theory").
    problem_domain: Optional[str] = None
    # Typed claim object — when present, dispatches to typed verifier
    # instead of equality-only SymPy validator.
    proposal_claim: Optional[dict] = None

class ValidatorResult(BaseModel):
    passed: bool
    message: str
    raw_output: str = ""
    wall_time_ms: int = 0

class ValidateStepResponse(BaseModel):
    all_passed: bool
    sympy: Optional[ValidatorResult] = None
    pint: Optional[ValidatorResult] = None
    lean: Optional[ValidatorResult] = None

class SympyCheckRequest(BaseModel):
    lhs: str
    rhs: str


class SympyCheckResponse(BaseModel):
    is_equal: bool
    diff: Optional[str] = None
    error: Optional[str] = None


class ClaimCheckRequest(BaseModel):
    claim: dict

class ClaimCheckResponse(BaseModel):
    verified: bool
    reason: str
    wall_time_ms: int = 0


@router.post("/claim_check", response_model=ClaimCheckResponse)
async def claim_check_endpoint(req: ClaimCheckRequest):
    """Pre-submission typed claim check for model self-correction.

    The model calls this with a typed claim object to verify divisibility,
    inequality, gcd, congruence, or for_all claims before submitting.
    """
    result = verify_typed_claim(req.claim, "")
    return ClaimCheckResponse(**result)


@router.post("/sympy_check", response_model=SympyCheckResponse)
async def sympy_check_endpoint(req: SympyCheckRequest):
    """Pre-submission equality check for model self-correction.

    The model calls this with its proposed formal expression split into LHS and RHS.
    If is_equal=False and diff is set, the diff tells the model exactly what's wrong
    so it can correct the formal before submitting.
    """
    result = sympy_check(req.lhs, req.rhs)
    return SympyCheckResponse(**result)


def _is_equality(expr: str) -> bool:
    """Check if expression is a single equality (has exactly one =, no ==, no <=, no >=)."""
    return ("=" in expr and "==" not in expr and "<=" not in expr
            and ">=" not in expr and expr.count("=") == 1)


def _claim_goal_from_payload(claim: dict, formal_lean: str | None) -> tuple[str, list[str], str] | None:
    """Translate a typed claim into a Lean theorem goal when feasible."""
    if formal_lean:
        variables = _extract_variables(formal_lean)
        typ = "ℚ" if "/" in formal_lean else "Int"
        return formal_lean, variables, typ

    claim_type = claim.get("type", "")
    if claim_type == "inequality":
        lhs = _sympy_to_lean(claim.get("lhs", ""))
        rhs = _sympy_to_lean(claim.get("rhs", ""))
        relation = claim.get("relation", "")
        if lhs is None or rhs is None or relation not in ("<=", "<", ">=", ">"):
            return None
        joined = f"{claim.get('lhs', '')} {claim.get('rhs', '')}"
        typ = "ℚ" if _needs_rationals(joined, joined) else "Int"
        return f"({lhs}) {relation} ({rhs})", _extract_variables(joined), typ

    if claim_type == "divisibility":
        divisor = _sympy_to_lean(claim.get("divisor", ""))
        dividend = _sympy_to_lean(claim.get("dividend", ""))
        if divisor is None or dividend is None:
            return None
        joined = f"{claim.get('divisor', '')} {claim.get('dividend', '')}"
        return f"({divisor}) ∣ ({dividend})", _extract_variables(joined), "Int"

    if claim_type == "congruence":
        expr = _sympy_to_lean(claim.get("expr", ""))
        remainder = _sympy_to_lean(claim.get("remainder", ""))
        modulus = _sympy_to_lean(claim.get("modulus", ""))
        if expr is None or remainder is None or modulus is None:
            return None
        joined = f"{claim.get('expr', '')} {claim.get('remainder', '')} {claim.get('modulus', '')}"
        goal = f"(({expr}) % ({modulus})) = (({remainder}) % ({modulus}))"
        return goal, _extract_variables(joined), "Int"

    return None


def _build_claim_theorem(goal: str, variables: list[str], typ: str) -> str:
    tactics = "first | omega | norm_num | simp | decide"
    if variables:
        params = " ".join(f"({v} : {typ})" for v in variables)
        return f"theorem check {params} : {goal} := by {tactics}"
    return f"theorem check : {goal} := by {tactics}"


async def _lean_validate_claim(
    claim: dict,
    formal: str | None,
    formal_lean: str | None,
) -> ValidatorResult | None:
    """Advisory Lean check for typed claims that can be rendered as theorem goals."""
    import time

    built = _claim_goal_from_payload(claim, formal_lean)
    if built is None:
        return None

    goal, variables, typ = built
    theorem = _build_claim_theorem(goal, variables, typ)
    start = time.perf_counter_ns()

    try:
        from ..lean_repl.router import _ensure_repl_ready

        repl = await _ensure_repl_ready()
        result = await asyncio.wait_for(repl.cmd(theorem), timeout=45.0)
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        if not result.has_errors and len(result.sorries) == 0:
            return ValidatorResult(
                passed=True,
                message="Lean verified typed claim (REPL)",
                raw_output=theorem,
                wall_time_ms=wall_ms,
            )

        errors = [m.get("data", "") for m in result.messages if m.get("severity") == "error"]
        detail = "; ".join(filter(None, errors)) or "all tactics failed"
        return ValidatorResult(
            passed=False,
            message=f"Lean: {detail}",
            raw_output=theorem,
            wall_time_ms=wall_ms,
        )
    except (asyncio.TimeoutError, RuntimeError):
        pass

    success, diagnostics, stderr = _run_lean(f"{_MATHLIB_IMPORT}{theorem}\n", timeout=45)
    wall_ms = (time.perf_counter_ns() - start) // 1_000_000
    if success:
        return ValidatorResult(
            passed=True,
            message="Lean verified typed claim",
            raw_output=theorem,
            wall_time_ms=wall_ms,
        )

    messages = [d.get("data", "") for d in diagnostics if d.get("severity") == "error"]
    detail = "; ".join(filter(None, messages)) or stderr or "all tactics failed"
    return ValidatorResult(
        passed=False,
        message=f"Lean: {detail}",
        raw_output=theorem,
        wall_time_ms=wall_ms,
    )


@router.post("/step", response_model=ValidateStepResponse)
async def validate_step(req: ValidateStepRequest):
    results = {}
    formal = req.proposal_formal
    validatable_type = req.proposal_type in ("tactic", "computation", "lemma", "algebraic")

    # Typed claim dispatch: when proposal_claim is present and type != equality,
    # use the typed verifier. Result slots into the "sympy" field so the Rust
    # pipeline needs zero changes for gating.
    if req.proposal_claim and validatable_type:
        claim_type = req.proposal_claim.get("type", "equality")
        if claim_type != "equality":
            claim_result = verify_typed_claim(req.proposal_claim, req.goal_state)
            results["sympy"] = ValidatorResult(
                passed=claim_result["verified"],
                message=claim_result["reason"],
                raw_output=str(req.proposal_claim),
                wall_time_ms=claim_result.get("wall_time_ms", 0),
            )
            if req.run_lean and _lean_available():
                lean_result = await _lean_validate_claim(
                    req.proposal_claim, formal, req.proposal_formal_lean
                )
                if lean_result is not None:
                    results["lean"] = lean_result
        else:
            # Equality typed claim — delegate to standard path
            if formal and (_is_equality(formal) or "<=" in formal or ">=" in formal):
                results["sympy"] = validate_sympy(formal, req.goal_state)
    # Standard equality path (no typed claim)
    elif formal and validatable_type and _is_equality(formal):
        results["sympy"] = validate_sympy(formal, req.goal_state)
        # Lean is advisory council — run only when requested (sampled by caller).
        if req.run_lean and _lean_available():
            results["lean"] = await _lean_validate(formal, req.proposal_formal_lean)
    # Inequality path: formal contains <= or >= (no typed claim provided)
    elif formal and validatable_type and ("<=" in formal or ">=" in formal):
        results["sympy"] = validate_sympy(formal, req.goal_state)

    # Only route to Pint if the expression likely contains physical units.
    # Pure integer/algebraic expressions (4**27 - 1, x**2 + 1) skip Pint.
    # Also skip Pint entirely for non-physics domains.
    _SKIP_PINT_DOMAINS = {"algebra", "number_theory", "combinatorics", "geometry", "analysis"}
    domain = (req.problem_domain or "").lower().replace(" ", "_")
    if req.proposal_type == "computation" and domain not in _SKIP_PINT_DOMAINS:
        expr = formal or req.proposal_natural
        if _likely_has_units(expr):
            results["pint"] = validate_pint(expr)

    # SymPy is the gatekeeper. Lean is advisory (council) — logged and surfaced
    # but never vetoes all_passed. Pint is required when present.
    if "sympy" in results:
        all_passed = results["sympy"].passed
        if "pint" in results and not results["pint"].passed:
            all_passed = False
        # Lean result is carried through for logging/display but doesn't gate.
    else:
        # No SymPy ran — fall back to whatever validators ran (excluding Lean)
        non_lean = {k: v for k, v in results.items() if k != "lean"}
        all_passed = all(r.passed for r in non_lean.values()) if non_lean else False
    return ValidateStepResponse(all_passed=all_passed, **results)


async def _lean_validate(formal: str, formal_lean: str | None) -> ValidatorResult | None:
    """Validate via Lean — prefer persistent REPL (<1s), fall back to subprocess (~14s).

    Returns None when the expression can't be translated to Lean (skip, don't fail).
    """
    import time
    start = time.perf_counter_ns()

    # Try the persistent REPL first (sub-second when warm)
    try:
        from ..lean_repl.router import _ensure_repl_ready
        repl = await _ensure_repl_ready()
        if True:  # repl is guaranteed READY after _ensure_repl_ready
            # If solver provided native Lean, split and check directly
            if formal_lean and "=" in formal_lean and "==" not in formal_lean:
                parts = formal_lean.split("=", 1)
                lhs, rhs = parts[0].strip(), parts[1].strip()
                variables = _extract_variables(formal_lean)
                typ = "ℚ" if _needs_rationals(lhs, rhs) else "Int"
            else:
                # Auto-translate from SymPy syntax
                parts = formal.split("=")
                if len(parts) != 2:
                    wall_ms = (time.perf_counter_ns() - start) // 1_000_000
                    return ValidatorResult(passed=False, message="Cannot split formal into LHS = RHS", wall_time_ms=wall_ms)
                lean_lhs = _sympy_to_lean(parts[0])
                lean_rhs = _sympy_to_lean(parts[1])
                if lean_lhs is None or lean_rhs is None:
                    return None  # untranslatable — skip Lean
                lhs, rhs = lean_lhs, lean_rhs
                variables = _extract_variables(formal)
                typ = "ℚ" if _needs_rationals(lhs, rhs) else "Int"

            result = await asyncio.wait_for(
                repl.check_equality(lhs, rhs, variables, typ),
                timeout=45.0,
            )
            wall_ms = (time.perf_counter_ns() - start) // 1_000_000
            # Reconstruct the Lean code that was sent so raw_output is useful for feedback
            if variables:
                params = " ".join(f"({v} : {typ})" for v in variables)
                lean_code = f"theorem check {params} : {lhs} = {rhs} := by first | ring | omega | norm_num | simp | decide"
            else:
                lean_code = f"theorem check : {lhs} = {rhs} := by first | ring | omega | norm_num | simp | decide"
            if result.passed:
                return ValidatorResult(passed=True, message="Lean verified (REPL)", raw_output=lean_code, wall_time_ms=wall_ms)
            else:
                return ValidatorResult(
                    passed=False,
                    message=f"Lean: {result.error or 'all tactics failed'}",
                    raw_output=lean_code,
                    wall_time_ms=wall_ms,
                )
    except (asyncio.TimeoutError, RuntimeError):
        pass  # fall through to subprocess

    # Fallback: subprocess validator (~14s)
    try:
        if formal_lean:
            lean_result = await asyncio.wait_for(
                asyncio.get_event_loop().run_in_executor(
                    None, validate_lean_native, formal_lean
                ),
                timeout=45.0,
            )
        else:
            lean_result = await asyncio.wait_for(
                asyncio.get_event_loop().run_in_executor(
                    None, validate_lean, formal, ""
                ),
                timeout=45.0,
            )
        return lean_result
    except asyncio.TimeoutError:
        return ValidatorResult(passed=False, message="Lean timed out (advisory)", wall_time_ms=30000)
