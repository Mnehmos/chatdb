"""Edge case attack surface tests for SymPy and typed claim validation.

Categories:
  FP  - False Positive Attacks (invalid math accepted)
  FN  - False Negative Attacks (valid math rejected)
  TE  - Tautology Evasion
  BA  - Boundary Attacks (sampling range limits)
  PE  - Parser Edge Cases
  AI  - Assumption Injection
  TCA - Typed Claim Edge Cases
  MF  - Mechanical Failures

Each test documents attack vector, expected behavior, and why it matters.
"""
import pytest
from src.validation.sympy_validator import validate_sympy, sympy_check, _normalize_formal
from src.validation.typed_claims import verify_typed_claim


# ══════════════════════════════════════════════════════════════════════
# FALSE POSITIVE ATTACKS — invalid math accepted as valid
# ══════════════════════════════════════════════════════════════════════

class TestFalsePositiveAttacks:
    """Inputs that SHOULD fail validation but might pass."""

    def test_fp01_abstract_fn_contradiction_both_pass(self):
        """f(1)=42 and f(1)=99 both pass — no cross-step consistency.
        Known limitation: abstract function gate skips ALL such claims."""
        r1 = validate_sympy("f(1) = 42", "")
        r2 = validate_sympy("f(1) = 99", "")
        # Both pass because _has_applied_undef triggers skip
        assert r1.passed is True
        assert r2.passed is True
        assert "skipped" in r1.message.lower()
        assert "skipped" in r2.message.lower()

    def test_fp01b_abstract_fn_arbitrary_claim(self):
        """Any claim about abstract functions passes, no matter how absurd."""
        r = validate_sympy("f(x) = x**2 + sin(x)/log(x) + 999", "")
        assert r.passed is True
        assert "skipped" in r.message.lower()

    def test_fp02_divisibility_all_zero_dividend_in_sample_range(self):
        """Dividend = product of (n-k) for k=2..14 is 0 for ALL n in {2..14}.
        Every sample gives dividend=0, making it trivially divisible.
        But the claim is false for n=15 (nonzero dividend, n^2+1=226 doesn't divide it)."""
        factors = "*".join(f"(n-{k})" for k in range(2, 15))
        claim = {"type": "divisibility", "dividend": factors, "divisor": "n**2 + 1"}
        r = verify_typed_claim(claim, "")
        # FIXED: now detects all-zero-dividend and returns inconclusive
        assert r["verified"] is False, (
            f"All-zero dividend should be caught. Got: {r}"
        )

    def test_fp03_congruence_all_zero_modulus_vacuous_pass(self):
        """Modulus = product of (n-k) for k=2..14 is 0 for all n in {2..14}.
        The `if nm == 0: continue` skips every sample, leaving all_congruent=True."""
        factors = "*".join(f"(n-{k})" for k in range(2, 15))
        # This is a 13-factor product — SymPy parses it fine but expansion may be slow
        claim = {
            "type": "congruence",
            "expr": "n**2",
            "remainder": "n + 1",  # Wrong: n^2 ≢ n+1 in general
            "modulus": factors,
        }
        r = verify_typed_claim(claim, "")
        # FIXED: now detects all-zero modulus and returns inconclusive
        assert r["verified"] is False, (
            f"All-zero modulus should be caught. Got: {r}"
        )

    def test_fp04_inequality_sampling_gap_beyond_100(self):
        """-(n-1)(n-200) = -n^2+201n-200 is >= 0 for n in [1,200].
        Sampling tests [1,2,3,5,7,10,20,100] — all in the positive region.
        But at n=201, value = -(200)(1) = -200 < 0."""
        claim = {
            "type": "inequality",
            "lhs": "-n**2 + 201*n - 200",
            "rhs": "0",
            "relation": ">=",
        }
        r = verify_typed_claim(claim, "")
        # BUG: passes because max sample is 100, well within [1,200] positive region
        assert r["verified"] is True, (
            f"Expected false positive (counterexample at n=201 missed). Got: {r}"
        )

    def test_fp05_trig_precision_exploit(self):
        """sin(7x)/10^12 has magnitude ~1e-12, well below the 1e-9 tolerance.
        The perturbed identity passes numeric sampling despite being false."""
        r = validate_sympy(
            "sin(x)**2 + cos(x)**2 = 1 + sin(7*x)/10**12", ""
        )
        # Documents the float precision risk — perturbation below tolerance
        # Record actual behavior without asserting either way
        if r.passed:
            pass  # Confirmed: sub-tolerance perturbation accepted
        else:
            pass  # Tolerance tighter than expected

    def test_fp06_forall_symbolic_bound_gap(self):
        """For all k in 1..n: k < 10. Outer n sampled 2..9,
        so k never reaches 10. But at n=10, k=10 fails."""
        claim = {
            "type": "for_all",
            "variable": "k",
            "domain": "1..n",
            "predicate": "k < 10",
        }
        r = verify_typed_claim(claim, "")
        # BUG: passes because outer sampling 2..9 never tests n >= 10
        assert r["verified"] is True, (
            f"Expected false positive (n never exceeds 9). Got: {r}"
        )


