"""Wolfram|Alpha API client — computational knowledge queries.

API: https://api.wolframalpha.com/v2/query (Full Results)
      https://api.wolframalpha.com/v1/result (Short Answers)
Rate limit: 2000/month free tier (requires AppID)
"""
import httpx
import xml.etree.ElementTree as ET
from typing import Dict, Any, List, Optional

from .config import rate_limiter, get_api_key

WOLFRAM_FULL_API = "https://api.wolframalpha.com/v2/query"
WOLFRAM_SHORT_API = "https://api.wolframalpha.com/v1/result"


async def query(
    input_text: str,
    format_type: str = "full",
) -> Dict[str, Any]:
    """Query Wolfram|Alpha.

    Args:
        input_text: The query (e.g., "solve x^2 + 2x + 1 = 0", "integral of sin(x)")
        format_type: "full" for structured pods, "short" for plain-text answer
    """
    await rate_limiter.acquire("wolfram_alpha")

    app_id = get_api_key("wolfram_alpha")
    if not app_id:
        return {"error": "Wolfram|Alpha requires an AppID. Set WOLFRAM_APP_ID or configure in settings."}

    if format_type == "short":
        return await _query_short(input_text, app_id)
    return await _query_full(input_text, app_id)


async def _query_short(input_text: str, app_id: str) -> Dict[str, Any]:
    """Short answer API — returns a single text result."""
    async with httpx.AsyncClient(timeout=20, follow_redirects=True) as client:
        resp = await client.get(
            WOLFRAM_SHORT_API,
            params={"appid": app_id, "i": input_text},
        )
        if resp.status_code == 501:
            return {"input": input_text, "answer": None, "error": "Wolfram|Alpha couldn't interpret the query"}
        resp.raise_for_status()
        return {"input": input_text, "answer": resp.text}


async def _query_full(input_text: str, app_id: str) -> Dict[str, Any]:
    """Full results API — returns structured pods with multiple result types."""
    async with httpx.AsyncClient(timeout=20, follow_redirects=True) as client:
        resp = await client.get(
            WOLFRAM_FULL_API,
            params={
                "appid": app_id,
                "input": input_text,
                "output": "xml",
                "format": "plaintext",
            },
        )
        resp.raise_for_status()

    root = ET.fromstring(resp.text)
    success = root.get("success") == "true"

    if not success:
        # Check for suggestions
        tips = root.findall(".//tip")
        suggestions = [t.get("text", "") for t in tips]
        return {
            "input": input_text,
            "success": False,
            "pods": [],
            "suggestions": suggestions,
        }

    pods: List[Dict[str, Any]] = []
    for pod in root.findall("pod"):
        subpods = []
        for sp in pod.findall("subpod"):
            plaintext = sp.find("plaintext")
            text = plaintext.text if plaintext is not None and plaintext.text else ""
            subpods.append({"title": sp.get("title", ""), "text": text})

        pods.append({
            "title": pod.get("title", ""),
            "scanner": pod.get("scanner", ""),
            "id": pod.get("id", ""),
            "primary": pod.get("primary") == "true",
            "subpods": subpods,
        })

    return {
        "input": input_text,
        "success": True,
        "pods": pods,
    }
