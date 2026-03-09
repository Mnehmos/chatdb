"""Agent endpoints -- Council, Scout, Librarian."""
from fastapi import APIRouter
from pydantic import BaseModel
from typing import Optional, List
from .council import run_council_session
from .scout import run_scout_query
from .reconnaissance import run_reconnaissance
from .librarian import process_finding

router = APIRouter()

class CouncilRequest(BaseModel):
    trigger_type: str
    problem_id: str
    attempt_id: Optional[str] = None
    steps: List[dict]
    council_models: List[str] = ["claude-sonnet-4-5-20250929"]

@router.post("/council/deliberate")
async def council_deliberate(req: CouncilRequest):
    return await run_council_session(req)

class ScoutRequest(BaseModel):
    trigger_type: str  # "pre_solve", "mid_solve", "manual"
    query: str
    sources: List[str] = ["arxiv", "semantic_scholar"]
    domain: Optional[str] = None
    problem_id: Optional[str] = None
    max_results: int = 3

@router.post("/scout/query")
async def scout_query(req: ScoutRequest):
    return await run_scout_query(req)

class ReconnaissanceRequest(BaseModel):
    problem_statement: str
    problem_source: str = ""
    problem_title: Optional[str] = None
    domain: Optional[str] = None
    sources: List[str] = ["arxiv", "semantic_scholar", "wolfram_alpha", "oeis"]

@router.post("/scout/reconnaissance")
async def reconnaissance(req: ReconnaissanceRequest):
    return await run_reconnaissance(req)

class LibrarianRequest(BaseModel):
    finding: dict
    action_type: str

@router.post("/librarian/process")
async def librarian_process(req: LibrarianRequest):
    return await process_finding(req)
