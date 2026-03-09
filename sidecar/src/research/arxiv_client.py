"""arXiv API client — search, metadata, PDF download links.

API: https://export.arxiv.org/api/query
Rate limit: 1 req / 3 seconds (no key required)
"""
import httpx
import xml.etree.ElementTree as ET
from typing import Optional, List, Dict, Any

from .config import rate_limiter

ARXIV_API = "https://export.arxiv.org/api/query"
NS = {"atom": "http://www.w3.org/2005/Atom", "arxiv": "http://arxiv.org/schemas/atom"}


def _parse_entry(entry: ET.Element) -> Dict[str, Any]:
    """Parse a single Atom entry into a dict."""
    def text(tag: str, ns: str = "atom") -> str:
        el = entry.find(f"{ns}:{tag}", NS) if ns else entry.find(tag)
        return el.text.strip() if el is not None and el.text else ""

    authors = [
        a.find("atom:name", NS).text
        for a in entry.findall("atom:author", NS)
        if a.find("atom:name", NS) is not None
    ]

    # Extract arXiv ID from the entry id URL
    entry_id = text("id")
    arxiv_id = entry_id.rsplit("/abs/", 1)[-1] if "/abs/" in entry_id else entry_id

    categories = [
        c.get("term", "")
        for c in entry.findall("atom:category", NS)
    ]

    return {
        "arxiv_id": arxiv_id,
        "title": " ".join(text("title").split()),
        "authors": authors,
        "abstract": " ".join(text("summary").split()),
        "published": text("published"),
        "updated": text("updated"),
        "categories": categories,
        "pdf_url": f"https://arxiv.org/pdf/{arxiv_id}",
        "abs_url": f"https://arxiv.org/abs/{arxiv_id}",
    }


async def search(
    query: str,
    max_results: int = 10,
    sort_by: str = "relevance",
    categories: Optional[List[str]] = None,
) -> Dict[str, Any]:
    """Search arXiv for papers matching a query.

    Args:
        query: Search query (supports arXiv query syntax: au:, ti:, abs:, cat:)
        max_results: Maximum number of results (1-50)
        sort_by: "relevance" or "lastUpdatedDate" or "submittedDate"
        categories: Optional category filter (e.g., ["math.CO", "cs.AI"])
    """
    await rate_limiter.acquire("arxiv")

    max_results = min(max(1, max_results), 50)

    search_query = f"all:{query}"
    if categories:
        cat_filter = " OR ".join(f"cat:{c}" for c in categories)
        search_query = f"({search_query}) AND ({cat_filter})"

    sort_map = {
        "relevance": "relevance",
        "lastUpdatedDate": "lastUpdatedDate",
        "submittedDate": "submittedDate",
    }

    params = {
        "search_query": search_query,
        "max_results": max_results,
        "sortBy": sort_map.get(sort_by, "relevance"),
        "sortOrder": "descending",
    }

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(ARXIV_API, params=params)
        resp.raise_for_status()

    root = ET.fromstring(resp.text)
    entries = root.findall("atom:entry", NS)

    results = [_parse_entry(e) for e in entries]
    total = root.find("{http://a9.com/-/spec/opensearch/1.1/}totalResults")
    total_count = int(total.text) if total is not None and total.text else len(results)

    return {
        "query": query,
        "total_results": total_count,
        "returned": len(results),
        "results": results,
    }


async def get_paper(arxiv_id: str) -> Dict[str, Any]:
    """Get metadata for a specific arXiv paper by ID."""
    await rate_limiter.acquire("arxiv")

    params = {"id_list": arxiv_id, "max_results": 1}
    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(ARXIV_API, params=params)
        resp.raise_for_status()

    root = ET.fromstring(resp.text)
    entries = root.findall("atom:entry", NS)
    if not entries:
        return {"error": f"Paper {arxiv_id} not found"}

    return _parse_entry(entries[0])
