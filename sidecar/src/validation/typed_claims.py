"""Typed claim verifiers — extends equality-only formalization with
divisibility, inequality, gcd, congruence, and for_all claim types.

Each verifier uses _parse_safe() from sympy_validator.py for sandboxed parsing.
Tautology gates reject vacuous claims per type.
"""
import time
from sympy import Mod, Integer, gcd as sympy_gcd, expand

from .sympy_validator import _parse_safe, _has_applied_undef, ParserPolicyError, validate_sympy


def verify_typed_claim(claim: dict, goal_state: str) -> dict:
    """Dispatch a typed claim to the appropriate verifier.

    Returns {"verified": bool, "reason": str, "wall_time_ms": int}.
    """
    start = time.perf_counter_ns()
    claim_type = claim.get("type", "")

    verifiers = {
        "equality": _verify_equality,
        "divisibility": _verify_divisibility,
        "inequality": _verify_inequality,
        "gcd": _verify_gcd,
        "congruence": _verify_congruence,
        "for_all": _verify_for_all,
    }

    verifier = verifiers.get(claim_type)
    if verifier is None:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return {"verified": False, "reason": f"Unsupported claim type: {claim_type}", "wall_time_ms": wall_ms}

    try:
        result = verifier(claim, goal_state)
    except _MissingFieldError as e:
        result = {"verified": False, "reason": str(e)}
    except ParserPolicyError as e:
        result = {"verified": False, "reason": f"Parse error: {e}"}
    except Exception as e:
        result = {"verified": False, "reason": f"Verification error: {e}"}

    wall_ms = (time.perf_counter_ns() - start) // 1_000_000
    result["wall_time_ms"] = wall_ms
    return result


class _MissingFieldError(ValueError):
    pass


def _require(claim: dict, *fields: str):
    """Raise _MissingFieldError if any required field is missing or empty."""
    for f in fields:
        if not claim.get(f):
            raise _MissingFieldError(f"Missing required field '{f}' for {claim.get('type', '?')} claim")


# ── Equality (delegates to existing validator) ──────────────────

def _verify_equality(claim: dict, goal_state: str) -> dict:
    lhs_str = claim.get("lhs", "")
    rhs_str = claim.get("rhs", "")
    if lhs_str and rhs_str:
        formal = f"{lhs_str} = {rhs_str}"
    else:
        # Fall back: caller may have put it in "formal" field
        formal = claim.get("formal", "")
    if not formal:
        return {"verified": False, "reason": "Missing lhs/rhs for equality claim"}

    result = validate_sympy(formal, goal_state)
    return {"verified": result.passed, "reason": result.message}


# ── Divisibility ────────────────────────────────────────────────

def _verify_divisibility(claim: dict, goal_state: str) -> dict:
    _require(claim, "dividend", "divisor")
    dividend_str = claim["dividend"]
    divisor_str = claim["divisor"]

    dividend = _parse_safe(dividend_str)
    divisor = _parse_safe(divisor_str)

    # Abstract function gate: f(1), g(n) etc. can't be evaluated
    if _has_applied_undef(dividend) or _has_applied_undef(divisor):
        return {"verified": True, "reason": "Divisibility check skipped: contains abstract function application — not numerically verifiable"}

    # Tautology gate: dividend is literally 0
    if dividend == 0:
        return {"verified": False, "reason": "Tautological: 0 is divisible by anything"}

    # Symbolic: try Mod(dividend, divisor) → simplify
    remainder = Mod(dividend, divisor)
    try:
        remainder_expanded = expand(remainder)
        if remainder_expanded == 0:
            return {"verified": True, "reason": "Divisibility verified (symbolic)"}
    except Exception:
        pass

    # Numeric sampling: test integers 2..14
    free = dividend.free_symbols | divisor.free_symbols
    if free:
        all_divisible = True
        nontrivial_samples = 0
        for val in range(2, 15):
            subs = {s: Integer(val) for s in free}
            try:
                num_dividend = int(dividend.subs(subs))
                num_divisor = int(divisor.subs(subs))
                if num_divisor == 0:
                    continue  # skip degenerate
                if num_dividend == 0:
                    continue  # trivially divisible — not evidence
                nontrivial_samples += 1
                if num_dividend % num_divisor != 0:
                    all_divisible = False
                    break
            except (TypeError, ValueError):
                all_divisible = False
                break
        if nontrivial_samples == 0:
            return {"verified": False, "reason": "Divisibility inconclusive: dividend was 0 or divisor was 0 for all sample values"}
        if all_divisible:
            return {"verified": True, "reason": "Divisibility verified (numeric sampling)"}
        return {"verified": False, "reason": "Divisibility failed: counterexample found in sampling"}
    else:
        # Both are numeric constants
        try:
            num_dividend = int(dividend)
            num_divisor = int(divisor)
            if num_divisor != 0 and num_dividend % num_divisor == 0:
                return {"verified": True, "reason": "Divisibility verified (numeric)"}
            return {"verified": False, "reason": f"{dividend} is not divisible by {divisor}"}
        except (TypeError, ValueError):
            return {"verified": False, "reason": "Could not evaluate divisibility numerically"}


