"""Tests for Lean 4 REPL session manager."""
import asyncio
import json
import pytest
from unittest.mock import AsyncMock, MagicMock, patch

from src.lean_repl.session import LeanREPL, REPLResult, REPLState


# --- Unit tests (mock subprocess) ---


class TestREPLLifecycle:
    """Test REPL process lifecycle management."""

    def test_initial_state_is_cold(self):
        repl = LeanREPL()
        assert repl.state == REPLState.COLD

    @pytest.mark.asyncio
    async def test_start_transitions_to_warming(self):
        """Starting the REPL should transition through WARMING to READY."""
        repl = LeanREPL()
        with patch.object(repl, '_spawn_process', new_callable=AsyncMock) as mock_spawn:
            mock_spawn.return_value = True
            with patch.object(repl, '_import_mathlib', new_callable=AsyncMock) as mock_import:
                mock_import.return_value = REPLResult(env=0, messages=[], sorries=[])
                await repl.start()
        assert repl.state == REPLState.READY

    @pytest.mark.asyncio
    async def test_stop_kills_process(self):
        repl = LeanREPL()
        repl.state = REPLState.READY
        mock_proc = MagicMock()
        mock_proc.returncode = None
        mock_proc.terminate = MagicMock()
        mock_proc.wait = AsyncMock()
        repl._process = mock_proc
        await repl.stop()
        assert repl.state == REPLState.COLD
        mock_proc.terminate.assert_called_once()

    @pytest.mark.asyncio
    async def test_restart_cycles_process(self):
        repl = LeanREPL()
        repl.state = REPLState.READY
        repl._process = MagicMock()
        repl._process.returncode = None
        repl._process.terminate = MagicMock()
        repl._process.wait = AsyncMock()
        with patch.object(repl, 'start', new_callable=AsyncMock) as mock_start:
            await repl.restart()
            mock_start.assert_called_once()


class TestREPLCommands:
    """Test REPL command/tactic sending."""

    @pytest.mark.asyncio
    async def test_cmd_returns_result(self):
        """Sending a command returns a REPLResult with env id."""
        repl = LeanREPL()
        repl.state = REPLState.READY
        repl._base_env = 0
        response = {"env": 1, "messages": [], "sorries": []}
        with patch.object(repl, '_send_recv', new_callable=AsyncMock, return_value=response):
            result = await repl.cmd("def f := 2")
        assert result.env == 1
        assert result.messages == []

    @pytest.mark.asyncio
    async def test_cmd_with_env(self):
        """Commands can specify an env to build on."""
        repl = LeanREPL()
        repl.state = REPLState.READY
        repl._base_env = 0
        response = {"env": 2, "messages": [], "sorries": []}
        with patch.object(repl, '_send_recv', new_callable=AsyncMock, return_value=response) as mock:
            result = await repl.cmd("example : f = 2 := rfl", env=1)
        sent = mock.call_args[0][0]
        assert sent["env"] == 1

    @pytest.mark.asyncio
    async def test_tactic_returns_goals(self):
        """Tactic mode returns remaining goals."""
        repl = LeanREPL()
        repl.state = REPLState.READY
        repl._base_env = 0
        response = {"proofState": 1, "goals": ["⊢ Nat"]}
        with patch.object(repl, '_send_recv', new_callable=AsyncMock, return_value=response):
            result = await repl.tactic("ring", proof_state=0)
        assert result.goals == ["⊢ Nat"]
        assert result.proof_state == 1

    @pytest.mark.asyncio
    async def test_tactic_empty_goals_means_done(self):
        """Empty goals list means the proof is complete."""
        repl = LeanREPL()
        repl.state = REPLState.READY
        repl._base_env = 0
        response = {"proofState": 2, "goals": []}
        with patch.object(repl, '_send_recv', new_callable=AsyncMock, return_value=response):
            result = await repl.tactic("exact rfl", proof_state=1)
        assert result.goals == []
        assert result.is_proof_complete


class TestREPLQueue:
    """Test queuing behavior when REPL isn't ready."""

    @pytest.mark.asyncio
    async def test_cmd_queues_when_warming(self):
        """Commands sent during warmup should queue and execute once ready."""
        repl = LeanREPL()
        repl.state = REPLState.WARMING
        repl._ready_event = asyncio.Event()
        repl._base_env = 0

        response = {"env": 1, "messages": [], "sorries": []}

        async def simulate_ready():
            await asyncio.sleep(0.05)
            repl.state = REPLState.READY
            repl._ready_event.set()

        with patch.object(repl, '_send_recv', new_callable=AsyncMock, return_value=response):
            ready_task = asyncio.create_task(simulate_ready())
            result = await asyncio.wait_for(repl.cmd("def f := 2"), timeout=2.0)
            await ready_task

        assert result.env == 1

    @pytest.mark.asyncio
    async def test_cmd_fails_when_cold(self):
        """Commands should fail immediately when REPL is COLD (not started)."""
        repl = LeanREPL()
        repl.state = REPLState.COLD
        with pytest.raises(RuntimeError, match="not started"):
            await repl.cmd("def f := 2")


