"""Lean 4 REPL session manager.

Manages a persistent `lake env repl` subprocess that keeps Mathlib loaded
in memory. After the initial ~14s import, subsequent checks complete in <1s.

Architecture:
  - LeanREPL wraps a single subprocess (stdin/stdout JSON protocol)
  - On start(), imports Mathlib and caches the base env id
  - cmd() sends commands against the base env (or a specified env)
  - tactic() sends tactics against proof states
  - check_equality() is a convenience wrapper for the common case
  - When state is WARMING, commands queue until ready (asyncio.Event)
  - When state is COLD, commands fail immediately
"""
import asyncio
import json
import os
import pathlib
import shutil
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional


class REPLState(Enum):
    COLD = "cold"          # Not started
    WARMING = "warming"    # Process launched, importing Mathlib
    READY = "ready"        # Mathlib loaded, accepting commands
    ERROR = "error"        # Process crashed or failed to start


@dataclass
class REPLResult:
    """Result from a REPL command or tactic."""
    env: Optional[int] = None
    messages: list = field(default_factory=list)
    sorries: list = field(default_factory=list)
    goals: Optional[list] = None
    proof_state: Optional[int] = None
    passed: Optional[bool] = None
    error: Optional[str] = None
    raw: Optional[dict] = None

    @property
    def is_proof_complete(self) -> bool:
        return self.goals is not None and len(self.goals) == 0

    @property
    def has_errors(self) -> bool:
        return any(m.get("severity") == "error" for m in self.messages)


# Path to the Lake project with Mathlib dependency
# lean-checker is at repo root: chatdb/lean-checker
# session.py is at: chatdb/sidecar/src/lean_repl/session.py
_CHECKER_PROJECT = pathlib.Path(__file__).resolve().parent.parent.parent.parent / "lean-checker"
_ELAN_BIN = pathlib.Path.home() / ".elan" / "bin"

# Targeted imports — same as lean_validator.py
_MATHLIB_IMPORTS = (
    "import Mathlib.Tactic.Omega\n"
    "import Mathlib.Tactic.Ring\n"
    "import Mathlib.Tactic.NormNum\n"
    "set_option linter.unusedTactic false\n"
    "set_option linter.unreachableTactic false"
)


def _find_lake() -> Optional[str]:
    found = shutil.which("lake")
    if found:
        return found
    candidate = _ELAN_BIN / ("lake.exe" if os.name == "nt" else "lake")
    if candidate.exists():
        return str(candidate)
    return None


def _find_repl_bin() -> Optional[str]:
    """Find the built REPL binary."""
    for name in ("repl", "repl.exe"):
        candidate = _CHECKER_PROJECT / ".lake" / "packages" / "repl" / ".lake" / "build" / "bin" / name
        if candidate.exists():
            return str(candidate)
    return None


def _lean_env() -> dict:
    """Build env dict for subprocess — same as lean_validator."""
    env = os.environ.copy()
    elan_home = _ELAN_BIN.parent
    if "ELAN_HOME" not in env and elan_home.exists():
        env["ELAN_HOME"] = str(elan_home)
    fallback_home = str(_CHECKER_PROJECT)
    for key in ("HOME", "USERPROFILE"):
        if not env.get(key) or not pathlib.Path(env[key]).exists():
            env[key] = fallback_home
    return env