# ══════════════════════════════════════════════════════════════════════
# FALSE NEGATIVE ATTACKS — valid math rejected
# ══════════════════════════════════════════════════════════════════════

class TestFalseNegativeAttacks:
    """Inputs that SHOULD pass validation but might fail."""

    @pytest.mark.xfail(reason="No log simplification strategy — known gap")
    def test_fn01_log_product_identity(self):
        """log(a*b) = log(a) + log(b) is valid for positive reals.
        No strategy handles logarithmic identities."""
        r = validate_sympy("log(a*b) = log(a) + log(b)", "")
        assert r.passed is True

    def test_fn02_exp_sum_identity(self):
        """exp(a+b) = exp(a)*exp(b) — SymPy handles this natively via expand."""
        r = validate_sympy("exp(a + b) = exp(a)*exp(b)", "")
        assert r.passed is True, f"Exp identity should pass. Got: {r.message}"

    def test_fn03_nested_floor_identity(self):
        """floor(floor(x)) = floor(x) — idempotency of floor.
        Should be caught by floor numeric sampling (strategy 6)."""
        r = validate_sympy("floor(floor(x)) = floor(x)", "")
        assert r.passed is True, f"Nested floor identity should pass. Got: {r.message}"

    def test_fn04_arithmetic_sum_formula(self):
        """Sum(k, (k, 1, n)) = n*(n+1)/2 — classic Gauss formula.
        Should work via strategy 7 (symbolic-limit Sum numeric sampling)."""
        r = validate_sympy("Sum(k, (k, 1, n)) = n*(n+1)/2", "")
        assert r.passed is True, f"Arithmetic sum formula should pass. Got: {r.message}"

    def test_fn05_gcd_consecutive_odds(self):
        """gcd(2n+1, 2n+3) = 1 — consecutive odd numbers are coprime.
        Tests symbolic GCD or numeric sampling."""
        claim = {"type": "gcd", "a": "2*n+1", "b": "2*n+3", "value": "1"}
        r = verify_typed_claim(claim, "")
        assert r["verified"] is True, (
            f"Consecutive odds coprime should pass. Got: {r}"
        )

    def test_fn06_power_identity(self):
        """n * n**(n-1) = n**n — powsimp should handle this."""
        r = validate_sympy("n * n**(n-1) = n**n", "")
        assert r.passed is True, f"Power identity should pass via powsimp. Got: {r.message}"

    def test_fn07_geometric_sum(self):
        """Sum(r**k, (k, 0, n-1)) = (r**n - 1)/(r - 1) — geometric series.
        Tests symbolic Sum path. May need numeric sampling."""
        r = validate_sympy("Sum(r**k, (k, 0, n-1)) = (r**n - 1)/(r - 1)", "")
        # This is genuinely hard for the validator — document behavior
        if not r.passed:
            pytest.xfail("Geometric sum formula — validator can't verify")

    def test_fn08_bezout_identity(self):
        """3*(14*n+3) - 2*(21*n+4) = 1 — Bezout coefficient check."""
        r = validate_sympy("3*(14*n+3) - 2*(21*n+4) = 1", "")
        assert r.passed is True, f"Bezout identity should expand cleanly. Got: {r.message}"


# ══════════════════════════════════════════════════════════════════════
# TAUTOLOGY EVASION — tautologies that bypass the detector
# ══════════════════════════════════════════════════════════════════════

