"""Tests for Lean 4 validator."""
import pytest
from src.validation.lean_validator import (
    _extract_variables,
    _sympy_to_lean,
    _build_lean_file,
    _build_lean_file_multi,
    _lean_available,
    validate_lean,
)


# --- Unit tests (no Lean required) ---

def test_sympy_to_lean_power():
    assert _sympy_to_lean("x**2 - 1") == "x^2 - 1"

def test_sympy_to_lean_nested_power():
    assert _sympy_to_lean("a**3 + b**2") == "a^3 + b^2"

def test_sympy_to_lean_no_change():
    assert _sympy_to_lean("x + 1") == "x + 1"

def test_sympy_to_lean_strips_whitespace():
    assert _sympy_to_lean("  x**2  ") == "x^2"

def test_extract_variables_single():
    assert _extract_variables("x**2 - 1") == ["x"]

def test_extract_variables_multiple():
    assert _extract_variables("a*x**2 + b*x + c") == ["a", "b", "c", "x"]

def test_extract_variables_filters_constants():
    """Should exclude I, E, S, N, O (Lean/math constants)."""
    assert _extract_variables("E + x") == ["x"]
    assert _extract_variables("N + I") == []

def test_extract_variables_empty():
    assert _extract_variables("1 + 2") == []

def test_build_lean_file_with_vars():
    code = _build_lean_file("x ^ 2 - 1", "(x - 1) * (x + 1)", ["x"], "ring")
    assert "theorem step (x : Int)" in code
    assert ": x ^ 2 - 1 = (x - 1) * (x + 1)" in code
    assert ":= by ring" in code

def test_build_lean_file_no_vars():
    code = _build_lean_file("2 ^ 3", "8", [], "decide")
    assert "theorem step :" in code
    assert "(x : Int)" not in code

def test_build_lean_file_multiple_vars():
    code = _build_lean_file("a + b", "b + a", ["a", "b"], "ring")
    assert "(a : Int)" in code
    assert "(b : Int)" in code

def test_build_lean_file_multi_with_vars():
    code = _build_lean_file_multi("x^2 - 1", "(x-1)*(x+1)", ["x"])
    assert "theorem step (x : Int)" in code
    assert "first | ring | omega | norm_num | simp | decide" in code
    assert "import Mathlib.Tactic.Ring" in code
    assert "import Mathlib.Tactic.NormNum" in code

def test_build_lean_file_multi_no_vars():
    code = _build_lean_file_multi("2^10", "1024", [])
    assert "theorem step :" in code
    assert "first | ring | omega | norm_num | simp | decide" in code


# --- Graceful degradation ---

def test_validate_lean_no_equality():
    """No '=' in formal → immediate failure (no lean needed)."""
    result = validate_lean("x**2 + 1", "")
    assert result.passed is False
    assert "No equality" in result.message

def test_validate_lean_double_equals():
    """'==' not supported → immediate failure."""
    result = validate_lean("x**2 == 1", "")
    assert result.passed is False

def test_validate_lean_multiple_equals():
    """Multiple '=' not supported → immediate failure."""
    result = validate_lean("a = b = c", "")
    assert result.passed is False
    assert "Expected exactly one" in result.message

def test_validate_lean_empty_formal():
    result = validate_lean("", "")
    assert result.passed is False


# --- Integration tests (require Lean 4 installed) ---

needs_lean = pytest.mark.skipif(
    not _lean_available(),
    reason="Lean 4 not installed (install elan to enable)"
)

@needs_lean
def test_lean_simple_identity():
    """x^2 - 1 = (x-1)(x+1) should be verified."""
    result = validate_lean("x**2 - 1 = (x-1)*(x+1)", "")
    assert result.passed is True
    assert "Lean verified" in result.message
    assert result.wall_time_ms > 0

@needs_lean
def test_lean_invalid_equality():
    """x^2 - 1 = (x-1)*(x+2) should fail all tactics."""
    result = validate_lean("x**2 - 1 = (x-1)*(x+2)", "")
    assert result.passed is False

@needs_lean
def test_lean_numeric_equality():
    """2^10 = 1024 should be verified by decide or omega."""
    result = validate_lean("2**10 = 1024", "")
    assert result.passed is True

@needs_lean
def test_lean_commutative():
    """a + b = b + a should be verified by ring."""
    result = validate_lean("a + b = b + a", "")
    assert result.passed is True

@needs_lean
def test_lean_distributive():
    """a*(b+c) = a*b + a*c should be verified by ring."""
    result = validate_lean("a*(b+c) = a*b + a*c", "")
    assert result.passed is True

@needs_lean
def test_lean_caret_notation():
    """LLMs use ^ — SymPy uses **. Both should work."""
    result = validate_lean("x^2 - 1 = (x-1)*(x+1)", "")
    assert result.passed is True