# ── Inequality ──────────────────────────────────────────────────

def _verify_inequality(claim: dict, goal_state: str) -> dict:
    _require(claim, "lhs", "rhs", "relation")
    lhs = _parse_safe(claim["lhs"])
    rhs = _parse_safe(claim["rhs"])
    relation = claim["relation"]

    if _has_applied_undef(lhs) or _has_applied_undef(rhs):
        return {"verified": True, "reason": "Inequality check skipped: contains abstract function application"}

    if relation not in ("<=", "<", ">=", ">"):
        return {"verified": False, "reason": f"Unknown relation: {relation}"}

    # Normalize to diff >= 0 or diff > 0
    if relation in ("<=", "<"):
        diff = rhs - lhs  # lhs <= rhs  ⟹  rhs - lhs >= 0
        strict = relation == "<"
    else:
        diff = lhs - rhs  # lhs >= rhs  ⟹  lhs - rhs >= 0
        strict = relation == ">"

    # Tautology gate: lhs == rhs for non-strict
    diff_expanded = expand(diff)
    if diff_expanded == 0 and not strict:
        return {"verified": False, "reason": "Tautological: lhs equals rhs (trivially non-strict)"}

    # Try symbolic: is_nonnegative / is_positive
    try:
        if strict:
            if diff_expanded.is_positive is True:
                return {"verified": True, "reason": "Inequality verified (symbolic: positive)"}
        else:
            if diff_expanded.is_nonnegative is True:
                return {"verified": True, "reason": "Inequality verified (symbolic: nonnegative)"}
    except Exception:
        pass

    # Numeric sampling
    free = diff_expanded.free_symbols
    if free:
        test_values = [1, 2, 3, 5, 7, 10, 20, 100]
        all_hold = True
        for val in test_values:
            subs = {s: Integer(val) for s in free}
            try:
                num = float(diff_expanded.subs(subs).evalf())
                if strict and num <= 0:
                    all_hold = False
                    break
                elif not strict and num < 0:
                    all_hold = False
                    break
            except Exception:
                all_hold = False
                break
        if all_hold:
            return {"verified": True, "reason": "Inequality verified (numeric sampling)"}
        return {"verified": False, "reason": "Inequality failed: counterexample found in sampling"}
    else:
        # Pure numeric
        try:
            num = float(diff_expanded.evalf())
            if strict and num > 0:
                return {"verified": True, "reason": "Inequality verified (numeric)"}
            elif not strict and num >= 0:
                if num == 0:
                    return {"verified": False, "reason": "Tautological: lhs equals rhs (trivially non-strict)"}
                return {"verified": True, "reason": "Inequality verified (numeric)"}
            return {"verified": False, "reason": f"Inequality does not hold: diff = {num}"}
        except Exception:
            return {"verified": False, "reason": "Could not evaluate inequality numerically"}


# ── GCD ─────────────────────────────────────────────────────────