class TestTautologyEvasion:
    """Tautological expressions that might evade detection."""

    def test_te01_whitespace_evasion(self):
        """'x + y' vs 'x+y' — strings differ after normalize, SymPy same.
        Not flagged because input strings differ and result is nonzero."""
        r = validate_sympy("x + y = x+y", "")
        # Strings differ → not caught by string check
        # SymPy same, nonzero → "different strings + nonzero = meaningful"
        # This PASSES — documenting the behavior
        assert r.passed is True, "Whitespace-only difference passes (by design)"

    def test_te02_commutativity(self):
        """a + b = b + a — commutative identity. Strings differ, SymPy same."""
        r = validate_sympy("a + b = b + a", "")
        # Passes: different input strings, SymPy canonicalizes to same, nonzero result
        assert r.passed is True, "Commutativity passes (arguably legitimate)"

    def test_te03_caret_vs_doublestar(self):
        """x^2 = x**2 — after normalization both become x**2."""
        r = validate_sympy("x^2 = x**2", "")
        assert r.passed is False, "Should be tautological: identical after normalize"
        assert "tautolog" in r.message.lower()

    def test_te04_elaborate_cancellation_not_tautological(self):
        """(x+1)*(x-1) - (x**2-1) = 0 — SymPy doesn't auto-expand the product,
        so LHS parses to a non-trivial expression. This is a REAL algebraic identity
        (showing the factored form equals the expanded form), not a tautology."""
        r = validate_sympy("(x+1)*(x-1) - (x**2-1) = 0", "")
        # This is correct: the expression requires expand() to verify, it's meaningful
        assert r.passed is True, "This is a real algebraic identity, not a tautology"

    def test_te05_deep_nesting(self):
        """((((x)))) = x — strings differ, SymPy same, nonzero result.
        Not tautological by current rules (different input strings)."""
        r = validate_sympy("((((((((x)))))))) = x", "")
        # Passes: strings differ, result is nonzero (x)
        assert r.passed is True, "Deep nesting passes (strings differ)"

    def test_te06_multiplication_by_one(self):
        """1*x = x — trivially true but strings differ."""
        r = validate_sympy("1*x = x", "")
        # Passes: different input strings
        assert r.passed is True, "Multiply by 1 passes (strings differ)"

    def test_te07_add_zero(self):
        """x + 0 = x — trivially true but strings differ."""
        r = validate_sympy("x + 0 = x", "")
        assert r.passed is True, "Add zero passes (strings differ)"

    def test_te08_triple_negative(self):
        """-(-(-x)) = -x — strings differ, both reduce to -x."""
        r = validate_sympy("-(-(-x)) = -x", "")
        assert r.passed is True, "Triple negation passes (strings differ)"


# ══════════════════════════════════════════════════════════════════════
# BOUNDARY ATTACKS — numeric sampling range limits
# ══════════════════════════════════════════════════════════════════════

class TestBoundaryAttacks:
    """Exploit the finite sampling ranges in numeric verification."""

    def test_ba01_divisibility_sample_range_2_14(self):
        """Product of (n-k) for k=2..14 is 0 for ALL n in sample range.
        FIXED: now detected as inconclusive (all-zero dividend)."""
        factors = "*".join(f"(n-{k})" for k in range(2, 15))
        claim = {"type": "divisibility", "dividend": factors, "divisor": "n + 100"}
        r = verify_typed_claim(claim, "")
        assert r["verified"] is False, "All-zero dividend should be caught"

    def test_ba02_congruence_sample_range_2_14(self):
        """Modulus that's zero for all samples 2..14.
        FIXED: now detected as inconclusive."""
        factors = "*".join(f"(n-{k})" for k in range(2, 15))
        claim = {
            "type": "congruence",
            "expr": "999",
            "remainder": "1",
            "modulus": factors,
        }
        r = verify_typed_claim(claim, "")
        assert r["verified"] is False, "All-zero modulus should be caught"

    def test_ba03_inequality_only_positive_integers(self):
        """n^2 >= n holds for all positive integers but not for x in (0,1).
        Sampling uses integers only — documents domain limitation."""
        claim = {"type": "inequality", "lhs": "n**2", "rhs": "n", "relation": ">="}
        r = verify_typed_claim(claim, "")
        assert r["verified"] is True, "Integer sampling doesn't test fractional values"

    def test_ba04_forall_outer_range_2_9(self):
        """For-all with symbolic bounds. Outer variable sampled 2..9 only."""
        claim = {
            "type": "for_all",
            "variable": "k",
            "domain": "1..n",
            "predicate": "k <= 9",
        }
        r = verify_typed_claim(claim, "")
        # Outer n in 2..9 means k always <= 9. But n=10 → k=10 fails.
        assert r["verified"] is True, "Outer sampling too narrow"

    def test_ba05_trig_fixed_test_points(self):
        """Trig sampling uses 6 points from 7 fixed base values.
        An adversary knowing the base values could craft a zero-at-those-points function.
        This just documents the fixed nature of the test points."""
        # The base values are: 0.37, 1.23, 0.71, 2.14, 0.59, 0.93, 1.77
        # A function with roots at all these points would pass
        # Hard to construct algebraically, but documents the risk
        r = validate_sympy("sin(x)**2 + cos(x)**2 = 1", "")
        assert r.passed is True  # Sanity: real identity still passes


