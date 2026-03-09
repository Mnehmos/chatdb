"""Tests for typed claim verifiers."""
import pytest
from src.validation.typed_claims import verify_typed_claim


# ── Divisibility ──────────────────────────────────────────────

def test_divisibility_valid():
    """3 divides 12 → verified."""
    claim = {"type": "divisibility", "dividend": "12", "divisor": "3"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is True

def test_divisibility_symbolic():
    """a divides a*k for symbolic k."""
    claim = {"type": "divisibility", "dividend": "a*k", "divisor": "a"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is True

def test_divisibility_invalid():
    """3 does not divide 7."""
    claim = {"type": "divisibility", "dividend": "7", "divisor": "3"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False

def test_divisibility_tautology():
    """dividend = 0 is trivially divisible by anything → tautology."""
    claim = {"type": "divisibility", "dividend": "0", "divisor": "x"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False
    assert "tautolog" in r["reason"].lower()


# ── Inequality ────────────────────────────────────────────────

def test_inequality_valid_le():
    """x**2 >= 0 for all x."""
    claim = {"type": "inequality", "lhs": "0", "rhs": "x**2", "relation": "<="}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is True

def test_inequality_valid_strict():
    """2 < 3."""
    claim = {"type": "inequality", "lhs": "2", "rhs": "3", "relation": "<"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is True

def test_inequality_invalid():
    """5 <= 3 is false."""
    claim = {"type": "inequality", "lhs": "5", "rhs": "3", "relation": "<="}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False

def test_inequality_tautology():
    """x <= x with non-strict is trivially true → tautology."""
    claim = {"type": "inequality", "lhs": "x", "rhs": "x", "relation": "<="}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False
    assert "tautolog" in r["reason"].lower()


# ── GCD ───────────────────────────────────────────────────────

def test_gcd_valid():
    """gcd(12, 8) = 4."""
    claim = {"type": "gcd", "a": "12", "b": "8", "value": "4"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is True

def test_gcd_invalid():
    """gcd(12, 8) != 2."""
    claim = {"type": "gcd", "a": "12", "b": "8", "value": "2"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False

def test_gcd_tautology():
    """gcd(a, a) = a is trivially true → tautology."""
    claim = {"type": "gcd", "a": "x", "b": "x", "value": "x"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False
    assert "tautolog" in r["reason"].lower()


# ── Congruence ────────────────────────────────────────────────

def test_congruence_valid():
    """2**10 = 1024 ≡ 2 (mod 7)."""
    claim = {"type": "congruence", "expr": "2**10", "remainder": "2", "modulus": "7"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is True

def test_congruence_invalid():
    """2**10 = 1024 ≢ 3 (mod 7)."""
    claim = {"type": "congruence", "expr": "2**10", "remainder": "3", "modulus": "7"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False

def test_congruence_tautology():
    """0 ≡ 0 (mod anything) is trivially true → tautology."""
    claim = {"type": "congruence", "expr": "0", "remainder": "0", "modulus": "7"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False
    assert "tautolog" in r["reason"].lower()


# ── For-all ───────────────────────────────────────────────────

def test_for_all_valid():
    """for all k in 1..5: k divides 120 (=5!)."""
    claim = {
        "type": "for_all",
        "variable": "k",
        "domain": "1..5",
        "predicate": "Mod(120, k) == 0",
    }
    r = verify_typed_claim(claim, "")
    assert r["verified"] is True

def test_for_all_invalid():
    """for all k in 1..5: k divides 7 → false (2 doesn't divide 7)."""
    claim = {
        "type": "for_all",
        "variable": "k",
        "domain": "1..5",
        "predicate": "Mod(7, k) == 0",
    }
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False


# ── Equality delegation ──────────────────────────────────────

def test_equality_delegates():
    """Equality type delegates to existing validate_sympy."""
    claim = {"type": "equality", "lhs": "x**2 - 1", "rhs": "(x-1)*(x+1)"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is True


# ── Unknown type ──────────────────────────────────────────────

def test_unknown_type():
    """Unknown claim type returns error, not crash."""
    claim = {"type": "banana", "lhs": "1", "rhs": "1"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False
    assert "unknown" in r["reason"].lower() or "unsupported" in r["reason"].lower()


# ── Abstract functions (f(1), g(n)) ──────────────────────────

def test_abstract_function_divisibility():
    """Divisibility with f(1) → skip, not reject."""
    claim = {"type": "divisibility", "dividend": "1 - f(1)", "divisor": "f(1)"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is True
    assert "skipped" in r["reason"].lower()

def test_abstract_function_inequality():
    """Inequality with f(n) → skip, not reject."""
    claim = {"type": "inequality", "lhs": "f(n)", "rhs": "n", "relation": "<="}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is True
    assert "skipped" in r["reason"].lower()


# ── Missing fields ────────────────────────────────────────────

def test_missing_fields():
    """Divisibility claim missing 'divisor' → graceful error."""
    claim = {"type": "divisibility", "dividend": "12"}
    r = verify_typed_claim(claim, "")
    assert r["verified"] is False
    assert "missing" in r["reason"].lower() or "required" in r["reason"].lower()
