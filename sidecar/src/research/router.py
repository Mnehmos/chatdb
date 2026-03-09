"""Research API router — unified endpoints for all research sources."""
from fastapi import APIRouter, HTTPException
from pydantic import BaseModel
from typing import Optional, List, Dict, Any

router = APIRouter()


# ── Request/Response models ──────────────────────────────────────────

class SearchRequest(BaseModel):
    source: str  # arxiv, semantic_scholar, oeis, crossref, openalex, wolfram, huggingface_models, huggingface_datasets, loogle, reservoir, pubmed
    query: str
    max_results: int = 10
    # Source-specific optional params
    sort: Optional[str] = None
    year: Optional[str] = None
    categories: Optional[List[str]] = None
    fields_of_study: Optional[List[str]] = None
    task: Optional[str] = None
    format_type: Optional[str] = None  # wolfram: "full" or "short"
    db: Optional[str] = None  # pubmed: "pubmed" or "pmc"

class GetRequest(BaseModel):
    source: str  # arxiv, semantic_scholar, oeis, crossref, unpaywall, openalex, huggingface, loogle, pubmed
    id: str  # paper ID, DOI, sequence ID, etc.

class CitationRequest(BaseModel):
    paper_id: str
    direction: str = "citations"  # "citations" or "references"
    max_results: int = 20


# ── Search endpoint ──────────────────────────────────────────────────

@router.post("/search")
async def research_search(req: SearchRequest) -> Dict[str, Any]:
    """Unified search across all research sources."""
    try:
        source = req.source.lower().replace("-", "_")

        if source == "arxiv":
            from .arxiv_client import search
            return await search(req.query, req.max_results, req.sort or "relevance", req.categories)

        elif source == "semantic_scholar":
            from .semantic_scholar import search
            return await search(req.query, req.max_results, req.year, req.fields_of_study)

        elif source == "oeis":
            from .oeis_client import search
            return await search(req.query, req.max_results)

        elif source == "crossref":
            from .crossref_client import search
            return await search(req.query, req.max_results, req.sort or "relevance")

        elif source == "openalex":
            from .openalex_client import search
            return await search(req.query, req.max_results, req.sort or "relevance_score:desc")

        elif source in ("wolfram", "wolfram_alpha"):
            from .wolfram_client import query
            return await query(req.query, req.format_type or "full")

        elif source in ("huggingface_models", "hf_models"):
            from .huggingface_client import search_models
            return await search_models(req.query, req.max_results, req.task, req.sort or "downloads")

        elif source in ("huggingface_datasets", "hf_datasets"):
            from .huggingface_client import search_datasets
            return await search_datasets(req.query, req.max_results, req.sort or "downloads")

        elif source == "loogle":
            from .lean_ecosystem import loogle_search
            return await loogle_search(req.query)

        elif source == "reservoir":
            from .lean_ecosystem import reservoir_search
            return await reservoir_search(req.query, req.max_results)

        elif source == "pubmed":
            from .pubmed_client import search
            return await search(req.query, req.max_results, req.sort or "relevance", req.db or "pubmed")

        elif source in ("tavily",):
            from .tavily_client import search
            return await search(req.query, req.max_results)

        elif source in ("google", "web", "web_search"):
            from .google_search import search
            return await search(req.query, req.max_results)

        elif source in ("math_stackexchange", "math.stackexchange", "mse"):
            from .stackexchange_client import search
            return await search(req.query, req.max_results, site="math")

        elif source in ("mathoverflow", "mo"):
            from .stackexchange_client import search
            return await search(req.query, req.max_results, site="mathoverflow")

        elif source in ("stackexchange_both", "mse_mo"):
            from .stackexchange_client import search_both
            return await search_both(req.query, req.max_results)

        else:
            raise HTTPException(400, f"Unknown source: {req.source}")

    except HTTPException:
        raise
    except Exception as e:
        return {"error": str(e), "source": req.source, "query": req.query}


# ── Get-by-ID endpoint ──────────────────────────────────────────────

