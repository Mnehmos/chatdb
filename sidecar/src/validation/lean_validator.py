"""Lean 4 validator — subprocess approach with Mathlib.

Auto-translates SymPy-syntax formal expressions into Lean 4 theorems,
then runs `lake env lean --json` (with Mathlib for `ring`, `norm_num`, etc.)
to type-check. Uses `first | ring | omega | norm_num | simp | decide` in a
single invocation so Mathlib is imported only once.

Gracefully degrades when Lean 4 is not installed.
"""
import json
import os
import pathlib
import re
import shutil
import subprocess
import tempfile
import time

# Path to the Lake project with Mathlib dependency
_CHECKER_PROJECT = pathlib.Path(__file__).resolve().parent.parent.parent / "lean-checker"

# elan installs to ~/.elan/bin — check there if lake isn't on PATH
_ELAN_BIN = pathlib.Path.home() / ".elan" / "bin"


def _find_lake() -> str | None:
    """Find the lake binary — check PATH first, then ~/.elan/bin."""
    found = shutil.which("lake")
    if found:
        return found
    candidate = _ELAN_BIN / ("lake.exe" if os.name == "nt" else "lake")
    if candidate.exists():
        return str(candidate)
    return None


def _path_is_usable(path_str: str | None) -> bool:
    """Best-effort existence check that treats inaccessible paths as unusable."""
    if not path_str:
        return False
    try:
        return pathlib.Path(path_str).exists()
    except OSError:
        return False


def _lean_env() -> dict[str, str]:
    """Build an env that prefers a usable elan home and writable home directory."""
    env = os.environ.copy()
    elan_home = _ELAN_BIN.parent
    if "ELAN_HOME" not in env and elan_home.exists():
        env["ELAN_HOME"] = str(elan_home)

    fallback_home = str(_CHECKER_PROJECT)
    for key in ("HOME", "USERPROFILE"):
        if not _path_is_usable(env.get(key)):
            env[key] = fallback_home

    return env


