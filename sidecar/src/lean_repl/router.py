"""FastAPI endpoints for the Lean 4 REPL scratch pad.

Provides three endpoints for solver agents:
  POST /lean/check   — verify an equality (lhs = rhs)
  POST /lean/cmd     — send arbitrary Lean command
  POST /lean/tactic  — send a tactic against a proof state
  GET  /lean/status  — REPL readiness state
"""
import asyncio
from fastapi import APIRouter
from pydantic import BaseModel
from typing import Optional

from . import formalize as formalize_pipeline
from .session import LeanREPL, REPLState

router = APIRouter()

# Singleton REPL instance — started by sidecar lifespan
_repl: Optional[LeanREPL] = None
_restart_lock = asyncio.Lock()


def get_repl() -> LeanREPL:
    """Get or create the singleton REPL."""
    global _repl
    if _repl is None:
        _repl = LeanREPL()
    return _repl


async def start_repl():
    """Start the REPL (called from sidecar lifespan)."""
    repl = get_repl()
    if repl.state == REPLState.COLD:
        await repl.start()


async def stop_repl():
    """Stop the REPL (called from sidecar shutdown)."""
    repl = get_repl()
    if repl.state != REPLState.COLD:
        await repl.stop()


async def _ensure_repl_ready() -> LeanREPL:
    """Get the REPL, auto-restarting if it crashed.

    Uses a lock to prevent multiple concurrent restart attempts.
    """
    repl = get_repl()
    if repl.state == REPLState.READY:
        return repl

    if repl.state in (REPLState.ERROR, REPLState.COLD):
        async with _restart_lock:
            # Re-check after acquiring lock (another caller may have restarted)
            if repl.state == REPLState.READY:
                return repl
            if repl.state in (REPLState.ERROR, REPLState.COLD):
                print(f"Lean REPL: auto-restarting (was {repl.state.value})")
                try:
                    await repl.restart()
                    print("Lean REPL: auto-restart succeeded")
                except Exception as e:
                    print(f"Lean REPL: auto-restart failed — {e}")
                    raise RuntimeError(f"REPL auto-restart failed: {e}") from e

    if repl.state == REPLState.WARMING:
        await repl._ready_event.wait()
        if repl.state != REPLState.READY:
            raise RuntimeError(f"REPL failed to warm up (state={repl.state})")

    return repl


# --- Request / Response models ---

class LeanCheckRequest(BaseModel):
    lhs: str
    rhs: str
    variables: list[str] = []
    typ: str = "Int"

class LeanCheckResponse(BaseModel):
    passed: bool
    error: Optional[str] = None
    env: Optional[int] = None
    wall_time_ms: int = 0

class LeanCmdRequest(BaseModel):
    cmd: str
    env: Optional[int] = None

class LeanCmdResponse(BaseModel):
    env: Optional[int] = None
    messages: list = []
    sorries: list = []
    has_errors: bool = False

class LeanTacticRequest(BaseModel):
    tactic: str
    proof_state: int

class LeanTacticResponse(BaseModel):
    proof_state: Optional[int] = None
    goals: list = []
    is_complete: bool = False
    messages: list = []

class LeanStatusResponse(BaseModel):
    state: str
    base_env: Optional[int] = None


class LeanFormalizeStep(BaseModel):
    step_number: int
    proposal_type: str
    natural: str
    formal: Optional[str] = None
    model: str
    obligation_id: Optional[str] = None
    obligation_desc: Optional[str] = None
    obligation_type: Optional[str] = None


class LeanFormalizeObligation(BaseModel):
    id: str
    description: str
    obligation_type: str
    status: str


class LeanFormalizeRequest(BaseModel):
    problem_id: str
    problem_statement: str
    problem_domain: Optional[str] = None
    problem_formal_statement: Optional[str] = None
    attempt_id: str
    final_answer: Optional[str] = None
    verified_chain: list[LeanFormalizeStep]
    obligations: list[LeanFormalizeObligation] = []


class LeanFormalizeResponse(BaseModel):
    success: bool
    lean_source: str
    errors: list[str] = []
    attempts: int = 0


# --- Endpoints ---

@router.get("/status", response_model=LeanStatusResponse)
async def lean_status():
    repl = get_repl()
    return LeanStatusResponse(
        state=repl.state.value,
        base_env=repl._base_env,
    )


@router.post("/check", response_model=LeanCheckResponse)
async def lean_check(req: LeanCheckRequest):
    """Verify an equality: lhs = rhs using ring/omega/norm_num/simp/decide.

    This is the primary tool for solver agents to kernel-verify expressions
    during reasoning, before submitting a step.
    """
    import time
    start = time.perf_counter_ns()

    try:
        repl = await _ensure_repl_ready()
        result = await asyncio.wait_for(
            repl.check_equality(req.lhs, req.rhs, req.variables, req.typ),
            timeout=45.0,
        )
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return LeanCheckResponse(
            passed=result.passed or False,
            error=result.error,
            env=result.env,
            wall_time_ms=wall_ms,
        )
    except (RuntimeError, asyncio.TimeoutError) as e:
        wall_ms = (time.perf_counter_ns() - start) // 1_000_000
        return LeanCheckResponse(
            passed=False,
            error=str(e),
            wall_time_ms=wall_ms,
        )


@router.post("/cmd", response_model=LeanCmdResponse)
async def lean_cmd(req: LeanCmdRequest):
    """Send an arbitrary Lean 4 command to the REPL.

    Use this for defining helper lemmas, exploring types, or building
    incremental proofs. The returned env id can be used in subsequent calls.
    """
    try:
        repl = await _ensure_repl_ready()
        result = await asyncio.wait_for(
            repl.cmd(req.cmd, env=req.env),
            timeout=45.0,
        )
        return LeanCmdResponse(
            env=result.env,
            messages=result.messages,
            sorries=result.sorries,
            has_errors=result.has_errors,
        )
    except (RuntimeError, asyncio.TimeoutError) as e:
        return LeanCmdResponse(
            messages=[{"severity": "error", "data": str(e)}],
            has_errors=True,
        )


@router.post("/tactic", response_model=LeanTacticResponse)
async def lean_tactic(req: LeanTacticRequest):
    """Send a tactic to the REPL against a proof state.

    Use this for interactive proof building: create a theorem with sorry,
    get a proofState id, then send tactics to close goals.
    """
    try:
        repl = await _ensure_repl_ready()
        result = await asyncio.wait_for(
            repl.tactic(req.tactic, proof_state=req.proof_state),
            timeout=45.0,
        )
        return LeanTacticResponse(
            proof_state=result.proof_state,
            goals=result.goals or [],
            is_complete=result.is_proof_complete,
            messages=result.messages,
        )
    except (RuntimeError, asyncio.TimeoutError) as e:
        return LeanTacticResponse(
            messages=[{"severity": "error", "data": str(e)}],
        )


@router.post("/formalize", response_model=LeanFormalizeResponse)
async def lean_formalize(req: LeanFormalizeRequest):
    """Build a deterministic Lean theorem candidate from a completed proof."""
    try:
        repl = await _ensure_repl_ready()
        success, lean_source, errors, attempts = await formalize_pipeline.run_formalization(
            req, repl
        )
        return LeanFormalizeResponse(
            success=success,
            lean_source=lean_source,
            errors=errors,
            attempts=attempts,
        )
    except (RuntimeError, asyncio.TimeoutError) as e:
        return LeanFormalizeResponse(
            success=False,
            lean_source="",
            errors=[str(e)],
            attempts=0,
        )