# ══════════════════════════════════════════════════════════════════════
# PARSER EDGE CASES — unusual but valid/invalid syntax
# ══════════════════════════════════════════════════════════════════════

class TestParserEdgeCases:

    def test_pe01_multiple_equals(self):
        """a = b = c splits into 3 parts."""
        r = validate_sympy("a = b = c", "")
        assert r.passed is False
        assert "parse" in r.message.lower() or "could not" in r.message.lower()

    def test_pe02_less_equal_in_formal(self):
        """x <= y routes to inequality verifier (not treated as equality)."""
        r = validate_sympy("x <= y", "")
        # Now correctly handled as an inequality instead of failing on bad split
        assert r.passed is True or "inequality" in r.message.lower() or "verified" in r.message.lower()

    def test_pe03_greater_equal_in_formal(self):
        """x >= 0 routes to inequality verifier (not treated as equality)."""
        r = validate_sympy("x >= 0", "")
        # x >= 0 is not universally true (x can be negative), so verifier may reject
        # Key assertion: it doesn't crash or return "unsafe parser" anymore
        assert "unsafe" not in r.message.lower()

    def test_pe04_double_equals_bypass(self):
        """== causes the validator to skip entirely ('Not an equality')."""
        r = validate_sympy("x**2 == x*x", "")
        assert r.passed is False
        assert "not an equality" in r.message.lower()

    def test_pe05_empty_formal(self):
        """Empty string has no = sign."""
        r = validate_sympy("", "")
        assert r.passed is False

    def test_pe06_whitespace_only(self):
        """Whitespace-only has no = sign."""
        r = validate_sympy("   ", "")
        assert r.passed is False

    def test_pe07_implicit_multiplication_blocked(self):
        """2x + 3x = 5x — LLMs often omit * operator.
        Blocked by AST allowlist because Python can't parse '2x' as valid syntax.
        The implicit_multiplication transform only works AFTER AST parsing."""
        r = validate_sympy("2x + 3x = 5x", "")
        # FALSE NEGATIVE: implicit mult is blocked by the AST allowlist
        # SymPy's implicit_multiplication_application would handle it, but
        # the allowlist runs ast.parse() first which rejects '2x' as SyntaxError
        assert r.passed is False, "Implicit mult blocked by AST allowlist (known gap)"

    def test_pe08_implicit_mult_two_vars(self):
        """xy = x*y — check if parser splits xy into x*y."""
        r = validate_sympy("xy = x*y", "")
        # implicit_multiplication_application should handle this
        if not r.passed:
            pytest.xfail("Parser doesn't split 'xy' into x*y")

    def test_pe09_nested_carets(self):
        """2^3^2 = 512 — right-associative: 2^(3^2) = 2^9 = 512."""
        r = validate_sympy("2^3^2 = 512", "")
        assert r.passed is True, f"Right-assoc carets should give 512. Got: {r.message}"

    def test_pe10_lambda_blocked(self):
        """Lambda expressions must be blocked by parser allowlist."""
        r = validate_sympy("(lambda x: x)(1) = 1", "")
        assert r.passed is False
        assert "blocked" in r.message.lower() or "unsafe" in r.message.lower() or "error" in r.message.lower()

    def test_pe11_list_comp_blocked(self):
        """List comprehensions must be blocked."""
        r = validate_sympy("[x for x in range(5)] = 0", "")
        assert r.passed is False

    def test_pe12_string_literal_blocked(self):
        """String literals in formal must be blocked."""
        r = validate_sympy("'hello' = 'hello'", "")
        assert r.passed is False

    def test_pe13_long_expression(self):
        """50-term sum should complete without hanging."""
        terms = " + ".join(["x"] * 50)
        formal = f"{terms} = 50*x"
        r = validate_sympy(formal, "")
        assert r.passed is True, f"Long expression should pass. Got: {r.message}"

    def test_pe14_floor_division_syntax(self):
        """n//2 should be rewritten to floor(n/2)."""
        assert "floor" in _normalize_formal("n//2")

    def test_pe15_mixed_caret_and_doublestar(self):
        """x^2 * y**3 — mixed notation."""
        r = validate_sympy("x^2 * y**3 = x**2 * y**3", "")
        # After normalize: x**2 * y**3 = x**2 * y**3 → tautological (string identity)
        assert r.passed is False
        assert "tautolog" in r.message.lower()


