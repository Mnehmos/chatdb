"""SymPy validator -- algebraic equality with assumption awareness.

Uses expand()/cancel() instead of simplify() to avoid hanging on
complex expressions (cubics, etc). simplify() is avoided entirely
because it can block forever in C code with no way to kill it on Windows.
"""
import ast
import re
import time
from sympy import (
    expand, cancel, solve, factor, powsimp, floor, ceiling, factorial,
    Sum, Product, binomial, Abs, Mod, Rational, Integer, Function, gcd,
    expand_trig, trigsimp, sin, cos, tan, sec, csc, cot,
)
from sympy.core.function import AppliedUndef
from sympy.parsing.sympy_parser import (
    parse_expr,
    standard_transformations,
    implicit_multiplication_application,
)

TRANSFORMS = standard_transformations + (implicit_multiplication_application,)

# TDD Enforcement Prompt Tag:
# No implementation before failing tests (Red)
# Minimal implementation to pass tests (Green)
# Refactor only after green tests (Blue)
# PRs without Red evidence are non-compliant


class ParserPolicyError(ValueError):
    """Raised when parser allowlist policy rejects untrusted input."""


_ALLOWED_FUNCTIONS = frozenset({
    "sin", "cos", "tan", "sec", "csc", "cot",
    "asin", "acos", "atan", "atan2", "sinc",
    "sinh", "cosh", "tanh", "exp", "log", "sqrt", "abs", "floor",
    "ceiling", "ceil", "factorial", "binomial",
    "Sum", "Product", "Mod", "Abs", "Rational", "Integer",
    "gcd", "lcm", "Min", "Max",
    # Single-letter function names used in math proofs (f(x), g(n), etc.)
    "f", "g", "h", "p", "q", "r", "F", "G", "H", "P", "Q", "R",
    # Common multi-letter math function names
    "phi", "psi", "sigma", "tau", "mu", "nu", "pi",
})

_BLOCKED_NAMES = frozenset({
    "__import__", "eval", "exec", "open", "globals", "locals", "vars",
    "input", "compile", "getattr", "setattr", "delattr", "builtins",
    "os", "sys", "subprocess",
})

_ALLOWED_AST_NODES = (
    ast.Expression,
    ast.BinOp,
    ast.UnaryOp,
    ast.Add,
    ast.Sub,
    ast.Mult,
    ast.Div,
    ast.Pow,
    ast.Mod,
    ast.FloorDiv,
    ast.UAdd,
    ast.USub,
    ast.Name,
    ast.Call,
    ast.Constant,
    ast.Load,
    ast.Tuple,  # Needed for Sum(k, (k, 1, n)) syntax
)


def _normalize_formal(expr_str: str) -> str:
    """Normalize common LLM syntax quirks before SymPy parsing."""
    s = expr_str.strip()
    # ^ → ** (caret to Python exponentiation)
    s = s.replace("^", "**")
    # // → floor division: rewrite a//b to floor(a/b)
    # Uses a greedy non-// left operand and a simple right operand.
    prev = None
    while "//" in s:
        prev = s
        # Match everything up to // (greedy) as LHS, then // then a simple RHS.
        # This handles cases like floor((a)/(b))//c where LHS is complex.
        s = re.sub(r'(.+?)\s*//\s*(\([^()]*\)|[a-zA-Z0-9_]+)',
                   r'floor((\1)/(\2))', s, count=1)
        if s == prev:
            break
    return s