class TestREPLCheck:
    """Test the high-level check_equality convenience method."""

    @pytest.mark.asyncio
    async def test_check_equality_valid(self):
        """Valid equality returns passed=True."""
        repl = LeanREPL()
        repl.state = REPLState.READY
        repl._base_env = 0
        # No errors, no sorries = success
        response = {"env": 1, "messages": [], "sorries": []}
        with patch.object(repl, '_send_recv', new_callable=AsyncMock, return_value=response):
            result = await repl.check_equality("x^2 - 1", "(x-1)*(x+1)", ["x"])
        assert result.passed is True

    @pytest.mark.asyncio
    async def test_check_equality_invalid(self):
        """Invalid equality returns passed=False with error message."""
        repl = LeanREPL()
        repl.state = REPLState.READY
        repl._base_env = 0
        response = {
            "env": 1,
            "messages": [{"severity": "error", "data": "ring failed", "pos": {"line": 1, "column": 0}}],
            "sorries": [],
        }
        with patch.object(repl, '_send_recv', new_callable=AsyncMock, return_value=response):
            result = await repl.check_equality("x^2 - 1", "(x-1)*(x+2)", ["x"])
        assert result.passed is False
        assert "ring failed" in result.error


# --- Integration tests (require Lean + REPL built) ---

from src.validation.lean_validator import _lean_available
import pathlib
import subprocess

_CHECKER_PROJECT = pathlib.Path(__file__).resolve().parent.parent.parent / "lean-checker"

def _repl_available() -> bool:
    """Check if `lake exe repl` works."""
    if not _lean_available():
        return False
    try:
        from src.lean_repl.session import _find_lake
        lake = _find_lake()
        if not lake:
            return False
        proc = subprocess.run(
            [lake, "exe", "repl"],
            input='{"cmd": "#check Nat"}\n\n',
            capture_output=True, text=True, timeout=30,
            cwd=str(_CHECKER_PROJECT),
        )
        return '"env"' in proc.stdout
    except Exception:
        return False

needs_repl = pytest.mark.skipif(
    not _repl_available(),
    reason="Lean 4 REPL not available (run `lake build repl` in lean-checker/)"
)


@needs_repl
class TestREPLIntegration:
    """Integration tests that actually launch the Lean REPL process."""

    @pytest.mark.asyncio
    async def test_start_and_import_mathlib(self):
        repl = LeanREPL()
        try:
            await asyncio.wait_for(repl.start(), timeout=120)
            assert repl.state == REPLState.READY
            assert repl._base_env >= 0
        finally:
            await repl.stop()

    @pytest.mark.asyncio
    async def test_check_ring_identity(self):
        repl = LeanREPL()
        try:
            await asyncio.wait_for(repl.start(), timeout=120)
            result = await repl.check_equality("x^2 - 1", "(x-1)*(x+1)", ["x"])
            assert result.passed is True
        finally:
            await repl.stop()

    @pytest.mark.asyncio
    async def test_check_invalid_equality(self):
        repl = LeanREPL()
        try:
            await asyncio.wait_for(repl.start(), timeout=120)
            result = await repl.check_equality("x^2 - 1", "(x-1)*(x+2)", ["x"])
            assert result.passed is False
        finally:
            await repl.stop()

    @pytest.mark.asyncio
    async def test_tactic_mode(self):
        """Test interactive tactic mode: sorry → apply tactic → complete."""
        repl = LeanREPL()
        try:
            await asyncio.wait_for(repl.start(), timeout=120)
            # Create a theorem with sorry to get proof state
            result = await repl.cmd(
                "theorem test (x : Int) : x + 0 = x := by sorry"
            )
            assert len(result.sorries) > 0
            ps = result.sorries[0]["proofState"]

            # Apply ring tactic
            tac_result = await repl.tactic("ring", proof_state=ps)
            assert tac_result.is_proof_complete
        finally:
            await repl.stop()

    @pytest.mark.asyncio
    async def test_sequential_checks_fast(self):
        """After warmup, subsequent checks should be fast (<2s each)."""
        import time
        repl = LeanREPL()
        try:
            await asyncio.wait_for(repl.start(), timeout=120)
            # First check warms up
            await repl.check_equality("1 + 1", "2", [])
            # Second check should be fast
            t0 = time.perf_counter()
            result = await repl.check_equality("a + b", "b + a", ["a", "b"])
            elapsed = time.perf_counter() - t0
            assert result.passed is True
            assert elapsed < 5.0, f"Second check took {elapsed:.1f}s, expected <5s"
        finally:
            await repl.stop()

    @pytest.mark.asyncio
    async def test_env_chaining(self):
        """Definitions persist across commands in the same env chain."""
        repl = LeanREPL()
        try:
            await asyncio.wait_for(repl.start(), timeout=120)
            r1 = await repl.cmd("def myConst := 42")
            r2 = await repl.cmd("example : myConst = 42 := rfl", env=r1.env)
            assert not any(m.get("severity") == "error" for m in r2.messages)
        finally:
            await repl.stop()
