"""ChatDB Sidecar -- FastAPI server for validators and agents."""
import asyncio
import threading
from fastapi import FastAPI
from contextlib import asynccontextmanager
from .validation.router import router as validation_router
from .patterns.router import router as patterns_router
from .agents.router import router as agents_router
from .training_data.router import router as export_router
from .research.router import router as research_router
from .lean_repl.router import router as lean_router, start_repl, stop_repl
from .validation import lean_validator
from .validation.lean_validator import warmup_lean, _lean_available

@asynccontextmanager
async def lifespan(app: FastAPI):
    print("ChatDB Sidecar starting...")
    # Warm up the old subprocess validator in background thread
    if _lean_available():
        thread = threading.Thread(target=warmup_lean, daemon=True)
        thread.start()
    # Start the persistent Lean REPL (imports Mathlib once, stays warm)
    if _lean_available():
        asyncio.create_task(_start_repl_background())
    yield
    print("ChatDB Sidecar shutting down.")
    await stop_repl()


async def _start_repl_background():
    """Start REPL in background — log but don't crash on failure."""
    try:
        await start_repl()
        print("Lean REPL: ready")
    except Exception as e:
        print(f"Lean REPL: failed to start — {e}")

app = FastAPI(title="ChatDB Sidecar", version="0.1.0", lifespan=lifespan)
app.include_router(validation_router, prefix="/validate", tags=["validation"])
app.include_router(patterns_router, prefix="/patterns", tags=["patterns"])
app.include_router(agents_router, prefix="/agents", tags=["agents"])
app.include_router(export_router, prefix="/export", tags=["export"])
app.include_router(research_router, prefix="/research", tags=["research"])
app.include_router(lean_router, prefix="/lean", tags=["lean-repl"])

@app.get("/health")
async def health():
    from .lean_repl.router import get_repl
    with lean_validator._warmup_lock:
        warming = lean_validator.lean_warming_up
        ready = lean_validator.lean_warm
        attempts = lean_validator.warmup_attempts
    repl = get_repl()
    return {
        "status": "ok",
        "service": "chatdb-sidecar",
        "lean_available": _lean_available(),
        "lean_warming_up": warming,
        "lean_ready": ready,
        "lean_warmup_attempts": attempts,
        "lean_repl_state": repl.state.value,
    }
