"""Scout Agent — pre-solve research briefing using the research API clients.

The scout runs ONCE before the proof loop starts.  Given a problem statement
and optional domain, it queries the most relevant research sources in parallel
and returns a compact briefing for injection into solver prompts.

Sources queried (based on domain):
  - Always: arXiv, Semantic Scholar
  - Math/number theory: OEIS, Loogle (Mathlib)
  - Physics/engineering: Wolfram|Alpha (if key available)
  - All: Crossref (for formal references)
"""

import asyncio
import logging
from typing import Any, Dict, List, Optional

log = logging.getLogger(__name__)

# Domain → which extra sources to query beyond the defaults (arxiv + semantic_scholar)
DOMAIN_SOURCES: Dict[str, List[str]] = {
    "number_theory": ["oeis", "loogle"],
    "algebra": ["loogle", "oeis"],
    "combinatorics": ["oeis", "loogle"],
    "analysis": ["loogle"],
    "geometry": ["loogle"],
    "topology": ["loogle"],
    "logic": ["loogle"],
    "general": [],
}

# Max results per source during briefing (keep it tight)
BRIEFING_MAX_PER_SOURCE = 3


async def run_scout_query(req) -> Dict[str, Any]:
    """Run a multi-source research query.

    Accepts the ScoutRequest model from router.py:
      - trigger_type: "pre_solve" | "mid_solve" | "manual"
      - query: the search query (usually the problem statement)
      - sources: list of source IDs to query
      - domain: optional problem domain for source selection
      - problem_id: optional problem ID for caching
    """
    from ..research.router import research_search, SearchRequest

    sources = getattr(req, "sources", None) or ["arxiv", "semantic_scholar"]
    domain = getattr(req, "domain", None) or ""
    max_results = getattr(req, "max_results", BRIEFING_MAX_PER_SOURCE)

    # If trigger_type is pre_solve, auto-select sources based on domain
    if getattr(req, "trigger_type", "") == "pre_solve" and domain:
        extras = DOMAIN_SOURCES.get(domain.lower().replace(" ", "_"), [])
        sources = list(dict.fromkeys(sources + extras))  # dedup, preserve order

    async def _query_source(source: str) -> Dict[str, Any]:
        try:
            sub = SearchRequest(
                source=source,
                query=req.query,
                max_results=max_results,
            )
            result = await research_search(sub)
            return {"source": source, **result}
        except Exception as e:
            log.warning("Scout: %s query failed: %s", source, e)
            return {"source": source, "error": str(e)}

    tasks = [_query_source(s) for s in sources]
    raw_results = await asyncio.gather(*tasks, return_exceptions=True)

    results = []
    for source, raw in zip(sources, raw_results):
        if isinstance(raw, Exception):
            results.append({"source": source, "error": str(raw)})
        else:
            results.append(raw)

    # Build a compact text briefing from structured results
    briefing = _build_briefing(req.query, results)

    return {
        "query": req.query,
        "domain": domain,
        "sources_queried": sources,
        "results_count": sum(_count_results(r) for r in results),
        "results": results,
        "briefing": briefing,
    }


def _count_results(r: Dict[str, Any]) -> int:
    """Count results from a source response."""
    if "error" in r:
        return 0
    if "results" in r:
        items = r["results"]
        return len(items) if isinstance(items, list) else 0
    if "total_results" in r:
        return min(r.get("total_results", 0), BRIEFING_MAX_PER_SOURCE)
    return 0


def _build_briefing(query: str, results: List[Dict[str, Any]]) -> str:
    """Produce a compact text briefing for solver prompt injection.

    Extracts paper titles, abstracts/summaries, theorem names, sequence names
    and formats them as a concise block that fits in a solver prompt.
    """
    lines: List[str] = []

    for r in results:
        source = r.get("source", "unknown")
        if "error" in r:
            continue

        items = r.get("results", [])
        if isinstance(items, dict):
            # Some sources return dicts (e.g., wolfram pods)
            items = [items]
        if not isinstance(items, list):
            continue

        for item in items[:BRIEFING_MAX_PER_SOURCE]:
            if not isinstance(item, dict):
                continue

            line = _format_item(source, item)
            if line:
                lines.append(line)

    if not lines:
        return ""

    return "RESEARCH CONTEXT (from automated literature search):\n" + "\n".join(lines)


def _format_item(source: str, item: Dict[str, Any]) -> Optional[str]:
    """Format a single research result into a one-line summary."""
    if source in ("arxiv", "semantic_scholar", "openalex", "crossref", "pubmed"):
        title = item.get("title", "")
        abstract = item.get("abstract") or item.get("summary", "")
        year = item.get("year") or item.get("published", "")
        if not title:
            return None
        # Truncate abstract to 150 chars
        if abstract and len(abstract) > 150:
            abstract = abstract[:147] + "..."
        parts = [f"[{source}]", title.strip()]
        if year:
            parts.append(f"({year})")
        if abstract:
            parts.append(f"— {abstract.strip()}")
        return " ".join(parts)

    elif source == "oeis":
        name = item.get("name", "")
        seq_id = item.get("id", "")
        values = item.get("values", "")
        if not name:
            return None
        parts = [f"[OEIS {seq_id}]", name.strip()]
        if values:
            parts.append(f"Values: {values[:60]}")
        return " ".join(parts)

    elif source == "loogle":
        name = item.get("name", "")
        type_str = item.get("type", "")
        doc = item.get("doc", "")
        if not name:
            return None
        parts = [f"[Mathlib]", name.strip()]
        if type_str:
            parts.append(f": {type_str[:100]}")
        if doc and len(doc) > 0:
            parts.append(f"— {doc[:100]}")
        return " ".join(parts)

    elif source in ("wolfram", "wolfram_alpha"):
        # Wolfram returns pods
        pods = item.get("pods", [])
        if isinstance(pods, list) and pods:
            first = pods[0] if isinstance(pods[0], dict) else {}
            title = first.get("title", "")
            text = first.get("plaintext", "")
            if text:
                return f"[Wolfram] {title}: {text[:150]}"
        answer = item.get("short_answer") or item.get("result", "")
        if answer:
            return f"[Wolfram] {str(answer)[:200]}"
        return None

    elif source == "reservoir":
        name = item.get("name", "")
        desc = item.get("description", "")
        if not name:
            return None
        return f"[Lean pkg] {name}: {desc[:100]}" if desc else f"[Lean pkg] {name}"

    else:
        # Generic fallback
        title = item.get("title") or item.get("name", "")
        if title:
            return f"[{source}] {title[:150]}"
        return None