@router.post("/get")
async def research_get(req: GetRequest) -> Dict[str, Any]:
    """Get a specific item by ID from a research source."""
    try:
        source = req.source.lower().replace("-", "_")

        if source == "arxiv":
            from .arxiv_client import get_paper
            return await get_paper(req.id)

        elif source == "semantic_scholar":
            from .semantic_scholar import get_paper
            return await get_paper(req.id)

        elif source == "oeis":
            from .oeis_client import get_sequence
            return await get_sequence(req.id)

        elif source == "crossref":
            from .crossref_client import get_by_doi
            return await get_by_doi(req.id)

        elif source == "unpaywall":
            from .unpaywall_client import get_pdf_url
            return await get_pdf_url(req.id)

        elif source == "openalex":
            from .openalex_client import get_work
            return await get_work(req.id)

        elif source in ("huggingface", "hf"):
            from .huggingface_client import get_model_info
            return await get_model_info(req.id)

        elif source == "pubmed":
            from .pubmed_client import get_article
            return await get_article(req.id)

        elif source in ("math_stackexchange", "mse"):
            from .stackexchange_client import get_question
            return await get_question(int(req.id), site="math")

        elif source in ("mathoverflow", "mo"):
            from .stackexchange_client import get_question
            return await get_question(int(req.id), site="mathoverflow")

        else:
            raise HTTPException(400, f"Unknown source for get: {req.source}")

    except HTTPException:
        raise
    except Exception as e:
        return {"error": str(e), "source": req.source, "id": req.id}


# ── Citations endpoint ───────────────────────────────────────────────

@router.post("/citations")
async def research_citations(req: CitationRequest) -> Dict[str, Any]:
    """Get citations or references for a paper (Semantic Scholar)."""
    try:
        from .semantic_scholar import get_citations, get_references

        if req.direction == "references":
            return await get_references(req.paper_id, req.max_results)
        else:
            return await get_citations(req.paper_id, req.max_results)

    except Exception as e:
        return {"error": str(e), "paper_id": req.paper_id}


# ── Multi-source search ─────────────────────────────────────────────

class MultiSearchRequest(BaseModel):
    query: str
    sources: List[str]  # e.g., ["arxiv", "semantic_scholar", "openalex"]
    max_results_per_source: int = 5

@router.post("/multi")
async def research_multi(req: MultiSearchRequest) -> Dict[str, Any]:
    """Search multiple sources in parallel."""
    import asyncio

    async def _search_one(source: str) -> Dict[str, Any]:
        try:
            sub = SearchRequest(source=source, query=req.query, max_results=req.max_results_per_source)
            return await research_search(sub)
        except Exception as e:
            return {"source": source, "error": str(e)}

    tasks = [_search_one(s) for s in req.sources]
    results = await asyncio.gather(*tasks, return_exceptions=True)

    combined = {}
    for source, result in zip(req.sources, results):
        if isinstance(result, Exception):
            combined[source] = {"error": str(result)}
        else:
            combined[source] = result

    return {"query": req.query, "sources": req.sources, "results": combined}


# ── Available sources ────────────────────────────────────────────────

@router.get("/sources")
async def list_sources() -> Dict[str, Any]:
    """List all available research sources and their capabilities."""
    return {
        "sources": [
            {"id": "arxiv", "name": "arXiv", "capabilities": ["search", "get"], "requires_key": False},
            {"id": "semantic_scholar", "name": "Semantic Scholar", "capabilities": ["search", "get", "citations"], "requires_key": False, "key_optional": True},
            {"id": "oeis", "name": "OEIS", "capabilities": ["search", "get"], "requires_key": False},
            {"id": "crossref", "name": "Crossref", "capabilities": ["search", "get"], "requires_key": False},
            {"id": "unpaywall", "name": "Unpaywall", "capabilities": ["get"], "requires_key": False, "requires_email": True},
            {"id": "openalex", "name": "OpenAlex", "capabilities": ["search", "get"], "requires_key": False, "key_optional": True},
            {"id": "wolfram_alpha", "name": "Wolfram|Alpha", "capabilities": ["search"], "requires_key": True},
            {"id": "huggingface_models", "name": "HuggingFace Models", "capabilities": ["search", "get"], "requires_key": False, "key_optional": True},
            {"id": "huggingface_datasets", "name": "HuggingFace Datasets", "capabilities": ["search"], "requires_key": False, "key_optional": True},
            {"id": "loogle", "name": "Loogle (Mathlib)", "capabilities": ["search"], "requires_key": False},
            {"id": "reservoir", "name": "Lean Reservoir", "capabilities": ["search"], "requires_key": False},
            {"id": "pubmed", "name": "PubMed", "capabilities": ["search", "get"], "requires_key": False, "key_optional": True},
            {"id": "math_stackexchange", "name": "Math StackExchange", "capabilities": ["search", "get"], "requires_key": False, "key_optional": True},
            {"id": "mathoverflow", "name": "MathOverflow", "capabilities": ["search", "get"], "requires_key": False, "key_optional": True},
            {"id": "tavily", "name": "Tavily Search", "capabilities": ["search"], "requires_key": True},
        ]
    }
