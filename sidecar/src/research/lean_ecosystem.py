"""Lean ecosystem clients — Loogle (Mathlib search) and Reservoir.

Loogle API: https://loogle.lean-lang.org/json
Reservoir: https://reservoir.lean-lang.org/api/v1
Rate limit: ~1 req/2s (no key required)
"""
import httpx
from typing import Dict, Any, Optional

from .config import rate_limiter

LOOGLE_API = "https://loogle.lean-lang.org"
RESERVOIR_API = "https://reservoir.lean-lang.org/api/v1"


async def loogle_search(query: str) -> Dict[str, Any]:
    """Search Lean 4 Mathlib via Loogle.

    Args:
        query: Loogle query syntax, e.g.:
            - Type signature: "Nat -> Nat -> Nat"
            - Name pattern: "List.map"
            - Combined: "_ + _ = _ + _"
    """
    await rate_limiter.acquire("loogle")

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{LOOGLE_API}/json",
            params={"q": query},
        )
        resp.raise_for_status()
        data = resp.json()

    hits = data.get("hits", [])
    results = [
        {
            "name": h.get("name", ""),
            "type": h.get("type", ""),
            "module": h.get("module", ""),
            "doc": h.get("doc", ""),
            "url": f"https://leanprover-community.github.io/mathlib4_docs/{h.get('module', '').replace('.', '/')}.html#{h.get('name', '')}",
        }
        for h in hits[:20]
    ]

    return {
        "query": query,
        "total": len(hits),
        "returned": len(results),
        "results": results,
        "suggestion": data.get("suggestion"),
        "error": data.get("error"),
    }


async def reservoir_search(
    query: str, max_results: int = 10
) -> Dict[str, Any]:
    """Search Lean 4 package index (Reservoir).

    Args:
        query: Package name or keyword search
    """
    await rate_limiter.acquire("loogle")  # same rate limit

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{RESERVOIR_API}/packages",
            params={"q": query},
        )
        resp.raise_for_status()
        data = resp.json()

    packages = data if isinstance(data, list) else data.get("packages", [])
    results = [
        {
            "name": p.get("name", ""),
            "description": p.get("description", ""),
            "owner": p.get("owner", ""),
            "stars": p.get("stars", 0),
            "url": p.get("homepage") or p.get("url", ""),
            "updated_at": p.get("updatedAt", ""),
        }
        for p in packages[:max_results]
    ]

    return {
        "query": query,
        "returned": len(results),
        "results": results,
    }
