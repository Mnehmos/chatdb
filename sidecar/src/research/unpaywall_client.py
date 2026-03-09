"""Unpaywall API client — find open access PDF links for DOIs.

API: https://api.unpaywall.org/v2/{doi}
Rate limit: 10 req/s (requires email, no key)
"""
import httpx
from typing import Dict, Any, Optional

from .config import rate_limiter, get_api_key

UNPAYWALL_API = "https://api.unpaywall.org/v2"


async def get_pdf_url(doi: str) -> Dict[str, Any]:
    """Find open access PDF URL for a given DOI.

    Returns the best available OA location with PDF link.
    Requires UNPAYWALL_EMAIL to be configured.
    """
    await rate_limiter.acquire("unpaywall")

    email = get_api_key("unpaywall_email")
    if not email:
        return {"error": "Unpaywall requires an email address. Set UNPAYWALL_EMAIL or configure in settings."}

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(
            f"{UNPAYWALL_API}/{doi}",
            params={"email": email},
        )
        if resp.status_code == 404:
            return {"doi": doi, "is_oa": False, "pdf_url": None, "message": "DOI not found in Unpaywall"}
        resp.raise_for_status()
        data = resp.json()

    best_oa = data.get("best_oa_location") or {}
    oa_locations = data.get("oa_locations", [])

    # Find best PDF URL from OA locations
    pdf_url = best_oa.get("url_for_pdf") or best_oa.get("url")
    all_pdfs = [
        {
            "url": loc.get("url_for_pdf") or loc.get("url"),
            "host_type": loc.get("host_type"),
            "version": loc.get("version"),
            "license": loc.get("license"),
        }
        for loc in oa_locations
        if loc.get("url_for_pdf") or loc.get("url")
    ]

    return {
        "doi": doi,
        "title": data.get("title", ""),
        "is_oa": data.get("is_oa", False),
        "oa_status": data.get("oa_status", "closed"),
        "pdf_url": pdf_url,
        "all_locations": all_pdfs[:5],
        "journal": data.get("journal_name", ""),
        "year": data.get("year"),
    }