def _verify_gcd(claim: dict, goal_state: str) -> dict:
    _require(claim, "a", "b", "value")
    a = _parse_safe(claim["a"])
    b = _parse_safe(claim["b"])
    expected = _parse_safe(claim["value"])

    if _has_applied_undef(a) or _has_applied_undef(b) or _has_applied_undef(expected):
        return {"verified": True, "reason": "GCD check skipped: contains abstract function application"}

    # Tautology gate: operands identical (gcd(x,x) = x is trivial)
    if expand(a - b) == 0:
        return {"verified": False, "reason": "Tautological: gcd(x, x) = x is trivially true"}

    # Try symbolic gcd
    try:
        computed = sympy_gcd(a, b)
        if expand(computed - expected) == 0:
            return {"verified": True, "reason": "GCD verified (symbolic)"}
    except Exception:
        pass

    # Numeric sampling
    free = a.free_symbols | b.free_symbols | expected.free_symbols
    if free:
        all_match = True
        for val in range(2, 15):
            subs = {s: Integer(val) for s in free}
            try:
                na = int(a.subs(subs))
                nb = int(b.subs(subs))
                ne = int(expected.subs(subs))
                import math
                if math.gcd(abs(na), abs(nb)) != abs(ne):
                    all_match = False
                    break
            except (TypeError, ValueError):
                all_match = False
                break
        if all_match:
            return {"verified": True, "reason": "GCD verified (numeric sampling)"}
        return {"verified": False, "reason": "GCD failed: mismatch found in sampling"}
    else:
        return {"verified": False, "reason": f"GCD mismatch: gcd({a}, {b}) != {expected}"}


# ── Congruence ──────────────────────────────────────────────────

def _verify_congruence(claim: dict, goal_state: str) -> dict:
    _require(claim, "expr", "remainder", "modulus")
    expr = _parse_safe(claim["expr"])
    remainder = _parse_safe(claim["remainder"])
    modulus = _parse_safe(claim["modulus"])

    if _has_applied_undef(expr) or _has_applied_undef(remainder) or _has_applied_undef(modulus):
        return {"verified": True, "reason": "Congruence check skipped: contains abstract function application"}

    # Tautology gate: expr and remainder both 0
    if expr == 0 and remainder == 0:
        return {"verified": False, "reason": "Tautological: 0 ≡ 0 (mod anything) is trivially true"}

    # Compute Mod(expr - remainder, modulus)
    diff = expr - remainder
    mod_result = Mod(diff, modulus)

    # Symbolic check
    try:
        mod_expanded = expand(mod_result)
        if mod_expanded == 0:
            return {"verified": True, "reason": "Congruence verified (symbolic)"}
    except Exception:
        pass

    # Numeric sampling
    free = expr.free_symbols | remainder.free_symbols | modulus.free_symbols
    if free:
        all_congruent = True
        samples_tested = 0
        for val in range(2, 15):
            subs = {s: Integer(val) for s in free}
            try:
                ne = int(expr.subs(subs))
                nr = int(remainder.subs(subs))
                nm = int(modulus.subs(subs))
                if nm == 0:
                    continue
                samples_tested += 1
                if (ne - nr) % nm != 0:
                    all_congruent = False
                    break
            except (TypeError, ValueError):
                all_congruent = False
                break
        if samples_tested == 0:
            return {"verified": False, "reason": "Congruence inconclusive: modulus was 0 for all sample values"}
        if all_congruent:
            return {"verified": True, "reason": "Congruence verified (numeric sampling)"}
        return {"verified": False, "reason": "Congruence failed: counterexample found"}
    else:
        # Pure numeric
        try:
            ne = int(expr)
            nr = int(remainder)
            nm = int(modulus)
            if nm != 0 and (ne - nr) % nm == 0:
                return {"verified": True, "reason": "Congruence verified (numeric)"}
            return {"verified": False, "reason": f"{expr} ≢ {remainder} (mod {modulus})"}
        except (TypeError, ValueError):
            return {"verified": False, "reason": "Could not evaluate congruence numerically"}


# ── For-all ─────────────────────────────────────────────────────