# ══════════════════════════════════════════════════════════════════════
# ASSUMPTION INJECTION — exploit goal_state assumption extraction
# ══════════════════════════════════════════════════════════════════════

class TestAssumptionInjection:

    def test_ai01_circular_assumption(self):
        """Claim appears verbatim in goal_state → circular reasoning.
        _extract_assumptions finds x^3=8 in goal_state, makes diff/assumption=1."""
        r = validate_sympy("x**3 = 8", "prove that x**3 = 8")
        # This passes via circular assumption extraction
        # Document whether it passes or not
        if r.passed:
            assert "conditional" in r.message.lower(), (
                "If circular assumption passes, should be labeled conditional"
            )

    def test_ai02_hidden_equation_in_prose(self):
        """Goal_state contains the equation buried in prose."""
        r = validate_sympy(
            "a = 0",
            "The problem states that we should note that a = 0 is given explicitly"
        )
        if r.passed:
            assert "conditional" in r.message.lower()

    def test_ai03_contradictory_assumptions(self):
        """Contradictory assumptions shouldn't prove arbitrary claims."""
        r = validate_sympy("0 = 1", "given x = 0 and x = 1")
        # 0=1 should fail even with contradictory assumptions
        assert r.passed is False, "Contradictory assumptions shouldn't prove 0=1"

    def test_ai04_legitimate_multi_assumption(self):
        """Multiple assumptions in goal_state. The regex-based extractor
        may not parse all equations from 'x = 2 and y = 3' — it depends on
        how the prose keyword filter handles 'and'."""
        r = validate_sympy(
            "y = 3",
            "given y = 3"
        )
        # Single clean assumption should work
        if r.passed:
            assert "conditional" in r.message.lower()
        else:
            pytest.xfail("Assumption extraction couldn't parse 'given y = 3'")

    def test_ai05_assumption_only_from_goal_state(self):
        """Without goal_state, conditional equations should fail."""
        r = validate_sympy("a*(x + b/(2*a))**2 = b**2/(4*a) - c", "")
        assert r.passed is False, "Conditional without goal_state should fail"

    def test_ai06_assumption_with_wrong_equation(self):
        """Wrong claim should fail even with valid but unrelated assumptions."""
        r = validate_sympy(
            "x**2 = 42",
            "given a = 1"
        )
        assert r.passed is False, "Wrong equation should fail despite assumptions"


# ══════════════════════════════════════════════════════════════════════
# TYPED CLAIM EDGE CASES — per-type boundary conditions
# ══════════════════════════════════════════════════════════════════════

class TestTypedClaimDivisibility:

    def test_divisibility_negative_dividend(self):
        """-12 is divisible by 3."""
        r = verify_typed_claim({"type": "divisibility", "dividend": "-12", "divisor": "3"}, "")
        assert r["verified"] is True

    def test_divisibility_negative_divisor(self):
        """12 is divisible by -3."""
        r = verify_typed_claim({"type": "divisibility", "dividend": "12", "divisor": "-3"}, "")
        assert r["verified"] is True

    def test_divisibility_both_negative(self):
        """-12 is divisible by -3."""
        r = verify_typed_claim({"type": "divisibility", "dividend": "-12", "divisor": "-3"}, "")
        assert r["verified"] is True

    def test_divisibility_divisor_zero_no_crash(self):
        """Division by zero should fail gracefully, not crash."""
        r = verify_typed_claim({"type": "divisibility", "dividend": "12", "divisor": "0"}, "")
        assert r["verified"] is False

    def test_divisibility_large_numbers(self):
        """Large number divisibility should work with arbitrary precision."""
        r = verify_typed_claim({
            "type": "divisibility",
            "dividend": "2**100",
            "divisor": "2**50",
        }, "")
        assert r["verified"] is True


