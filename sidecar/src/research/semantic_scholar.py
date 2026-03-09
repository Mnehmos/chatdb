"""Semantic Scholar API client — paper search, citations, references.

API: https://api.semanticscholar.org/graph/v1
Rate limit: 1 req/s (no key), 10 req/s (with key)
"""
import httpx
from typing import Optional, List, Dict, Any

from .config import rate_limiter, get_api_key

S2_API = "https://api.semanticscholar.org/graph/v1"
FIELDS_PAPER = "title,abstract,year,authors,citationCount,referenceCount,url,externalIds,fieldsOfStudy,tldr"
FIELDS_CITATION = "title,year,authors,citationCount,url"


def _headers() -> Dict[str, str]:
    key = get_api_key("semantic_scholar")
    if key:
        return {"x-api-key": key}
    return {}


async def search(
    query: str,
    max_results: int = 10,
    year: Optional[str] = None,
    fields_of_study: Optional[List[str]] = None,
) -> Dict[str, Any]:
    """Search Semantic Scholar for papers.

    Args:
        query: Natural language search query
        max_results: 1-100
        year: Year filter (e.g., "2020-", "2018-2022", "2024")
        fields_of_study: Filter by field (e.g., ["Mathematics", "Computer Science"])
    """
    await rate_limiter.acquire("semantic_scholar")

    params: Dict[str, Any] = {
        "query": query,
        "limit": min(max(1, max_results), 100),
        "fields": FIELDS_PAPER,
    }
    if year:
        params["year"] = year
    if fields_of_study:
        params["fieldsOfStudy"] = ",".join(fields_of_study)

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{S2_API}/paper/search", params=params, headers=_headers()
        )
        resp.raise_for_status()
        data = resp.json()

    papers = data.get("data", [])
    return {
        "query": query,
        "total": data.get("total", len(papers)),
        "returned": len(papers),
        "results": papers,
    }


async def get_paper(paper_id: str) -> Dict[str, Any]:
    """Get paper details by Semantic Scholar ID, DOI, or arXiv ID.

    Accepts: S2 paper ID, DOI:xxx, ARXIV:xxx, PMID:xxx, etc.
    """
    await rate_limiter.acquire("semantic_scholar")

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{S2_API}/paper/{paper_id}",
            params={"fields": FIELDS_PAPER},
            headers=_headers(),
        )
        resp.raise_for_status()
        return resp.json()


async def get_citations(
    paper_id: str, max_results: int = 20
) -> Dict[str, Any]:
    """Get papers that cite the given paper."""
    await rate_limiter.acquire("semantic_scholar")

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{S2_API}/paper/{paper_id}/citations",
            params={"fields": FIELDS_CITATION, "limit": min(max_results, 100)},
            headers=_headers(),
        )
        resp.raise_for_status()
        data = resp.json()

    citing = [c.get("citingPaper", c) for c in data.get("data", [])]
    return {"paper_id": paper_id, "total": len(citing), "citations": citing}


async def get_references(
    paper_id: str, max_results: int = 20
) -> Dict[str, Any]:
    """Get papers referenced by the given paper."""
    await rate_limiter.acquire("semantic_scholar")

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{S2_API}/paper/{paper_id}/references",
            params={"fields": FIELDS_CITATION, "limit": min(max_results, 100)},
            headers=_headers(),
        )
        resp.raise_for_status()
        data = resp.json()

    refs = [r.get("citedPaper", r) for r in data.get("data", [])]
    return {"paper_id": paper_id, "total": len(refs), "references": refs}
