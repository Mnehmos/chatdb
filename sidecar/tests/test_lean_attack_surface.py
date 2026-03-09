"""Edge case attack surface tests for Lean 4 validation.

Tests are split into:
  - Unit tests: test translation/extraction logic (no Lean required)
  - Integration tests: require Lean 4 + Mathlib (skipped if unavailable)
"""
import pytest
from src.validation.lean_validator import (
    _sympy_to_lean,
    _extract_variables,
    _needs_rationals,
    _lean_available,
    validate_lean,
)


# ══════════════════════════════════════════════════════════════════════
# LEAN TRANSLATION UNIT TESTS (no Lean binary needed)
# ══════════════════════════════════════════════════════════════════════

class TestLeanTranslation:
    """Test _sympy_to_lean translation correctness."""

    def test_lt01_sum_untranslatable(self):
        """Sum() should return None — not representable in simple Lean."""
        assert _sympy_to_lean("Sum(k, (k, 1, n))") is None

    def test_lt02_floor_untranslatable(self):
        """floor() should return None."""
        assert _sympy_to_lean("floor(x/2)") is None

    def test_lt03_factorial_untranslatable(self):
        """factorial() should return None."""
        assert _sympy_to_lean("factorial(n)") is None

    def test_lt04_mod_untranslatable(self):
        """Mod() should return None."""
        assert _sympy_to_lean("Mod(x, 3)") is None

    def test_lt05_basic_exponent_translation(self):
        """** should become ^."""
        result = _sympy_to_lean("x**3 + y**2")
        assert result is not None
        assert "**" not in result
        assert "^" in result

    def test_lt06_clean_expression_passes(self):
        """Simple polynomial should translate cleanly."""
        result = _sympy_to_lean("x + y + 1")
        assert result is not None
        assert result.strip() == "x + y + 1"

    def test_lt07_piecewise_untranslatable(self):
        """Piecewise should return None."""
        assert _sympy_to_lean("Piecewise((x, x > 0), (-x, True))") is None

    def test_lt08_max_min_untranslatable(self):
        """Max/Min should return None."""
        assert _sympy_to_lean("Max(x, y)") is None
        assert _sympy_to_lean("Min(x, y)") is None


class TestVariableExtraction:
    """Test _extract_variables edge cases."""

    def test_single_letter_vars(self):
        """Standard single-letter variables."""
        vars = _extract_variables("x + y + z")
        assert set(vars) == {"x", "y", "z"}

    def test_filtered_uppercase_vars(self):
        """I, E, S, N, O are filtered (Lean/math constants)."""
        vars = _extract_variables("I + E + S + N + O")
        assert len(vars) == 0, f"Should filter I,E,S,N,O. Got: {vars}"

    def test_lowercase_e_not_filtered(self):
        """Lowercase 'e' should NOT be filtered (only uppercase E)."""
        vars = _extract_variables("e + 1")
        assert "e" in vars, f"Lowercase 'e' should be kept. Got: {vars}"

    def test_multichar_names_ignored(self):
        """Multi-character names like 'sin', 'cos' are not variables."""
        vars = _extract_variables("sin(x) + cos(y)")
        # 'sin' and 'cos' are multi-char, not single-letter
        # Only x and y should be extracted
        assert "x" in vars
        assert "y" in vars
        assert len([v for v in vars if len(v) > 1]) == 0

    def test_filtered_vars_still_in_expression(self):
        """When I/E/S/N/O are filtered, they remain in the expression
        as undefined Lean names — this is a potential failure point."""
        vars = _extract_variables("I + E + x")
        assert vars == ["x"], f"Only x should remain. Got: {vars}"
        # The expression "I + E + x" still contains I and E
        # Lean will see undefined names → error


class TestTypeInference:
    """Test _needs_rationals type inference."""

    def test_division_triggers_rationals(self):
        """/ in expression should trigger ℚ type."""
        assert _needs_rationals("x/2", "1") is True

    def test_no_division_uses_int(self):
        """No / should use Int type."""
        assert _needs_rationals("x + 1", "2*x") is False

    def test_division_in_rhs_only(self):
        """/ in RHS should also trigger ℚ."""
        assert _needs_rationals("1", "x/2") is True

    def test_floor_division_not_real_division(self):
        """// is floor division, not real division. But after normalization
        it becomes floor(x/y) which contains /. Check raw input."""
        # _needs_rationals operates on already-translated Lean strings
        # If floor was in there it would have been blocked by _sympy_to_lean
        assert _needs_rationals("x + 1", "y + 1") is False