def _lean_runtime_available(lake: str) -> bool:
    """Return True only if lake can execute inside the checker project."""
    try:
        proc = subprocess.run(
            [lake, "env", "lean", "--version"],
            capture_output=True,
            text=True,
            timeout=30,
            cwd=str(_CHECKER_PROJECT),
            env=_lean_env(),
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return proc.returncode == 0


_lean_available_cache: bool | None = None

def _lean_available() -> bool:
    """Check if Lean is both installed and runnable for this project.

    Caches True permanently (won't un-install itself).
    Caches False for one call only — retries on next check so that
    slow installs or warmup races don't permanently disable Lean.
    """
    global _lean_available_cache
    if _lean_available_cache is True:
        return True
    lake = _find_lake()
    if not lake or not _CHECKER_PROJECT.exists():
        return False
    result = _lean_runtime_available(lake)
    if result:
        _lean_available_cache = True
    return result


def _extract_variables(expr: str) -> list[str]:
    """Find single-letter free variables in a SymPy expression."""
    tokens = set(re.findall(r'\b([a-zA-Z])\b', expr))
    # Filter out common Lean/math constants
    tokens -= {"I", "E", "S", "N", "O"}
    return sorted(tokens)


def _sympy_to_lean(formal: str) -> str | None:
    """Translate SymPy-syntax expression to Lean 4 syntax.

    Returns None if the expression contains constructs that can't be
    translated to valid Lean 4 (e.g. Sum, Integral, floor).  The caller
    should skip Lean validation rather than reject the step.

    Conversions applied:
      ** → ^
    """
    s = formal.strip()
    s = s.replace("**", "^")

    # Bail out if expression still contains SymPy function-call syntax
    # that Lean can't parse.  These patterns produce guaranteed parse errors.
    # NOTE: sqrt, log, exp, sin, cos, tan ARE valid in Lean 4 with Mathlib
    # (Real.sqrt, Real.log, Real.exp, Real.sin, etc.) so they are NOT blocked.
    _UNTRANSLATABLE = re.compile(
        r'\b(Sum|Product|Integral|floor|ceil|factorial|binomial|'
        r'Mod|Abs|atan2|'
        r'gamma|beta|zeta|Rational|oo|zoo|nan|sign|'
        r'Piecewise|Max|Min|limit|diff|integrate|solve|simplify)\s*\(',
        re.IGNORECASE,
    )
    if _UNTRANSLATABLE.search(s):
        return None

    return s


# Targeted Mathlib imports — only ring and norm_num need Mathlib.
# omega, simp, and decide are Lean 4 core tactics (no Mathlib needed).
# Using targeted imports reduces load time from ~3min to ~14s.
_MATHLIB_IMPORT = (
    "import Mathlib.Tactic.Omega\n"
    "import Mathlib.Tactic.Ring\n"
    "import Mathlib.Tactic.NormNum\n"
    "set_option linter.unusedTactic false\n"
    "set_option linter.unreachableTactic false\n\n"
)

# Tactics to try, in order of specificity
_TACTICS = ["ring", "omega", "norm_num", "simp", "decide"]


def _needs_rationals(lhs: str, rhs: str) -> bool:
    """Return True if the expression involves division and needs ℚ instead of Int."""
    return "/" in lhs or "/" in rhs


def _build_lean_file(lean_lhs: str, lean_rhs: str, variables: list[str], tactic: str) -> str:
    """Generate a complete .lean file that checks an equality theorem."""
    header = _MATHLIB_IMPORT
    typ = "ℚ" if _needs_rationals(lean_lhs, lean_rhs) else "Int"
    if variables:
        params = " ".join(f"({v} : {typ})" for v in variables)
        return f"{header}theorem step {params} : {lean_lhs} = {lean_rhs} := by {tactic}\n"
    else:
        return f"{header}theorem step : {lean_lhs} = {lean_rhs} := by {tactic}\n"


def _build_lean_file_multi(lean_lhs: str, lean_rhs: str, variables: list[str]) -> str:
    """Generate a .lean file that tries all tactics via `first |`.

    This imports Mathlib once and uses Lean's `first` combinator to try
    each tactic in sequence, avoiding 5 separate subprocess invocations.
    """
    header = _MATHLIB_IMPORT
    tactics_chain = " | ".join(_TACTICS)
    typ = "ℚ" if _needs_rationals(lean_lhs, lean_rhs) else "Int"
    if variables:
        params = " ".join(f"({v} : {typ})" for v in variables)
        return f"{header}theorem step {params} : {lean_lhs} = {lean_rhs} := by first | {tactics_chain}\n"
    else:
        return f"{header}theorem step : {lean_lhs} = {lean_rhs} := by first | {tactics_chain}\n"


def _run_lean(code: str, timeout: int = 30) -> tuple[bool, list[dict], str]:
    """Write code to temp file, run lake env lean --json, return (success, diagnostics, stderr).

    Uses `lake env lean` from the lean-checker project to access Mathlib.
    Timeout defaults to 30s — sufficient for targeted imports (~14s warm).
    Warmup passes a higher timeout for the first cold run.
    """
    lake = _find_lake()
    if not lake:
        return False, [{"severity": "error", "data": "lake binary not found"}], ""

    fd, path = tempfile.mkstemp(suffix=".lean", prefix="chatdb_")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            f.write(code)

        proc = subprocess.run(
            [lake, "env", "lean", "--json", "-T", str(timeout), path],
            capture_output=True,
            text=True,
            timeout=timeout + 30,
            cwd=str(_CHECKER_PROJECT),
            env=_lean_env(),
        )

        diagnostics = []
        for line in proc.stdout.strip().split("\n"):
            line = line.strip()
            if line:
                try:
                    diagnostics.append(json.loads(line))
                except json.JSONDecodeError:
                    pass

        errors = [d for d in diagnostics if d.get("severity") == "error"]
        warnings = [d for d in diagnostics if d.get("severity") == "warning"]
        has_sorry = any("sorry" in str(d.get("data", "")) for d in warnings)

        success = proc.returncode == 0 and not errors and not has_sorry
        return success, diagnostics, proc.stderr
    except subprocess.TimeoutExpired:
        return False, [{"severity": "error", "data": "Lean timed out"}], ""
    except FileNotFoundError:
        return False, [{"severity": "error", "data": "lake binary not found"}], ""
    finally:
        try:
            os.unlink(path)
        except OSError:
            pass


def validate_lean(formal: str, goal_state: str):
    """Validate an algebraic equality using Lean 4.

    Takes a SymPy-syntax formal expression (e.g. "x**2 - 1 = (x-1)*(x+1)"),
    auto-translates to Lean 4, and checks with the proof kernel.
    """
    from .router import ValidatorResult
    start = time.perf_counter_ns()

    # Skip if warmup is still running — avoid concurrent lake processes
    # that fight over the same toolchain and cause timeouts.
    with _warmup_lock:
        still_warming = lean_warming_up
    if still_warming:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return ValidatorResult(
            passed=False,
            message="Lean warming up — skipped (will validate once warm)",
            raw_output="",
            wall_time_ms=wall_ms,
        )

    # Input validation first (before checking lean availability)
    if not formal or "=" not in formal or "==" in formal:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return ValidatorResult(
            passed=False,
            message="No equality to check (formal must contain '=')",
            raw_output=formal or "",
            wall_time_ms=wall_ms,
        )

    # Split on '=' — take first and last parts for LHS/RHS
    parts = formal.split("=")
    if len(parts) != 2:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return ValidatorResult(
            passed=False,
            message=f"Expected exactly one '=' in formal, got {len(parts) - 1}",
            raw_output=formal,
            wall_time_ms=wall_ms,
        )

    if not _lean_available():
        return ValidatorResult(
            passed=False,
            message="Lean 4 not available (need elan + lean-checker project with Mathlib)",
            raw_output="",
            wall_time_ms=0,
        )

    lean_lhs = _sympy_to_lean(parts[0])
    lean_rhs = _sympy_to_lean(parts[1])

    # If either side contains untranslatable SymPy constructs, skip Lean
    # entirely — returning None tells the router to omit lean_passed rather
    # than mark it as failed.
    if lean_lhs is None or lean_rhs is None:
        return None

    variables = _extract_variables(formal)

    # Single invocation: try all tactics via `first | ring | omega | ...`
    # This imports Mathlib once instead of 5 separate subprocess calls.
    code = _build_lean_file_multi(lean_lhs, lean_rhs, variables)
    success, diagnostics, stderr = _run_lean(code)

    wall_ms = (time.perf_counter_ns() - start) // 1_000_000

    if success:
        return ValidatorResult(
            passed=True,
            message="Lean verified",
            raw_output=code.strip(),
            wall_time_ms=wall_ms,
        )

    # All tactics failed
    error_msgs = "; ".join(
        str(e.get("data", e.get("message", "unknown error")))
        for e in diagnostics
        if e.get("severity") == "error"
    )[:500]
    return ValidatorResult(
        passed=False,
        message=f"Lean: all tactics failed. {error_msgs}" if error_msgs else "Lean: all tactics failed",
        raw_output=code.strip(),
        wall_time_ms=wall_ms,
    )


def validate_lean_native(formal_lean: str):
    """Validate a solver-produced Lean 4 equality expression directly.

    Unlike validate_lean() which auto-translates from SymPy, this takes
    a Lean 4 expression produced by the solver and wraps it in a theorem.
    The solver is responsible for correct Lean syntax.

    The expression should be a Lean 4 equality, e.g.:
      "n * (n + 1) / 2 = Finset.sum (Finset.range n) (fun k => ↑k + 1)"
    """
    from .router import ValidatorResult
    start = time.perf_counter_ns()

    with _warmup_lock:
        still_warming = lean_warming_up
    if still_warming:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return ValidatorResult(
            passed=False,
            message="Lean warming up — skipped (will validate once warm)",
            raw_output="",
            wall_time_ms=wall_ms,
        )

    if not formal_lean or "=" not in formal_lean or "==" in formal_lean:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return ValidatorResult(
            passed=False,
            message="No equality to check (formal_lean must contain '=')",
            raw_output=formal_lean or "",
            wall_time_ms=wall_ms,
        )

    if not _lean_available():
        return ValidatorResult(
            passed=False,
            message="Lean 4 not available",
            raw_output="",
            wall_time_ms=0,
        )

    # Split on first '=' that isn't part of ':=' or '<=' or '>='
    # Simple approach: split on ' = ' (space-delimited equality)
    parts = formal_lean.split("=")
    if len(parts) < 2:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return ValidatorResult(
            passed=False,
            message="Could not split formal_lean into LHS = RHS",
            raw_output=formal_lean,
            wall_time_ms=wall_ms,
        )

    lhs = parts[0].strip()
    rhs = "=".join(parts[1:]).strip()

    variables = _extract_variables(formal_lean)

    # Determine type — use ℚ if division present
    typ = "ℚ" if _needs_rationals(lhs, rhs) else "Int"
    tactics_chain = " | ".join(_TACTICS)

    header = _MATHLIB_IMPORT
    if variables:
        params = " ".join(f"({v} : {typ})" for v in variables)
        code = f"{header}theorem step {params} : {lhs} = {rhs} := by first | {tactics_chain}\n"
    else:
        code = f"{header}theorem step : {lhs} = {rhs} := by first | {tactics_chain}\n"

    success, diagnostics, stderr = _run_lean(code)
    wall_ms = (time.perf_counter_ns() - start) // 1_000_000

    if success:
        return ValidatorResult(
            passed=True,
            message="Lean verified (native)",
            raw_output=code.strip(),
            wall_time_ms=wall_ms,
        )

    error_msgs = "; ".join(
        str(e.get("data", e.get("message", "unknown error")))
        for e in diagnostics
        if e.get("severity") == "error"
    )[:500]
    return ValidatorResult(
        passed=False,
        message=f"Lean (native): all tactics failed. {error_msgs}" if error_msgs else "Lean (native): all tactics failed",
        raw_output=code.strip(),
        wall_time_ms=wall_ms,
    )


# Global readiness flag — set by warmup_lean(), read by /health endpoint
import threading

_warmup_lock = threading.Lock()
lean_warm: bool = False
lean_warming_up: bool = False
warmup_attempts: int = 0
_MAX_WARMUP_ATTEMPTS: int = 3
_WARMUP_RETRY_DELAY: float = 30.0


def warmup_lean() -> bool:
    """Run a trivial Lean check to warm up Mathlib import cache.

    Retries up to 3 times with 30s delay between attempts.
    Returns True if Lean warmed up successfully.
    """
    global lean_warm, lean_warming_up, warmup_attempts

    if not _lean_available():
        print("Lean warmup: skipped (not available)")
        return False

    with _warmup_lock:
        lean_warming_up = True

    while warmup_attempts < _MAX_WARMUP_ATTEMPTS:
        warmup_attempts += 1
        print(f"Lean warmup: attempt {warmup_attempts}/{_MAX_WARMUP_ATTEMPTS} (may take 30-60s on first run)...")
        start = time.perf_counter_ns()
        code = _MATHLIB_IMPORT + "theorem warmup : 1 = 1 := by norm_num\n"
        success, _, _ = _run_lean(code, timeout=90)
        elapsed_s = (time.perf_counter_ns() - start) / 1_000_000_000

        if success:
            with _warmup_lock:
                lean_warm = True
                lean_warming_up = False
            print(f"Lean warmup: OK ({elapsed_s:.1f}s) on attempt {warmup_attempts}")
            return True

        print(f"Lean warmup: failed ({elapsed_s:.1f}s) on attempt {warmup_attempts}")
        if warmup_attempts < _MAX_WARMUP_ATTEMPTS:
            print(f"Lean warmup: retrying in {_WARMUP_RETRY_DELAY}s...")
            time.sleep(_WARMUP_RETRY_DELAY)

    with _warmup_lock:
        lean_warming_up = False
        lean_warm = False
    print(f"Lean warmup: exhausted all {_MAX_WARMUP_ATTEMPTS} attempts")
    return False