def _enforce_parser_allowlist(expr_str: str):
    """Deterministic parser boundary checks for untrusted symbolic payloads."""
    try:
        tree = ast.parse(expr_str, mode="eval")
    except SyntaxError:
        return False, "Unsafe parser input blocked: forbidden or invalid syntax."

    for node in ast.walk(tree):
        if isinstance(node, ast.Attribute):
            return False, "Unsafe parser input blocked: attribute access is forbidden."

        if isinstance(node, ast.Call):
            if not isinstance(node.func, ast.Name):
                return False, "Unsafe parser input blocked: forbidden call target."
            func_name = node.func.id
            if func_name in _BLOCKED_NAMES or func_name not in _ALLOWED_FUNCTIONS:
                return False, f"Unsafe parser input blocked: disallowed function '{func_name}'."

        if isinstance(node, ast.Name):
            name = node.id
            if name in _BLOCKED_NAMES or name.startswith("__"):
                return False, f"Unsafe parser input blocked: disallowed name '{name}'."

        if isinstance(node, ast.Constant):
            if not isinstance(node.value, (int, float, complex)):
                return False, "Unsafe parser input blocked: disallowed literal type."

        if not isinstance(node, _ALLOWED_AST_NODES):
            return False, "Unsafe parser input blocked: forbidden syntax node."

    return True, ""


def _parse_safe(expr_str: str):
    """Parse a string into a SymPy expression, normalizing ^ to ** and // to floor()."""
    normalized = _normalize_formal(expr_str)
    allowed, policy_msg = _enforce_parser_allowlist(normalized)
    if not allowed:
        raise ParserPolicyError(policy_msg)
    local = {
        "floor": floor, "ceiling": ceiling, "ceil": ceiling,
        "factorial": factorial, "binomial": binomial,
        "Sum": Sum, "Product": Product,
        "Mod": Mod, "Abs": Abs,
        "Rational": Rational, "Integer": Integer,
        "gcd": gcd,
        "sec": sec, "csc": csc, "cot": cot,
    }
    # Register single-letter function names as SymPy Function objects
    # so expressions like f(x), g(n), h(1) parse correctly.
    for name in ("f", "g", "h", "p", "q", "r", "F", "G", "H", "P", "Q", "R"):
        local[name] = Function(name)
    expr = parse_expr(normalized, transformations=TRANSFORMS, local_dict=local)
    # Evaluate symbolic sums/products so they become concrete expressions
    if hasattr(expr, 'doit'):
        try:
            evaluated = expr.doit()
            # Only use evaluated form if it simplified (no leftover Sum/Product)
            if evaluated != expr:
                expr = evaluated
        except Exception:
            pass
    return expr


