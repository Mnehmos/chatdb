"""Tavily Search API client — AI-optimized search with extracted content.

API: https://api.tavily.com
Returns clean, extracted page content — no scraping needed.
"""
import httpx
import logging
from typing import Any, Dict, List, Optional

from .config import rate_limiter, get_api_key

log = logging.getLogger(__name__)

TAVILY_API = "https://api.tavily.com"


async def search(
    query: str,
    max_results: int = 5,
    search_depth: str = "advanced",
    include_answer: bool = True,
    include_raw_content: bool = False,
    topic: str = "general",
) -> Dict[str, Any]:
    """Search via Tavily API.

    Args:
        query: Search string
        max_results: Number of results (1-10)
        search_depth: "basic" or "advanced" (advanced extracts more content)
        include_answer: Ask Tavily to generate a direct answer
        include_raw_content: Include full raw page content
        topic: "general" or "news"
    """
    await rate_limiter.acquire("tavily")

    api_key = get_api_key("tavily")
    if not api_key:
        return {"error": "No Tavily API key configured", "results": []}

    payload = {
        "api_key": api_key,
        "query": query,
        "max_results": min(max(1, max_results), 10),
        "search_depth": search_depth,
        "include_answer": include_answer,
        "include_raw_content": include_raw_content,
        "topic": topic,
    }

    async with httpx.AsyncClient(timeout=30.0) as client:
        resp = await client.post(
            f"{TAVILY_API}/search",
            json=payload,
            headers={
                "Content-Type": "application/json",
            },
        )
        resp.raise_for_status()
        data = resp.json()

    # Tavily returns: answer, results[{title, url, content, score}]
    tavily_answer = data.get("answer", "")
    raw_results = data.get("results", [])

    results: List[Dict[str, Any]] = []
    for item in raw_results:
        results.append({
            "title": item.get("title", ""),
            "link": item.get("url", ""),
            "abstract": item.get("content", ""),  # Tavily extracts page content
            "score": item.get("score", 0.0),
            "page_text": item.get("raw_content", ""),  # Only if include_raw_content
        })

    log.info("Tavily search for %r returned %d results (answer: %s)",
             query, len(results), bool(tavily_answer))

    return {
        "results": results,
        "total_results": len(results),
        "source": "tavily",
        "query": query,
        "tavily_answer": tavily_answer,
    }