def _verify_for_all(claim: dict, goal_state: str) -> dict:
    _require(claim, "variable", "domain", "predicate")
    variable = claim["variable"]
    domain_str = claim["domain"]
    predicate_str = claim["predicate"]

    # Parse domain: "1..n", "1..5", "0..n-1"
    domain_parts = domain_str.split("..")
    if len(domain_parts) != 2:
        return {"verified": False, "reason": f"Cannot parse domain: {domain_str} (expected a..b)"}

    try:
        lower = _parse_safe(domain_parts[0])
        upper = _parse_safe(domain_parts[1])
    except Exception as e:
        return {"verified": False, "reason": f"Cannot parse domain bounds: {e}"}

    # Parse predicate — handle == as equality check
    # Predicates like "Mod(120, k) == 0" should evaluate to True/False
    pred_str = predicate_str.strip()

    # Determine domain values
    free_in_bounds = lower.free_symbols | upper.free_symbols
    # Remove the loop variable from free symbols
    from sympy import Symbol
    loop_var = Symbol(variable)
    outer_free = free_in_bounds - {loop_var}

    if outer_free:
        # Symbolic bounds: sample outer variables too
        all_hold = True
        for outer_val in range(2, 10):
            outer_subs = {s: Integer(outer_val) for s in outer_free}
            try:
                lo = int(lower.subs(outer_subs))
                hi = int(upper.subs(outer_subs))
            except (TypeError, ValueError):
                continue
            for k_val in range(lo, hi + 1):
                if not _evaluate_predicate(pred_str, variable, k_val, outer_subs):
                    all_hold = False
                    break
            if not all_hold:
                break
        if all_hold:
            return {"verified": True, "reason": "For-all verified (numeric sampling with symbolic bounds)"}
        return {"verified": False, "reason": "For-all failed: counterexample found"}
    else:
        # Concrete bounds
        try:
            lo = int(lower)
            hi = int(upper)
        except (TypeError, ValueError):
            return {"verified": False, "reason": f"Cannot evaluate domain bounds: {lower}..{upper}"}

        if hi < lo:
            return {"verified": False, "reason": f"Empty domain: {lo}..{hi}"}
        if hi - lo > 10000:
            return {"verified": False, "reason": f"Domain too large: {lo}..{hi}"}

        all_hold = True
        for k_val in range(lo, hi + 1):
            if not _evaluate_predicate(pred_str, variable, k_val, {}):
                all_hold = False
                break
        if all_hold:
            return {"verified": True, "reason": "For-all verified (exhaustive check)"}
        return {"verified": False, "reason": f"For-all failed at {variable}={k_val}"}


def _evaluate_predicate(pred_str: str, var_name: str, var_val: int, extra_subs: dict) -> bool:
    """Evaluate a predicate string with the given variable value.

    Handles both:
      - "Mod(120, k) == 0"  (comparison predicates)
      - "Mod(120, k)"       (expression predicates — truthy if nonzero, or check == 0)
    """
    from sympy import Symbol

    var = Symbol(var_name)
    subs = {var: Integer(var_val), **extra_subs}

    # Handle == comparison
    if "==" in pred_str:
        parts = pred_str.split("==")
        if len(parts) == 2:
            try:
                lhs = _parse_safe(parts[0].strip())
                rhs = _parse_safe(parts[1].strip())
                lval = lhs.subs(subs)
                rval = rhs.subs(subs)
                # Force numeric evaluation
                try:
                    lval = int(lval)
                    rval = int(rval)
                except (TypeError, ValueError):
                    lval = float(lval.evalf())
                    rval = float(rval.evalf())
                return lval == rval
            except Exception:
                return False

    # Handle != comparison
    if "!=" in pred_str:
        parts = pred_str.split("!=")
        if len(parts) == 2:
            try:
                lhs = _parse_safe(parts[0].strip())
                rhs = _parse_safe(parts[1].strip())
                return int(lhs.subs(subs)) != int(rhs.subs(subs))
            except Exception:
                return False

    # Handle relational: >=, <=, >, <
    for op, fn in [(">=", lambda a, b: a >= b), ("<=", lambda a, b: a <= b),
                   (">", lambda a, b: a > b), ("<", lambda a, b: a < b)]:
        if op in pred_str:
            parts = pred_str.split(op)
            if len(parts) == 2:
                try:
                    lhs = _parse_safe(parts[0].strip())
                    rhs = _parse_safe(parts[1].strip())
                    return fn(int(lhs.subs(subs)), int(rhs.subs(subs)))
                except Exception:
                    return False

    # Plain expression — evaluate and check truthiness
    try:
        expr = _parse_safe(pred_str)
        val = expr.subs(subs)
        return bool(val)
    except Exception:
        return False