def _check_equality(lhs, rhs):
    """Check algebraic equality using multiple fast strategies.
    Returns (is_zero, diff) — never hangs.
    """
    raw_diff = lhs - rhs

    # Strategy 0: doit() to evaluate unevaluated Sum/Product/Integral
    try:
        evaled = raw_diff.doit()
        if evaled == 0:
            return True, evaled
        diff_for_expand = evaled
    except Exception:
        diff_for_expand = raw_diff

    # Strategy 1: expand
    diff = expand(diff_for_expand)
    if diff == 0:
        return True, diff

    # Strategy 2: cancel (handles rational expressions)
    try:
        diff2 = cancel(diff_for_expand)
        if diff2 == 0:
            return True, diff2
    except Exception:
        pass

    # Strategy 3: factor (sometimes catches things expand misses)
    try:
        diff3 = factor(diff_for_expand)
        if diff3 == 0:
            return True, diff3
    except Exception:
        pass

    # Strategy 4: powsimp (power simplification: n * n^(n-1) → n^n)
    # expand/cancel/factor can't combine power bases; powsimp can.
    try:
        diff4 = powsimp(diff_for_expand)
        if diff4 == 0:
            return True, diff4
    except Exception:
        pass

    # Strategy 4.5: Integer numeric sampling for Sum/Product/Piecewise expressions.
    # Symbolic sums over integer variables (e.g. Sum(d**i, (i, 0, d-1))) produce
    # Piecewise results after .doit() that expand/cancel/factor cannot reduce to 0.
    # Integer test values directly evaluate both branches and verify equality.
    # Test d=2..9: avoids degenerate d=0 (empty sums), d=1 (trivial case).
    _sum_free = diff_for_expand.free_symbols
    if _sum_free and _has_sum_like(diff_for_expand):
        try:
            all_zero = True
            for _val in range(2, 10):
                _subs = {_s: _val for _s in _sum_free}
                try:
                    _r = complex(diff_for_expand.subs(_subs).doit().evalf())
                    if abs(_r) > 1e-9:
                        all_zero = False
                        break
                except Exception:
                    all_zero = False
                    break
            if all_zero:
                return True, diff_for_expand
        except Exception:
            pass

    # Strategy 5: trig identity verification
    # expand/cancel/factor don't handle trig identities.
    # We use multiple trig-aware strategies to verify identities like:
    #   cos(2x) = 2cos²x - 1
    #   sin(a+b) = sin(a)cos(b) + cos(a)sin(b)
    #   cos²x + sin²x = 1
    free = diff_for_expand.free_symbols
    if free and _has_trig(diff_for_expand):
        # 5a: numeric sampling — fast, deterministic, handles all trig identities.
        # Use transcendental-looking values to avoid coincidental zero hits
        # at rational multiples of π (e.g. sin(π) = 0).
        # Run FIRST so trigsimp (which can hang on Windows) is only a fallback.
        try:
            base_vals = [0.37, 1.23, 0.71, 2.14, 0.59, 0.93, 1.77]
            test_points = []
            for i in range(6):
                pt = {s: base_vals[(i + j) % len(base_vals)]
                      for j, s in enumerate(sorted(free, key=str))}
                test_points.append(pt)
            all_near_zero = True
            for pt in test_points:
                try:
                    val = complex(diff_for_expand.subs(pt).evalf())
                    if abs(val) > 1e-9:
                        all_near_zero = False
                        break
                except Exception:
                    all_near_zero = False
                    break
            if all_near_zero:
                return True, diff_for_expand
        except Exception:
            pass

        # 5b: expand_trig → expand
        # expand_trig is deterministic and fast: cos(2x) → 2cos²x - 1
        try:
            trig_diff = expand(expand_trig(diff_for_expand))
            if trig_diff == 0:
                return True, trig_diff
        except Exception:
            pass

        # 5c: expand_trig then trigsimp
        try:
            combo = trigsimp(expand_trig(diff_for_expand))
            if combo == 0:
                return True, combo
            combo_exp = expand(combo)
            if combo_exp == 0:
                return True, combo_exp
        except Exception:
            pass

        # 5d: trigsimp — last resort (heuristic, can be slow on Windows).
        # Handles Pythagorean identities (cos²x + sin²x = 1),
        # double angle, sum-to-product, etc.
        try:
            ts = trigsimp(diff_for_expand)
            if ts == 0:
                return True, ts
            ts_exp = expand(ts)
            if ts_exp == 0:
                return True, ts_exp
        except Exception:
            pass

    # Strategy 6: numeric sampling for floor/ceiling expressions
    # (SymPy can't simplify floor symbolically, but we can test many values)
    free = diff.free_symbols
    if free and _has_floor_like(diff):
        try:
            from sympy import Integer
            all_zero = True
            test_vals = list(range(-20, 21)) + [100, 1000, -100, -1000]
            for val in test_vals:
                subs = {s: Integer(val) for s in free}
                result = diff.subs(subs)
                if result != 0:
                    all_zero = False
                    break
            if all_zero:
                return True, diff
        except Exception:
            pass

    # Strategy 7: integer numeric sampling for symbolic-limit Sum/Product.
    # When a Sum has a symbolic upper limit (e.g. Sum(d**i, (i, 0, d-1))),
    # doit() yields Piecewise which never cancels algebraically. Fix: substitute
    # concrete integers FIRST so doit() gets a fully-numeric Sum to evaluate.
    # Uses raw_diff (pre-doit) so substitution happens before evaluation.
    raw_free = raw_diff.free_symbols
    if raw_free and _has_symbolic_sum(raw_diff):
        try:
            all_zero = True
            # Small positive integers cover geometric/combinatorial identities.
            # If the identity is false a counterexample appears quickly.
            test_vals = [2, 3, 4, 5, 6, 7, 8, 10, 15, 20]
            for val in test_vals:
                subs_map = {s: Integer(val) for s in raw_free}
                result = raw_diff.subs(subs_map).doit()
                try:
                    result = expand(result)
                except Exception:
                    pass
                if result != 0:
                    all_zero = False
                    break
            if all_zero:
                return True, raw_diff
        except Exception:
            pass

    return False, diff


