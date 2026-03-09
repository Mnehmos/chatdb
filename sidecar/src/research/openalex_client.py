"""OpenAlex API client — works, concepts, citation analysis.

API: https://api.openalex.org
Rate limit: 10 req/s polite pool (with mailto), lower without
"""
import httpx
from typing import Optional, Dict, Any, List

from .config import rate_limiter, get_api_key

OPENALEX_API = "https://api.openalex.org"


def _params_base() -> Dict[str, str]:
    """Base params including polite-pool email if available."""
    email = get_api_key("openalex")
    if email:
        return {"mailto": email}
    return {}


def _parse_work(item: Dict) -> Dict[str, Any]:
    """Extract key fields from an OpenAlex work."""
    authorships = item.get("authorships", [])
    authors = [
        a.get("author", {}).get("display_name", "")
        for a in authorships[:10]
    ]

    concepts = [
        {"name": c.get("display_name", ""), "score": c.get("score", 0)}
        for c in item.get("concepts", [])[:5]
    ]

    return {
        "openalex_id": item.get("id", ""),
        "doi": item.get("doi", ""),
        "title": item.get("title", ""),
        "authors": authors,
        "year": item.get("publication_year"),
        "type": item.get("type", ""),
        "citation_count": item.get("cited_by_count", 0),
        "concepts": concepts,
        "open_access": item.get("open_access", {}),
        "abstract": item.get("abstract_inverted_index") is not None,
    }


async def search(
    query: str,
    max_results: int = 10,
    sort: str = "relevance_score:desc",
    filter_params: Optional[Dict[str, str]] = None,
) -> Dict[str, Any]:
    """Search OpenAlex for works.

    Args:
        query: Search query
        max_results: 1-50
        sort: Sort order (e.g., "cited_by_count:desc", "publication_year:desc")
        filter_params: OpenAlex filter params (e.g., {"from_publication_date": "2020-01-01"})
    """
    await rate_limiter.acquire("openalex")

    params = {
        **_params_base(),
        "search": query,
        "per_page": min(max(1, max_results), 50),
        "sort": sort,
    }

    # Build filter string
    if filter_params:
        filter_parts = [f"{k}:{v}" for k, v in filter_params.items()]
        params["filter"] = ",".join(filter_parts)

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(f"{OPENALEX_API}/works", params=params)
        resp.raise_for_status()
        data = resp.json()

    items = data.get("results", [])
    results = [_parse_work(item) for item in items]
    meta = data.get("meta", {})
    return {
        "query": query,
        "total": meta.get("count", len(results)),
        "returned": len(results),
        "results": results,
    }


async def get_work(work_id: str) -> Dict[str, Any]:
    """Get a specific work by OpenAlex ID or DOI."""
    await rate_limiter.acquire("openalex")

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{OPENALEX_API}/works/{work_id}", params=_params_base()
        )
        resp.raise_for_status()
        return _parse_work(resp.json())


async def get_concept(concept_name: str) -> Dict[str, Any]:
    """Search for a concept/topic and get related works."""
    await rate_limiter.acquire("openalex")

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{OPENALEX_API}/concepts",
            params={**_params_base(), "search": concept_name, "per_page": 5},
        )
        resp.raise_for_status()
        data = resp.json()

    results = data.get("results", [])
    concepts = [
        {
            "id": c.get("id", ""),
            "name": c.get("display_name", ""),
            "description": c.get("description", ""),
            "works_count": c.get("works_count", 0),
            "cited_by_count": c.get("cited_by_count", 0),
            "level": c.get("level", 0),
        }
        for c in results
    ]
    return {"query": concept_name, "concepts": concepts}
