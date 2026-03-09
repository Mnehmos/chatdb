"""Web search via DuckDuckGo — no API key required."""
import asyncio
import logging
from typing import Any, Dict, List

log = logging.getLogger(__name__)


async def search(query: str, max_results: int = 5) -> Dict[str, Any]:
    """Search via DuckDuckGo and return results.

    Uses the ddgs library which handles rate limits and JS challenges.
    No API key required.
    """
    from ddgs import DDGS

    def _sync_search():
        with DDGS() as ddgs:
            return list(ddgs.text(query, max_results=max_results))

    # Run synchronous search in thread pool
    loop = asyncio.get_event_loop()
    raw = await loop.run_in_executor(None, _sync_search)

    results: List[Dict[str, str]] = []
    for item in raw:
        results.append({
            "title": item.get("title", ""),
            "link": item.get("href", ""),
            "abstract": item.get("body", ""),  # "abstract" for _extract_answers compat
        })

    log.info("Web search for %r returned %d results", query, len(results))

    return {
        "results": results,
        "total_results": len(results),
        "source": "google",
        "query": query,
    }