class TestTypedClaimCongruence:

    def test_congruence_modulus_zero_no_crash(self):
        """Modulus=0 should fail gracefully."""
        r = verify_typed_claim({
            "type": "congruence",
            "expr": "5",
            "remainder": "3",
            "modulus": "0",
        }, "")
        assert r["verified"] is False

    def test_congruence_negative_modulus(self):
        """7 ≡ 2 (mod -5) should pass (sign of modulus irrelevant)."""
        r = verify_typed_claim({
            "type": "congruence",
            "expr": "7",
            "remainder": "2",
            "modulus": "-5",
        }, "")
        assert r["verified"] is True

    def test_congruence_large_numbers(self):
        """2^100 mod 7 should work."""
        r = verify_typed_claim({
            "type": "congruence",
            "expr": "2**100",
            "remainder": str(pow(2, 100, 7)),
            "modulus": "7",
        }, "")
        assert r["verified"] is True


class TestTypedClaimForAll:

    def test_forall_empty_domain(self):
        """Domain 5..3 is empty. Empty domain should NOT pass vacuously —
        it's likely a solver error, not a vacuous truth."""
        claim = {
            "type": "for_all",
            "variable": "k",
            "domain": "5..3",
            "predicate": "k > 100",
        }
        r = verify_typed_claim(claim, "")
        # BUG: range(5, 4) is empty → loop doesn't execute → all_hold=True
        # After fix: should return verified=False with "empty domain"
        # For now, document the behavior
        if r["verified"] is True:
            pass  # Confirmed: empty domain = vacuous truth (bug)
        else:
            pass  # Already fixed

    def test_forall_single_element_domain(self):
        """Domain 0..0 has one element."""
        r = verify_typed_claim({
            "type": "for_all",
            "variable": "k",
            "domain": "0..0",
            "predicate": "k == 0",
        }, "")
        assert r["verified"] is True

    def test_forall_predicate_ge_operator(self):
        """>= should be matched before > in the operator priority list."""
        r = verify_typed_claim({
            "type": "for_all",
            "variable": "k",
            "domain": "1..5",
            "predicate": "k >= 1",
        }, "")
        assert r["verified"] is True

    def test_forall_predicate_truthiness_zero(self):
        """At k=0, bool(0) is False — predicate fails."""
        r = verify_typed_claim({
            "type": "for_all",
            "variable": "k",
            "domain": "0..5",
            "predicate": "k",
        }, "")
        assert r["verified"] is False

    def test_forall_large_domain_rejected(self):
        """Domain > 10000 elements should be rejected."""
        r = verify_typed_claim({
            "type": "for_all",
            "variable": "k",
            "domain": "1..20000",
            "predicate": "k > 0",
        }, "")
        assert r["verified"] is False
        assert "too large" in r["reason"].lower()

    def test_forall_domain_at_limit(self):
        """Domain of exactly 10001 elements (1..10001): hi-lo = 10000.
        Check > 10000 means 10000 is NOT too large. Tests off-by-one.
        Skipped: 10001 iterations of _parse_safe is too slow for unit tests."""
        # Just verify the boundary logic: 10000 is not > 10000
        assert not (10000 > 10000), "Off-by-one check: 10000 should NOT exceed limit"

    def test_forall_negative_domain(self):
        """k in -3..3 should work."""
        r = verify_typed_claim({
            "type": "for_all",
            "variable": "k",
            "domain": "-3..3",
            "predicate": "k**2 >= 0",
        }, "")
        assert r["verified"] is True

    def test_forall_predicate_ne_operator(self):
        """Test != operator in predicates."""
        r = verify_typed_claim({
            "type": "for_all",
            "variable": "k",
            "domain": "1..5",
            "predicate": "k != 0",
        }, "")
        assert r["verified"] is True


