"""Tests for ChatDB validators."""
from src.validation.sympy_validator import validate_sympy
from src.validation.pint_validator import validate_pint

def test_sympy_valid_equality():
    result = validate_sympy("x**2 - 1 = (x-1)*(x+1)", "")
    assert result.passed is True

def test_sympy_invalid():
    result = validate_sympy("x**2 = x + 1", "")
    assert result.passed is False

def test_pint_valid():
    result = validate_pint("9.8 * meter / second**2")
    assert result.passed is True


# --- Assumption-aware tests ---

def test_sympy_conditional_direct():
    """The exact case that caused 25 rejections: diff = a*x**2 + b*x + c."""
    formal = "a*(x + b/(2*a))**2 = b**2/(4*a) - c"
    goal = "PROBLEM: given a*x**2 + b*x + c = 0, prove x = (-b + sqrt(b**2 - 4*a*c))/(2*a)"
    result = validate_sympy(formal, goal)
    assert result.passed is True
    assert "conditional" in result.message.lower()


def test_sympy_conditional_scaled():
    """The case that caused 21 rejections: diff = (a*x**2 + b*x + c)/a."""
    formal = "(x + b/(2*a))**2 = (b**2 - 4*a*c)/(4*a**2)"
    goal = "PROBLEM: given a*x**2 + b*x + c = 0, prove x = (-b + sqrt(b**2 - 4*a*c))/(2*a)"
    result = validate_sympy(formal, goal)
    assert result.passed is True


def test_sympy_unconditional_preserved():
    """Unconditional identities still work without assumptions."""
    result = validate_sympy("(a+b)**2 = a**2 + 2*a*b + b**2", "")
    assert result.passed is True


def test_sympy_invalid_with_assumptions():
    """Wrong equations still fail even with assumptions present."""
    formal = "x**2 = 42"
    goal = "PROBLEM: given a*x**2 + b*x + c = 0"
    result = validate_sympy(formal, goal)
    assert result.passed is False


def test_sympy_caret_notation():
    """LLMs often use ^ instead of **."""
    result = validate_sympy("x^2 - 1 = (x-1)*(x+1)", "")
    assert result.passed is True


def test_sympy_empty_goal_state():
    """Without goal_state, conditional equations fail (no assumptions)."""
    formal = "a*(x + b/(2*a))**2 = b**2/(4*a) - c"
    result = validate_sympy(formal, "")
    assert result.passed is False


# --- Tautology detection tests ---

def test_tautology_identity():
    """Exact identity a**b = a**b (non-abstract) should be rejected as tautological."""
    r = validate_sympy("a**b = a**b", "")
    assert r.passed is False
    assert "tautolog" in r.message.lower()


def test_tautology_x_minus_x():
    """x - x = 0 is vacuously true (structure: X - X = 0)."""
    r = validate_sympy("x - x = 0", "")
    assert r.passed is False
    assert "tautolog" in r.message.lower()


def test_tautology_subtraction_pattern():
    """b - k**k - (b - k**k) = 0 is X - X = 0 pattern."""
    r = validate_sympy("b - k**k - (b - k**k) = 0", "")
    assert r.passed is False
    assert "tautolog" in r.message.lower()


def test_tautology_self_referential():
    """f_n**f_n - n**n = f_n**f_n - n**n is LHS == RHS literally."""
    r = validate_sympy("f_n**f_n - n**n = f_n**f_n - n**n", "")
    assert r.passed is False
    assert "tautolog" in r.message.lower()


def test_legitimate_algebra_not_flagged():
    """(x+1)**2 = x**2 + 2*x + 1 is a real algebraic identity, not tautological."""
    r = validate_sympy("(x+1)**2 = x**2 + 2*x + 1", "")
    assert r.passed is True


def test_trig_identity_not_tautological():
    """sin(x)**2 + cos(x)**2 = 1 should NOT be flagged as tautological.
    Note: SymPy can't verify this without simplify(), so it may fail verification,
    but it must not be rejected as tautological."""
    r = validate_sympy("sin(x)**2 + cos(x)**2 = 1", "")
    assert "tautolog" not in r.message.lower()


def test_numeric_equality_not_flagged():
    """4**3 = 64 is a real computation, not tautological."""
    r = validate_sympy("4**3 = 64", "")
    assert r.passed is True


def test_factoring_identity_not_flagged():
    """x**2 - 1 = (x-1)*(x+1) is real factoring."""
    r = validate_sympy("x**2 - 1 = (x-1)*(x+1)", "")
    assert r.passed is True


# --- Divisibility carve-out tests ---

def test_mod_zero_not_tautological():
    """Mod(b**a - b**a, a) = 0 asserts divisibility — not tautological."""
    r = validate_sympy("Mod(b**a - b**a, a) = 0", "")
    assert "tautolog" not in r.message.lower()


def test_gcd_self_cancel_still_tautological():
    """gcd(a, b) - gcd(a, b) = 0 is string-identity (X - X = 0), caught by check 1."""
    r = validate_sympy("gcd(a, b) - gcd(a, b) = 0", "")
    assert r.passed is False
    assert "tautolog" in r.message.lower()


def test_pure_cancel_still_tautological():
    """b**a - b**a = 0 with no Mod wrapper is still vacuous cancellation."""
    r = validate_sympy("b**a - b**a = 0", "")
    assert r.passed is False
    assert "tautolog" in r.message.lower()
