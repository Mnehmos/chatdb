"""PubMed / PMC API client — biomedical literature search.

API: https://eutils.ncbi.nlm.nih.gov/entrez/eutils/
Rate limit: 3 req/s (no key), 10 req/s (with NCBI API key)
"""
import httpx
import xml.etree.ElementTree as ET
from typing import Dict, Any, Optional, List

from .config import rate_limiter, get_api_key

EUTILS_BASE = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils"


def _api_params() -> Dict[str, str]:
    """Base params including API key if available."""
    key = get_api_key("ncbi")
    if key:
        return {"api_key": key}
    return {}


async def search(
    query: str,
    max_results: int = 10,
    sort: str = "relevance",
    db: str = "pubmed",
) -> Dict[str, Any]:
    """Search PubMed for articles.

    Args:
        query: PubMed search query (supports MeSH terms, field tags)
        max_results: 1-50
        sort: "relevance" or "pub_date" or "first_author"
        db: "pubmed" or "pmc" (full-text)
    """
    await rate_limiter.acquire("pubmed")

    # Step 1: esearch — get PMIDs
    params = {
        **_api_params(),
        "db": db,
        "term": query,
        "retmax": min(max(1, max_results), 50),
        "sort": sort,
        "retmode": "json",
    }

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(f"{EUTILS_BASE}/esearch.fcgi", params=params)
        resp.raise_for_status()
        search_data = resp.json()

    result = search_data.get("esearchresult", {})
    id_list = result.get("idlist", [])
    total = int(result.get("count", 0))

    if not id_list:
        return {"query": query, "total": total, "returned": 0, "results": []}

    # Step 2: efetch — get article details
    await rate_limiter.acquire("pubmed")
    fetch_params = {
        **_api_params(),
        "db": db,
        "id": ",".join(id_list),
        "retmode": "xml",
    }

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(f"{EUTILS_BASE}/efetch.fcgi", params=fetch_params)
        resp.raise_for_status()

    articles = _parse_pubmed_xml(resp.text)
    return {
        "query": query,
        "total": total,
        "returned": len(articles),
        "results": articles,
    }


def _parse_pubmed_xml(xml_text: str) -> List[Dict[str, Any]]:
    """Parse PubMed efetch XML into structured article data."""
    articles = []
    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError:
        return articles

    for article_elem in root.findall(".//PubmedArticle"):
        medline = article_elem.find("MedlineCitation")
        if medline is None:
            continue

        pmid_el = medline.find("PMID")
        pmid = pmid_el.text if pmid_el is not None else ""

        art = medline.find("Article")
        if art is None:
            continue

        title_el = art.find("ArticleTitle")
        title = title_el.text if title_el is not None else ""

        # Abstract
        abstract_parts = []
        abs_el = art.find("Abstract")
        if abs_el is not None:
            for at in abs_el.findall("AbstractText"):
                label = at.get("Label", "")
                text = at.text or ""
                if label:
                    abstract_parts.append(f"{label}: {text}")
                else:
                    abstract_parts.append(text)
        abstract = " ".join(abstract_parts)

        # Authors
        authors = []
        author_list = art.find("AuthorList")
        if author_list is not None:
            for auth in author_list.findall("Author"):
                last = auth.find("LastName")
                first = auth.find("ForeName")
                name = f"{first.text if first is not None else ''} {last.text if last is not None else ''}".strip()
                if name:
                    authors.append(name)

        # Journal + date
        journal_el = art.find("Journal")
        journal = ""
        year = ""
        if journal_el is not None:
            jt = journal_el.find("Title")
            journal = jt.text if jt is not None else ""
            ji = journal_el.find("JournalIssue")
            if ji is not None:
                pd = ji.find("PubDate")
                if pd is not None:
                    y = pd.find("Year")
                    year = y.text if y is not None else ""

        # DOI
        doi = ""
        for eid in art.findall("ELocationID"):
            if eid.get("EIdType") == "doi":
                doi = eid.text or ""
                break

        articles.append({
            "pmid": pmid,
            "title": title,
            "authors": authors[:10],
            "abstract": abstract[:1000],
            "journal": journal,
            "year": year,
            "doi": doi,
            "url": f"https://pubmed.ncbi.nlm.nih.gov/{pmid}/",
        })

    return articles


async def get_article(pmid: str) -> Dict[str, Any]:
    """Get a specific PubMed article by PMID."""
    await rate_limiter.acquire("pubmed")

    params = {
        **_api_params(),
        "db": "pubmed",
        "id": pmid,
        "retmode": "xml",
    }

    async with httpx.AsyncClient(timeout=15, follow_redirects=True) as client:
        resp = await client.get(f"{EUTILS_BASE}/efetch.fcgi", params=params)
        resp.raise_for_status()

    articles = _parse_pubmed_xml(resp.text)
    if not articles:
        return {"error": f"PMID {pmid} not found"}
    return articles[0]