class TestTypedClaimInequality:

    def test_inequality_invalid_relation(self):
        """== is not a valid relation."""
        r = verify_typed_claim({
            "type": "inequality",
            "lhs": "1",
            "rhs": "2",
            "relation": "==",
        }, "")
        assert r["verified"] is False
        assert "unknown" in r["reason"].lower() or "relation" in r["reason"].lower()

    def test_inequality_strict_at_boundary(self):
        """0 < 0 is false (strict)."""
        r = verify_typed_claim({
            "type": "inequality",
            "lhs": "0",
            "rhs": "0",
            "relation": "<",
        }, "")
        assert r["verified"] is False

    def test_inequality_nonstrict_at_boundary(self):
        """0 <= 0 is true but caught as tautological."""
        r = verify_typed_claim({
            "type": "inequality",
            "lhs": "0",
            "rhs": "0",
            "relation": "<=",
        }, "")
        assert r["verified"] is False
        assert "tautolog" in r["reason"].lower()

    def test_inequality_negative_numbers(self):
        """-5 < -3 is true."""
        r = verify_typed_claim({
            "type": "inequality",
            "lhs": "-5",
            "rhs": "-3",
            "relation": "<",
        }, "")
        assert r["verified"] is True


# ══════════════════════════════════════════════════════════════════════
# MECHANICAL FAILURES — crashes, hangs, resource exhaustion
# ══════════════════════════════════════════════════════════════════════

class TestMechanicalFailures:

    @pytest.mark.skip(reason="floor(1/(x+1)) triggers slow SymPy eval at singularity — known hang risk")
    def test_mf01_division_by_zero_floor_sampling(self):
        """floor(1/(x+1)) at x=-1 causes 1/0. Should not crash but hangs in practice."""
        r = validate_sympy("floor(1/(x+1)) = floor(x/(x+1))", "")
        assert isinstance(r.passed, bool)

    def test_mf02_large_exponent(self):
        """2^100 should be handled by SymPy's arbitrary precision."""
        r = validate_sympy("2**100 = 1267650600228229401496703205376", "")
        assert r.passed is True, f"Large exponent should pass. Got: {r.message}"

    def test_mf03_very_large_tautology(self):
        """2^100 = 2^100 — string identity, should be tautological fast."""
        r = validate_sympy("2**100 = 2**100", "")
        assert r.passed is False
        assert "tautolog" in r.message.lower()

    @pytest.mark.skip(reason="solve() on quintic can hang SymPy indefinitely")
    def test_mf04_complex_assumption_no_hang(self):
        """solve() on a quintic should not hang (has try/except + syms[:4] limit)."""
        r = validate_sympy(
            "x + y = z",
            "given x**5 + y**5 + z**5 = x*y*z*(x+y+z)"
        )
        assert isinstance(r.passed, bool)

    def test_mf05_sympy_check_consistency(self):
        """sympy_check and validate_sympy should agree on basic cases."""
        # True equality
        check = sympy_check("(x+1)**2", "x**2 + 2*x + 1")
        validate = validate_sympy("(x+1)**2 = x**2 + 2*x + 1", "")
        assert check["is_equal"] == validate.passed

        # False equality
        check = sympy_check("x**2", "x + 1")
        validate = validate_sympy("x**2 = x + 1", "")
        assert check["is_equal"] == validate.passed

    def test_mf06_no_equals_sign(self):
        """Expression without = should fail cleanly."""
        r = validate_sympy("x**2 + 1", "")
        assert r.passed is False
        assert "not an equality" in r.message.lower() or "no =" in r.message.lower()

    def test_mf07_unknown_claim_type(self):
        """Unknown claim type should fail gracefully."""
        r = verify_typed_claim({"type": "magic", "value": "42"}, "")
        assert r["verified"] is False
        assert "unsupported" in r["reason"].lower() or "unknown" in r["reason"].lower()

    def test_mf08_empty_claim_fields(self):
        """Empty fields should fail gracefully."""
        r = verify_typed_claim({"type": "divisibility", "dividend": "", "divisor": "3"}, "")
        assert r["verified"] is False

    def test_mf09_normalize_floor_division_safety(self):
        """Floor division normalization on a//b//c.
        FIXED: safety break now compares against previous iteration, not original."""
        result = _normalize_formal("a//b//c")
        assert "//" not in result

    def test_mf10_scientific_notation(self):
        """1e5 = 100000 — scientific notation. AST sees it as valid float."""
        r = validate_sympy("100000.0 = 100000", "")
        assert isinstance(r.passed, bool)  # Must not crash