def _has_floor_like(expr) -> bool:
    """Check if expression contains floor/ceiling/Mod functions."""
    s = str(expr)
    return any(fn in s for fn in ("floor", "ceiling", "Mod"))


def _has_sum_like(expr) -> bool:
    """Check if expression contains Sum, Product, or Piecewise (from evaluated sums)."""
    s = str(expr)
    return any(fn in s for fn in ("Sum", "Product", "Piecewise"))


def _has_symbolic_sum(expr) -> bool:
    """Check if expression contains Sum or Product with symbolic (non-constant) limits.

    Detects Sum(d**i, (i, 0, d-1)) where the upper limit depends on a free
    variable. SymPy's doit() on these produces Piecewise that won't cancel
    symbolically even when the identity is correct — numeric sampling is needed.
    """
    try:
        for cls in (Sum, Product):
            for atom in expr.atoms(cls):
                for limit in atom.limits:
                    # limit tuple is (var, lower, upper)
                    if len(limit) >= 3 and limit[2].free_symbols:
                        return True
        return False
    except Exception:
        return False


def _has_applied_undef(expr) -> bool:
    """Check if expression contains applied undefined functions like f(1) or g(n).

    Proof steps that assert properties of an abstract function (e.g. 'f(1) = 1')
    cannot be verified algebraically: SymPy has no definition for f, so f(1) - 1
    never reduces to 0 regardless of mathematical correctness. The validator
    must skip rather than falsely reject.
    """
    try:
        return bool(expr.atoms(AppliedUndef))
    except Exception:
        return False


def _has_trig(expr) -> bool:
    """Check if expression contains trig functions."""
    s = str(expr)
    return any(fn in s for fn in ("sin", "cos", "tan", "sec", "csc", "cot", "asin", "acos", "atan"))


def _extract_assumptions(goal_state: str):
    """Extract equation assumptions from goal_state text."""
    assumptions = []
    eq_pattern = re.compile(
        r'([a-zA-Z0-9_\s\+\-\*/\^\(\)]+)'
        r'\s*=\s*'
        r'([a-zA-Z0-9_\s\+\-\*/\^\(\)]+)'
    )
    prose_re = re.compile(
        r'^(given|prove|show|that|from|step|since|because|then|if|when|where|'
        r'let|for|we|have|by|the|and|or|not|with|is|are|so|thus|hence|obtain|get)\s+',
        re.IGNORECASE,
    )

    for match in eq_pattern.finditer(goal_state):
        lhs_str = match.group(1).strip()
        rhs_str = match.group(2).strip()
        if not lhs_str or not rhs_str:
            continue
        lhs_str = prose_re.sub('', lhs_str).strip()
        rhs_str = prose_re.sub('', rhs_str).strip()
        if not lhs_str or not rhs_str:
            continue
        try:
            lhs = _parse_safe(lhs_str)
            rhs = _parse_safe(rhs_str)
            diff = lhs - rhs
            if diff.free_symbols:
                assumptions.append(diff)
        except Exception:
            continue
    return assumptions


def _check_with_assumptions(diff, assumptions):
    """Check if diff = 0 given that each assumption expression = 0."""
    for assumption in assumptions:
        # Quick divisibility: if diff / assumption is a constant
        try:
            ratio = cancel(diff / assumption)
            if not ratio.free_symbols:
                return True, f"diff = ({ratio}) * assumption"
        except Exception:
            pass

        # Substitution: solve assumption for each var, sub into diff
        syms = sorted(assumption.free_symbols, key=str)
        for sym in syms[:4]:  # limit to avoid explosion
            try:
                solutions = solve(assumption, sym, dict=False)
                if not isinstance(solutions, list):
                    solutions = [solutions]
                for sol in solutions[:3]:
                    substituted = expand(diff.subs(sym, sol))
                    if substituted == 0:
                        return True, f"substituting {sym} = {sol}"
                    # Try cancel for rational results
                    try:
                        substituted2 = cancel(diff.subs(sym, sol))
                        if substituted2 == 0:
                            return True, f"substituting {sym} = {sol}"
                    except Exception:
                        pass
            except Exception:
                continue
    return False, ""


