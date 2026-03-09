"""StackExchange API client — Math StackExchange + MathOverflow search.

API: https://api.stackexchange.com/2.3
Rate limit: 300 req/day without key, 10000 req/day with key
"""
import html
import httpx
import re
from typing import Any, Dict, List, Optional

from .config import rate_limiter, get_api_key

SE_API = "https://api.stackexchange.com/2.3"

# Supported math sites
MATH_SITES = {
    "math_stackexchange": "math",
    "mathoverflow": "mathoverflow",
    "math.stackexchange": "math",
    "math.stackexchange.com": "math",
    "mathoverflow.net": "mathoverflow",
}


def _clean_html(text: str) -> str:
    """Strip HTML tags and decode entities."""
    text = html.unescape(text)
    text = re.sub(r"<[^>]+>", " ", text)
    text = re.sub(r"\s+", " ", text)
    return text.strip()


def _parse_question(item: Dict) -> Dict[str, Any]:
    """Extract key fields from a SE question."""
    title = html.unescape(item.get("title", ""))
    body = _clean_html(item.get("body", ""))

    tags = item.get("tags", [])
    owner = item.get("owner", {})

    result = {
        "question_id": item.get("question_id"),
        "title": title,
        "abstract": body[:500] if body else "",
        "tags": tags,
        "score": item.get("score", 0),
        "answer_count": item.get("answer_count", 0),
        "view_count": item.get("view_count", 0),
        "is_answered": item.get("is_answered", False),
        "link": item.get("link", ""),
        "author": owner.get("display_name", ""),
        "creation_date": item.get("creation_date"),
    }

    # Include accepted answer body if present (via filter)
    answers = item.get("answers", [])
    if answers:
        # Sort by score descending, prefer accepted
        answers.sort(key=lambda a: (a.get("is_accepted", False), a.get("score", 0)), reverse=True)
        best = answers[0]
        best_body = _clean_html(best.get("body", ""))
        result["best_answer"] = best_body[:1000]
        result["best_answer_score"] = best.get("score", 0)
        result["best_answer_accepted"] = best.get("is_accepted", False)

    return result


async def search(
    query: str,
    max_results: int = 5,
    site: str = "math",
    sort: str = "relevance",
    tagged: Optional[str] = None,
) -> Dict[str, Any]:
    """Search a StackExchange site for questions matching a query.

    Args:
        query: Search string
        max_results: Number of results (1-30)
        site: SE site name — "math" for MSE, "mathoverflow" for MO
        sort: One of "relevance", "activity", "votes", "creation"
        tagged: Comma-separated tags to filter by
    """
    await rate_limiter.acquire("stackexchange")

    # Resolve site alias
    site = MATH_SITES.get(site, site)

    params: Dict[str, Any] = {
        "order": "desc",
        "sort": sort,
        "site": site,
        "q": query,
        "pagesize": min(max(1, max_results), 30),
    }

    api_key = get_api_key("stackexchange")
    if api_key:
        params["key"] = api_key

    if tagged:
        params["tagged"] = tagged

    async with httpx.AsyncClient(timeout=15) as client:
        # Try search/excerpts first — does full-text search and returns snippets
        resp = await client.get(f"{SE_API}/search/excerpts", params=params)
        resp.raise_for_status()
        data = resp.json()

    items = data.get("items", [])
    quota_remaining = data.get("quota_remaining", "?")

    # search/excerpts returns excerpt items, not full questions
    results = []
    for item in items:
        title = _clean_html(item.get("title", ""))
        body = _clean_html(item.get("excerpt", "") or item.get("body", ""))
        question_id = item.get("question_id")
        results.append({
            "question_id": question_id,
            "title": title,
            "abstract": body[:500] if body else "",
            "tags": item.get("tags", []),
            "score": item.get("score", 0),
            "is_answered": item.get("has_accepted_answer", False),
            "answer_count": item.get("answer_count", 0),
            "link": f"https://{site}.stackexchange.com/q/{question_id}" if question_id else "",
            "item_type": item.get("item_type", "question"),
        })

    return {
        "query": query,
        "site": site,
        "total_results": len(results),
        "quota_remaining": quota_remaining,
        "results": results,
    }


async def search_both(
    query: str,
    max_results: int = 3,
) -> Dict[str, Any]:
    """Search both Math StackExchange and MathOverflow in parallel."""
    import asyncio

    mse_task = search(query, max_results, site="math")
    mo_task = search(query, max_results, site="mathoverflow")

    mse_result, mo_result = await asyncio.gather(mse_task, mo_task, return_exceptions=True)

    combined_results = []
    for label, result in [("math.stackexchange", mse_result), ("mathoverflow", mo_result)]:
        if isinstance(result, Exception):
            continue
        for item in result.get("results", []):
            item["site"] = label
            combined_results.append(item)

    # Sort by score descending
    combined_results.sort(key=lambda x: x.get("score", 0), reverse=True)

    return {
        "query": query,
        "total_results": len(combined_results),
        "results": combined_results[:max_results * 2],
    }


async def get_question(
    question_id: int,
    site: str = "math",
) -> Dict[str, Any]:
    """Get a specific question with its answers."""
    await rate_limiter.acquire("stackexchange")

    site = MATH_SITES.get(site, site)

    params: Dict[str, Any] = {
        "site": site,
        "filter": "!nNPvSNdWme",  # Include body + answers with body
    }

    api_key = get_api_key("stackexchange")
    if api_key:
        params["key"] = api_key

    async with httpx.AsyncClient(timeout=15) as client:
        resp = await client.get(f"{SE_API}/questions/{question_id}", params=params)
        resp.raise_for_status()
        data = resp.json()

    items = data.get("items", [])
    if not items:
        return {"error": f"Question {question_id} not found on {site}"}

    return _parse_question(items[0])
