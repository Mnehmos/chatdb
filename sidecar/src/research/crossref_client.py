"""Crossref API client — DOI resolution, metadata, reference chaining.

API: https://api.crossref.org/works
Rate limit: 50 req/s polite pool (with mailto), lower without
"""
import httpx
from typing import Optional, Dict, Any

from .config import rate_limiter, get_api_key

CROSSREF_API = "https://api.crossref.org/works"


def _headers() -> Dict[str, str]:
    email = get_api_key("unpaywall_email")  # reuse email for polite pool
    headers = {"User-Agent": "ChatDB/1.0 (https://github.com/chatdb)"}
    if email:
        headers["User-Agent"] = f"ChatDB/1.0 (mailto:{email})"
    return headers


def _parse_work(item: Dict) -> Dict[str, Any]:
    """Extract key fields from a Crossref work item."""
    authors = []
    for a in item.get("author", []):
        name = f"{a.get('given', '')} {a.get('family', '')}".strip()
        if name:
            authors.append(name)

    title_list = item.get("title", [])
    title = title_list[0] if title_list else ""

    # Published date — try multiple date fields
    pub_date = ""
    for date_field in ["published-print", "published-online", "created"]:
        dp = item.get(date_field, {}).get("date-parts", [[]])
        if dp and dp[0]:
            parts = dp[0]
            pub_date = "-".join(str(p) for p in parts if p)
            break

    return {
        "doi": item.get("DOI", ""),
        "title": title,
        "authors": authors,
        "published": pub_date,
        "type": item.get("type", ""),
        "container_title": (item.get("container-title", []) or [""])[0],
        "citation_count": item.get("is-referenced-by-count", 0),
        "reference_count": item.get("references-count", 0),
        "url": item.get("URL", ""),
        "abstract": item.get("abstract", ""),
    }


async def search(
    query: str,
    max_results: int = 10,
    sort: str = "relevance",
) -> Dict[str, Any]:
    """Search Crossref for works matching a query."""
    await rate_limiter.acquire("crossref")

    params: Dict[str, Any] = {
        "query": query,
        "rows": min(max(1, max_results), 50),
        "sort": sort,
    }

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(CROSSREF_API, params=params, headers=_headers())
        resp.raise_for_status()
        data = resp.json()

    items = data.get("message", {}).get("items", [])
    total = data.get("message", {}).get("total-results", len(items))
    results = [_parse_work(item) for item in items]
    return {
        "query": query,
        "total": total,
        "returned": len(results),
        "results": results,
    }


async def get_by_doi(doi: str) -> Dict[str, Any]:
    """Get metadata for a specific DOI."""
    await rate_limiter.acquire("crossref")

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{CROSSREF_API}/{doi}", headers=_headers()
        )
        resp.raise_for_status()
        data = resp.json()

    item = data.get("message", {})
    if not item:
        return {"error": f"DOI {doi} not found"}
    return _parse_work(item)