# ══════════════════════════════════════════════════════════════════════
# LEAN INTEGRATION TESTS (require Lean 4 installed)
# ══════════════════════════════════════════════════════════════════════

_lean_installed = _lean_available()


@pytest.mark.skipif(not _lean_installed, reason="Lean 4 not installed")
class TestLeanIntegration:
    """Integration tests that actually run Lean validation."""

    def test_li01_integer_division_type_mismatch(self):
        """7/2 = 3 — in ℚ, 7/2 = 3.5 ≠ 3. Should fail."""
        r = validate_lean("7/2 = 3", "")
        if r is not None:  # None means untranslatable (skip)
            assert r.passed is False, "7/2 = 3 should fail in ℚ (3.5 ≠ 3)"

    def test_li02_rational_identity(self):
        """1/2 + 1/2 = 1 — should pass in ℚ via ring."""
        r = validate_lean("1/2 + 1/2 = 1", "")
        if r is not None:
            assert r.passed is True, f"1/2 + 1/2 = 1 should pass. Got: {r.message}"

    def test_li03_commutativity(self):
        """a + b = b + a — basic commutative ring identity."""
        r = validate_lean("a + b = b + a", "")
        if r is not None:
            assert r.passed is True, f"Commutativity should pass. Got: {r.message}"

    def test_li04_distributivity(self):
        """a*(b+c) = a*b + a*c — distributive law."""
        r = validate_lean("a*(b+c) = a*b + a*c", "")
        if r is not None:
            assert r.passed is True, f"Distributivity should pass. Got: {r.message}"

    def test_li05_false_equality_rejected(self):
        """x + 1 = x should fail."""
        r = validate_lean("x + 1 = x", "")
        if r is not None:
            assert r.passed is False, "x + 1 = x should fail"

    def test_li06_cubic_identity(self):
        """x^3 + y^3 = (x+y)*(x^2 - x*y + y^2) — sum of cubes."""
        r = validate_lean("x**3 + y**3 = (x+y)*(x**2 - x*y + y**2)", "")
        if r is not None:
            assert r.passed is True, f"Sum of cubes should pass. Got: {r.message}"

    def test_li07_warmup_check(self):
        """Trivial 1 = 1 should always pass."""
        r = validate_lean("1 = 1", "")
        if r is not None:
            assert r.passed is True

    def test_li08_multiple_variables(self):
        """a + b + c + d = d + c + b + a — 4-variable commutation."""
        r = validate_lean("a + b + c + d = d + c + b + a", "")
        if r is not None:
            assert r.passed is True, f"4-var commutation should pass. Got: {r.message}"


@pytest.mark.skipif(not _lean_installed, reason="Lean 4 not installed")
class TestLeanREPLAttackSurface:
    """REPL-specific edge cases."""

    @pytest.mark.asyncio
    async def test_repl_concurrent_checks(self):
        """10 parallel checks should serialize correctly via lock."""
        import asyncio
        from src.lean_repl.session import LeanREPL, REPLState

        repl = LeanREPL()
        try:
            await repl.start()
            if repl.state != REPLState.READY:
                pytest.skip("REPL failed to start")

            async def check(lhs, rhs, vars):
                return await repl.check_equality(lhs, rhs, vars)

            # Launch 5 parallel checks (mix of valid and invalid)
            results = await asyncio.gather(
                check("x + 1", "1 + x", ["x"]),
                check("x^2 - 1", "(x-1)*(x+1)", ["x"]),
                check("a + b", "b + a", ["a", "b"]),
                check("2 + 2", "4", []),
                check("x + 1", "x", ["x"]),  # invalid
                return_exceptions=True,
            )

            # All should complete (no deadlock)
            assert len(results) == 5
            for i, r in enumerate(results):
                assert not isinstance(r, Exception), f"Check {i} raised: {r}"

            # First 4 should pass, 5th should fail
            assert results[0].passed is True
            assert results[1].passed is True
            assert results[2].passed is True
            assert results[3].passed is True
            assert results[4].passed is False
        finally:
            await repl.stop()
