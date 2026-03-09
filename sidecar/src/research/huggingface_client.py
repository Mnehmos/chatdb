"""HuggingFace Hub API client — model and dataset search.

API: https://huggingface.co/api
Rate limit: ~10 req/s (generous, optional token for private repos)
"""
import httpx
from typing import Optional, Dict, Any, List

from .config import rate_limiter, get_api_key

HF_API = "https://huggingface.co/api"


def _headers() -> Dict[str, str]:
    token = get_api_key("huggingface")
    if token:
        return {"Authorization": f"Bearer {token}"}
    return {}


async def search_models(
    query: str,
    max_results: int = 10,
    task: Optional[str] = None,
    sort: str = "downloads",
) -> Dict[str, Any]:
    """Search HuggingFace models.

    Args:
        query: Search query
        max_results: 1-50
        task: Filter by task (e.g., "text-generation", "theorem-proving")
        sort: Sort by "downloads", "likes", "trending", "lastModified"
    """
    await rate_limiter.acquire("huggingface")

    params: Dict[str, Any] = {
        "search": query,
        "limit": min(max(1, max_results), 50),
        "sort": sort,
        "direction": -1,
    }
    if task:
        params["pipeline_tag"] = task

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{HF_API}/models", params=params, headers=_headers()
        )
        resp.raise_for_status()
        models = resp.json()

    results = [
        {
            "model_id": m.get("modelId", m.get("id", "")),
            "author": m.get("author", ""),
            "pipeline_tag": m.get("pipeline_tag", ""),
            "downloads": m.get("downloads", 0),
            "likes": m.get("likes", 0),
            "tags": m.get("tags", [])[:10],
            "last_modified": m.get("lastModified", ""),
            "url": f"https://huggingface.co/{m.get('modelId', m.get('id', ''))}",
        }
        for m in models
    ]
    return {"query": query, "returned": len(results), "results": results}


async def search_datasets(
    query: str,
    max_results: int = 10,
    sort: str = "downloads",
) -> Dict[str, Any]:
    """Search HuggingFace datasets."""
    await rate_limiter.acquire("huggingface")

    params: Dict[str, Any] = {
        "search": query,
        "limit": min(max(1, max_results), 50),
        "sort": sort,
        "direction": -1,
    }

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{HF_API}/datasets", params=params, headers=_headers()
        )
        resp.raise_for_status()
        datasets = resp.json()

    results = [
        {
            "dataset_id": d.get("id", ""),
            "author": d.get("author", ""),
            "downloads": d.get("downloads", 0),
            "likes": d.get("likes", 0),
            "tags": d.get("tags", [])[:10],
            "last_modified": d.get("lastModified", ""),
            "url": f"https://huggingface.co/datasets/{d.get('id', '')}",
        }
        for d in datasets
    ]
    return {"query": query, "returned": len(results), "results": results}


async def get_model_info(model_id: str) -> Dict[str, Any]:
    """Get detailed info about a specific model."""
    await rate_limiter.acquire("huggingface")

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{HF_API}/models/{model_id}", headers=_headers()
        )
        resp.raise_for_status()
        m = resp.json()

    return {
        "model_id": m.get("modelId", m.get("id", "")),
        "author": m.get("author", ""),
        "pipeline_tag": m.get("pipeline_tag", ""),
        "downloads": m.get("downloads", 0),
        "likes": m.get("likes", 0),
        "tags": m.get("tags", []),
        "library_name": m.get("library_name", ""),
        "config": m.get("config", {}),
        "card_data": m.get("cardData", {}),
        "url": f"https://huggingface.co/{model_id}",
    }
