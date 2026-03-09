"""Training data export endpoints."""
from fastapi import APIRouter
from typing import List

router = APIRouter()

@router.get("/training_data/stats")
async def training_data_stats():
    return {"layers": {f"layer_{i}": {"count": 0} for i in range(1, 8)}}

@router.post("/training_data/export")
async def export_training_data(layers: List[int] = [1, 2], format: str = "jsonl"):
    return {"status": "not_implemented", "layers": layers, "format": format}
