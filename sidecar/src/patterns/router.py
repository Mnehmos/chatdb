"""Pattern extraction endpoints."""
from fastapi import APIRouter
from pydantic import BaseModel
from typing import List

router = APIRouter()

class ExtractPatternRequest(BaseModel):
    problem_statement: str
    verified_steps: List[dict]
    domain: str

@router.post("/extract")
async def extract_patterns(req: ExtractPatternRequest):
    return []