class LeanREPL:
    """Persistent Lean 4 REPL session with Mathlib pre-loaded.

    Usage:
        repl = LeanREPL()
        await repl.start()  # imports Mathlib (~14s first time)
        result = await repl.check_equality("x^2 - 1", "(x-1)*(x+1)", ["x"])
        assert result.passed
        await repl.stop()
    """

    def __init__(self):
        self.state = REPLState.COLD
        self._process: Optional[asyncio.subprocess.Process] = None
        self._base_env: Optional[int] = None
        self._ready_event = asyncio.Event()
        self._lock = asyncio.Lock()

    async def start(self):
        """Launch the REPL process and import Mathlib."""
        self.state = REPLState.WARMING
        self._ready_event.clear()

        try:
            await self._spawn_process()
            result = await self._import_mathlib()
            self._base_env = result.env
            self.state = REPLState.READY
            self._ready_event.set()
        except Exception as e:
            self.state = REPLState.ERROR
            self._ready_event.set()  # unblock waiters with error
            raise RuntimeError(f"REPL failed to start: {e}") from e

    async def stop(self):
        """Kill the REPL process."""
        if self._process and self._process.returncode is None:
            self._process.terminate()
            try:
                await asyncio.wait_for(self._process.wait(), timeout=5.0)
            except asyncio.TimeoutError:
                self._process.kill()
        self._process = None
        self._base_env = None
        self.state = REPLState.COLD
        self._ready_event.clear()

    async def restart(self):
        """Stop and restart the REPL."""
        await self.stop()
        await self.start()

    async def cmd(self, command: str, env: Optional[int] = None) -> REPLResult:
        """Send a command to the REPL.

        If env is None, uses the base env (Mathlib imported).
        If REPL is WARMING, waits until ready.
        If REPL is COLD, raises immediately.
        """
        await self._wait_ready()

        payload: dict = {"cmd": command}
        if env is not None:
            payload["env"] = env
        elif self._base_env is not None:
            payload["env"] = self._base_env

        response = await self._send_recv(payload)
        return self._parse_cmd_result(response)

    async def tactic(self, tactic_str: str, proof_state: int) -> REPLResult:
        """Send a tactic to the REPL in tactic mode."""
        await self._wait_ready()

        payload = {"tactic": tactic_str, "proofState": proof_state}
        response = await self._send_recv(payload)
        return self._parse_tactic_result(response)

    async def check_equality(
        self,
        lhs: str,
        rhs: str,
        variables: list[str],
        typ: str = "Int",
    ) -> REPLResult:
        """Convenience: check if lhs = rhs using first | ring | omega | norm_num | simp | decide.

        Returns REPLResult with .passed = True/False.
        """
        if "/" in lhs or "/" in rhs:
            typ = "ℚ"

        tactics = "first | ring | omega | norm_num | simp | decide"
        if variables:
            params = " ".join(f"({v} : {typ})" for v in variables)
            code = f"theorem check {params} : {lhs} = {rhs} := by {tactics}"
        else:
            code = f"theorem check : {lhs} = {rhs} := by {tactics}"

        result = await self.cmd(code)
        result.passed = not result.has_errors and len(result.sorries) == 0
        if not result.passed:
            errors = [m["data"] for m in result.messages if m.get("severity") == "error"]
            result.error = "; ".join(errors) if errors else "Unknown error"
        return result

    # --- Internal methods ---

    async def _wait_ready(self):
        """Wait for REPL to be ready, or raise if COLD/ERROR."""
        if self.state == REPLState.COLD:
            raise RuntimeError("REPL not started. Call start() first.")
        if self.state == REPLState.ERROR:
            raise RuntimeError("REPL in error state. Call restart().")
        if self.state == REPLState.WARMING:
            await self._ready_event.wait()
            if self.state != REPLState.READY:
                raise RuntimeError(f"REPL failed to warm up (state={self.state})")

    async def _spawn_process(self):
        """Launch `lake exe repl` as async subprocess."""
        lake = _find_lake()

        if not lake:
            raise RuntimeError("lake binary not found")

        self._process = await asyncio.create_subprocess_exec(
            lake, "exe", "repl",
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            cwd=str(_CHECKER_PROJECT),
            env=_lean_env(),
        )

    async def _import_mathlib(self) -> REPLResult:
        """Send the Mathlib import command (no env = fresh environment)."""
        payload = {"cmd": _MATHLIB_IMPORTS}
        # Mathlib import can take 15-90s on cold start
        response = await self._send_recv(payload, timeout=120.0)
        result = self._parse_cmd_result(response)
        if result.has_errors:
            errors = [m["data"] for m in result.messages if m.get("severity") == "error"]
            raise RuntimeError(f"Mathlib import failed: {'; '.join(errors)}")
        return result

    async def _send_recv(self, payload: dict, timeout: float = 60.0) -> dict:
        """Send JSON to REPL stdin, read JSON response from stdout.

        Protocol: JSON object followed by blank line. Response is JSON followed by blank line.
        """
        async with self._lock:
            if not self._process or self._process.returncode is not None:
                raise RuntimeError("REPL process is not running")

            # Send command + blank line separator
            data = json.dumps(payload) + "\n\n"
            self._process.stdin.write(data.encode("utf-8"))
            await self._process.stdin.drain()

            # Read response lines until we get a complete JSON object
            # The REPL outputs one JSON object per response, followed by a blank line
            lines = []
            while True:
                line = await asyncio.wait_for(
                    self._process.stdout.readline(),
                    timeout=timeout,
                )
                if not line:
                    # EOF — process died
                    stderr = ""
                    if self._process.stderr:
                        try:
                            stderr = (await asyncio.wait_for(
                                self._process.stderr.read(4096), timeout=2.0
                            )).decode("utf-8", errors="replace")
                        except asyncio.TimeoutError:
                            pass
                    raise RuntimeError(f"REPL process terminated unexpectedly. stderr: {stderr}")

                text = line.decode("utf-8").strip()
                if not text:
                    # Blank line = end of response
                    if lines:
                        break
                    continue
                lines.append(text)

            raw = "\n".join(lines)
            try:
                return json.loads(raw)
            except json.JSONDecodeError as e:
                raise RuntimeError(f"REPL returned invalid JSON: {raw[:500]}") from e

    def _parse_cmd_result(self, response: dict) -> REPLResult:
        return REPLResult(
            env=response.get("env"),
            messages=response.get("messages", []),
            sorries=response.get("sorries", []),
            raw=response,
        )

    def _parse_tactic_result(self, response: dict) -> REPLResult:
        return REPLResult(
            proof_state=response.get("proofState"),
            goals=response.get("goals"),
            messages=response.get("messages", []),
            raw=response,
        )
