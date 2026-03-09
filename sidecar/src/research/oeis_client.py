"""OEIS (Online Encyclopedia of Integer Sequences) client.

API: https://oeis.org/search?fmt=json
Rate limit: ~1 req/2s (no key required)
"""
import httpx
from typing import Dict, Any, List, Optional

from .config import rate_limiter

OEIS_API = "https://oeis.org/search"


def _parse_sequence(entry: Dict) -> Dict[str, Any]:
    """Extract key fields from an OEIS result entry."""
    return {
        "id": f"A{entry.get('number', 0):06d}",
        "name": entry.get("name", ""),
        "data": entry.get("data", ""),  # comma-separated values
        "formula": entry.get("formula", []),
        "comment": entry.get("comment", [])[:3],  # first 3 comments
        "reference": entry.get("reference", [])[:3],
        "link": entry.get("link", [])[:3],
        "offset": entry.get("offset", ""),
        "author": entry.get("author", ""),
        "url": f"https://oeis.org/A{entry.get('number', 0):06d}",
    }


async def search(
    query: str, max_results: int = 10
) -> Dict[str, Any]:
    """Search OEIS by keyword, formula, or sequence values.

    Args:
        query: Search term. Can be:
            - Keywords: "fibonacci", "catalan"
            - Sequence values: "1,1,2,3,5,8,13"
            - A-number: "A000045"
    """
    await rate_limiter.acquire("oeis")

    params = {"q": query, "fmt": "json", "start": 0}
    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(OEIS_API, params=params)
        resp.raise_for_status()
        data = resp.json()

    # OEIS returns either a list of results or a dict with "results" key
    if isinstance(data, list):
        results_raw = data
        total = len(data)
    else:
        results_raw = data.get("results", []) or []
        total = data.get("count", len(results_raw))

    results = [_parse_sequence(r) for r in results_raw[:max_results]]
    return {
        "query": query,
        "total": total,
        "returned": len(results),
        "results": results,
    }


async def get_sequence(sequence_id: str) -> Dict[str, Any]:
    """Get a specific OEIS sequence by A-number (e.g., 'A000045')."""
    await rate_limiter.acquire("oeis")

    # Normalize: accept "45", "A45", "A000045"
    if not sequence_id.startswith("A"):
        sequence_id = f"A{int(sequence_id):06d}"

    params = {"q": f"id:{sequence_id}", "fmt": "json"}
    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(OEIS_API, params=params)
        resp.raise_for_status()
        data = resp.json()

    results_raw = data if isinstance(data, list) else (data.get("results", []) or [])
    if not results_raw:
        return {"error": f"Sequence {sequence_id} not found"}
    return _parse_sequence(results_raw[0])