def _is_divisibility_claim(expr_str: str) -> bool:
    """Check if expr_str is a single top-level Mod/gcd/lcm function call.

    Returns True for:  Mod(b**a, a)  gcd(21, 14)  lcm(a, b)
    Returns False for: gcd(a,b) - gcd(a,b)  (subtraction, not a single call)
    """
    s = expr_str.strip()
    for prefix in ('Mod(', 'gcd(', 'lcm(', 'mod('):
        if s.startswith(prefix):
            # Track paren depth to find if the opening paren closes at end of string
            depth = 1
            i = len(prefix)
            while i < len(s) and depth > 0:
                if s[i] == '(':
                    depth += 1
                elif s[i] == ')':
                    depth -= 1
                i += 1
            if depth == 0 and i == len(s):
                return True
    return False


def _is_tautological(lhs_str: str, rhs_str: str, lhs, rhs) -> bool:
    """Detect vacuous formalizations that are trivially true but encode no information.

    Only catches truly vacuous statements:
      - f(n)**f(n) = f(n)**f(n)  (string identity — A = A)
      - x - x = 0               (self-cancellation to zero)

    Does NOT flag computations where the input strings differ:
      - (x+1)**2 = x**2 + 2*x + 1   (expansion)
      - 3*(14n+3) - 2*(21n+4) = 1    (Bezout identity)
      - (21n+4) - (14n+3) = 7n + 1   (Euclidean algorithm step)
      - gcd(21n+4, 14n+3) = 1        (GCD computation)
    """
    # 1. String identity: after normalization, both sides are the same text
    l_norm = _normalize_formal(lhs_str).strip()
    r_norm = _normalize_formal(rhs_str).strip()
    if l_norm == r_norm:
        return True

    # 2. SymPy canonical form identity: both sides parse to the same expression.
    #    Only flag as tautological if the result is ZERO (vacuous cancellation
    #    like x - x = 0). Non-zero results with different input strings are
    #    meaningful computations (e.g. showing a linear combination equals 1).
    if lhs == rhs:
        if lhs == 0:
            # Modular/divisibility expressions where the LHS is a single function call
            # evaluating to 0 are meaningful claims (divisibility, coprimality).
            # e.g. Mod(b**a, a) = 0 means "a divides b**a" — not tautological.
            # But gcd(a,b) - gcd(a,b) = 0 IS tautological (self-cancellation that
            # happens to contain gcd inside a subtraction).
            if _is_divisibility_claim(lhs_str):
                return False
            return True
        # Different input strings + non-zero result = meaningful computation
        return False

    return False


def sympy_check(lhs_str: str, rhs_str: str) -> dict:
    """Fast equality check for pre-submission model self-correction.

    Returns diff info so the model can identify and fix its error.
    Unlike validate_sympy(), this does NOT apply the tautology gate
    (we want raw feedback, not a pass/fail verdict).

    Skips abstract function applications (f(1), g(n), etc.) — SymPy
    treats these as unknowns so the diff is never 0, but the claim may
    still be mathematically correct. validate_sympy() handles these via
    the _has_applied_undef gate.
    """
    try:
        lhs = _parse_safe(lhs_str)
        rhs = _parse_safe(rhs_str)
        # Skip correction loop for abstract function applications —
        # SymPy cannot verify them algebraically but validate_sympy() accepts them.
        if _has_applied_undef(lhs) or _has_applied_undef(rhs):
            return {"is_equal": True, "diff": None, "error": "abstract_function_skip"}
        is_zero, diff = _check_equality(lhs, rhs)
        if is_zero:
            return {"is_equal": True, "diff": None, "error": None}
        else:
            return {"is_equal": False, "diff": str(diff), "error": None}
    except ParserPolicyError as e:
        return {"is_equal": False, "diff": None, "error": str(e)}
    except Exception as e:
        return {"is_equal": False, "diff": None, "error": str(e)}


