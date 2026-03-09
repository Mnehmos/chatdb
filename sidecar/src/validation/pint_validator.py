"""Pint validator -- dimensional analysis."""
import time
from pint import UnitRegistry
ureg = UnitRegistry()

def validate_pint(expression: str):
    from .router import ValidatorResult
    start = time.perf_counter_ns()
    try:
        result = ureg.parse_expression(expression)
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return ValidatorResult(passed=True, message=f"Dimensionally consistent: {result.units}", raw_output=str(result), wall_time_ms=wall_ms)
    except Exception as e:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return ValidatorResult(passed=False, message=f"Dimensional error: {e}", raw_output=str(e), wall_time_ms=wall_ms)
