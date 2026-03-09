"""Security boundary tests for SymPy parser sandbox behavior."""


# TDD Enforcement Prompt Tag:
# No implementation before failing tests (Red)
# Minimal implementation to pass tests (Green)
# Refactor only after green tests (Blue)
# PRs without Red evidence are non-compliant

import pytest

from src.validation.sympy_validator import validate_sympy


def test_should_reject_untrusted_formal_expression_when_python_import_call_is_present():
    hostile = "__import__('os').system('echo CHATDB_UNSAFE_PARSER_EXECUTION')=0"

    first = validate_sympy(hostile, "")
    second = validate_sympy(hostile, "")

    assert first.passed is False, (
        "Untrusted parser input must be rejected deterministically. "
        f"Expected passed=False for hostile payload, got passed={first.passed}, "
        f"message={first.message!r}, raw_output={first.raw_output!r}."
    )
    assert second.passed is False, (
        "Repeated hostile parser input must still be rejected. "
        f"Expected passed=False, got passed={second.passed}, "
        f"message={second.message!r}, raw_output={second.raw_output!r}."
    )
    assert first.message == second.message, (
        "Hostile input rejection must be deterministic. "
        f"Expected same rejection message across repeated calls, got "
        f"first={first.message!r}, second={second.message!r}."
    )


@pytest.mark.parametrize(
    ("formal", "policy_label"),
    [
        ("globals()=0", "dangerous builtin name"),
        ("open('chatdb_parser_probe.txt')=0", "dangerous builtin function"),
        ("x.__class__=x", "attribute access"),
    ],
)
def test_should_enforce_parser_allowlist_when_dangerous_names_functions_or_attributes_appear(
    formal: str,
    policy_label: str,
):
    result = validate_sympy(formal, "")

    assert result.passed is False, (
        "Unsafe symbolic payload must be rejected by parser allowlist policy. "
        f"Policy case={policy_label!r}, formal={formal!r}, got passed={result.passed}, "
        f"message={result.message!r}, raw_output={result.raw_output!r}."
    )
    assert any(
        token in result.message.lower()
        for token in ("disallow", "forbidden", "blocked", "unsafe")
    ), (
        "Rejection reason should explicitly indicate parser allowlist/policy block. "
        f"Policy case={policy_label!r}, formal={formal!r}, got message={result.message!r}."
    )


def test_should_accept_valid_symbolic_expression_when_expression_is_safe_and_algebraic():
    result = validate_sympy("(a+b)**2 = a**2 + 2*a*b + b**2", "")
    assert result.passed is True, (
        "Safe algebraic identity should still pass validation after parser hardening. "
        f"Got passed={result.passed}, message={result.message!r}, raw_output={result.raw_output!r}."
    )


def test_should_accept_valid_symbolic_expression_when_allowed_sympy_function_is_used():
    result = validate_sympy("sin(x)**2 + cos(x)**2 = 1", "")
    assert result.passed is True, (
        "Allowed symbolic functions should remain usable for safe expressions. "
        f"Got passed={result.passed}, message={result.message!r}, raw_output={result.raw_output!r}."
    )