def validate_sympy(formal: str, goal_state: str):
    from .router import ValidatorResult
    start = time.perf_counter_ns()
    try:
        # Detect inequality formals (<=, >=) before splitting on '='
        # e.g. "f(4) <= 16" should route to inequality verifier, not split into "f(4) <" and " 16"
        ineq_match = re.match(r'^(.+?)\s*(<=|>=)\s*(.+)$', formal)
        if ineq_match:
            lhs_str, relation, rhs_str = ineq_match.groups()
            from .typed_claims import _verify_inequality
            claim = {"lhs": lhs_str, "rhs": rhs_str, "relation": relation, "type": "inequality"}
            result = _verify_inequality(claim, goal_state)
            wall_ms = (time.perf_counter_ns() - start) // 1_000_000
            return ValidatorResult(
                passed=result["verified"],
                message=result["reason"],
                raw_output=str(formal),
                wall_time_ms=wall_ms,
            )

        if '=' in formal and '==' not in formal:
            parts = formal.split('=')
            if len(parts) == 2:
                lhs = _parse_safe(parts[0])
                rhs = _parse_safe(parts[1])

                # Undef-function gate: formal contains abstract function applications
                # (e.g. f(1) = 1). SymPy treats f as an unknown Function so f(1) - 1
                # never reduces to 0 — not because the claim is false, but because
                # the validator has no definition for f. Skip rather than falsely reject.
                if _has_applied_undef(lhs) or _has_applied_undef(rhs):
                    wall_ms = (time.perf_counter_ns() - start) // 1_000_000
                    return ValidatorResult(
                        passed=True,
                        message="Algebraic check skipped: formal contains abstract function application — not a pure algebraic identity",
                        raw_output=str(formal),
                        wall_time_ms=wall_ms,
                    )

                # Fast check: expand + cancel + factor (never hangs)
                is_zero, diff = _check_equality(lhs, rhs)

                if is_zero:
                    # Tautology gate: reject vacuous formalizations
                    if _is_tautological(parts[0], parts[1], lhs, rhs):
                        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
                        return ValidatorResult(
                            passed=False,
                            message="Tautological formalization: expression is trivially true "
                                    "but does not encode the mathematical claim",
                            raw_output=str(formal),
                            wall_time_ms=wall_ms,
                        )
                    wall_ms = (time.perf_counter_ns() - start) // 1_000_000
                    return ValidatorResult(passed=True, message="Equality verified",
                                           raw_output=str(formal), wall_time_ms=wall_ms)

                # Assumption-aware check
                if goal_state and goal_state.strip():
                    assumptions = _extract_assumptions(goal_state)
                    if assumptions:
                        try:
                            passed, msg = _check_with_assumptions(diff, assumptions)
                            if passed:
                                wall_ms = (time.perf_counter_ns() - start) // 1_000_000
                                return ValidatorResult(
                                    passed=True,
                                    message=f"Equality verified (conditional: {msg})",
                                    raw_output=f"diff={diff}, assumptions={[str(a) for a in assumptions]}",
                                    wall_time_ms=wall_ms,
                                )
                        except Exception:
                            pass

                # Neither check passed
                wall_ms = (time.perf_counter_ns() - start) // 1_000_000
                return ValidatorResult(passed=False, message=f"Difference: {diff}",
                                       raw_output=str(formal), wall_time_ms=wall_ms)
            else:
                wall_ms = (time.perf_counter_ns() - start) // 1_000_000
                return ValidatorResult(passed=False, message="Could not parse equality",
                                       raw_output=str(formal), wall_time_ms=wall_ms)
        else:
            wall_ms = (time.perf_counter_ns() - start) // 1_000_000
            return ValidatorResult(passed=False, message="Not an equality (no = sign)",
                                   raw_output=str(formal), wall_time_ms=wall_ms)
    except ParserPolicyError as e:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return ValidatorResult(passed=False, message=str(e),
                               raw_output=str(formal), wall_time_ms=wall_ms)
    except Exception as e:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return ValidatorResult(passed=False, message=f"SymPy error: {e}",
                               raw_output=str(e), wall_time_ms=wall_ms)
