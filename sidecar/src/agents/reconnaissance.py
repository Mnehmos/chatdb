"""Pre-decomposition reconnaissance agent.

Searches for THIS EXACT PROBLEM's answer before the decomposer creates
an obligation graph.  Different from the scout: the scout searches for
related literature to inject into solver prompts.  Reconnaissance
searches for "does anyone already know the answer?" and feeds findings
into the decomposer prompt so it builds obligations around the correct
target.

Query strategies:
  1. Competition query  — regex-detect IMO/USAMO/Putnam + year + problem# in source
  2. Object query       — extract key mathematical terms from the problem statement
  3. Wolfram probe      — computational query if problem asks for a constant/count
  4. OEIS probe         — sequence lookup if integers/sequences involved
  5. Web search         — DuckDuckGo for AoPS, blogs, competition forums

Answer extraction:
  - Regex patterns for quick extraction from snippets
  - LLM call (cheap model) to read search results in context and extract the answer
"""

import asyncio
import logging
import os
import re
from collections import Counter
from typing import Any, Dict, List, Optional, Tuple

log = logging.getLogger(__name__)

# Regex patterns for competition detection
_COMPETITION_RE = re.compile(
    r"(IMO|USAMO|Putnam|APMO|EGMO|ISL|BMO|USAJMO|AIME|AMC)\s*"
    r"(\d{4})\s*"
    r"(?:Problem\s*|P|#)?\s*(\d+)?",
    re.IGNORECASE,
)

# Patterns to extract answers from text
_ANSWER_PATTERNS = [
    # "the answer is c = 4", "answer: 4", "the answer is 4"
    re.compile(r"(?:the\s+)?answer\s+is\s+(?:[a-z]\s*=\s*)?([^\s,.;]+(?:/[^\s,.;]+)?)", re.IGNORECASE),
    # "answer: 4" or "answer = 4"
    re.compile(r"answer\s*[:=]\s*([^\s,.;]+(?:/[^\s,.;]+)?)", re.IGNORECASE),
    # "c = 4" or "c=4" in context of "constant", "optimal", "smallest", "value"
    re.compile(r"(?:constant|optimal|smallest|largest|maximum|minimum|value)\s+(?:is\s+)?(?:[a-z]\s*=\s*)?(\d+(?:/\d+)?)", re.IGNORECASE),
    # Standalone "c = 4" pattern (common in AoPS/forum posts)
    re.compile(r"\bc\s*=\s*(\d+(?:/\d+)?)\b", re.IGNORECASE),
    # "equals 4", "= 4" near end of sentence
    re.compile(r"(?:equals?|=)\s+(\d+(?:/\d+)?)\s*(?:\.|,|$)", re.IGNORECASE),
]

# Keywords that suggest a problem is asking for a specific value
_VALUE_KEYWORDS = {"determine", "find", "smallest", "largest", "maximum", "minimum",
                   "compute", "calculate", "evaluate", "what is", "constant"}


def _strip_latex(text: str) -> str:
    """Strip LaTeX markup from text for search queries."""
    # Remove $...$ wrappers
    text = re.sub(r"\$([^$]*)\$", r"\1", text)
    # Remove common LaTeX commands
    text = re.sub(r"\\(?:mathbb|mathrm|text|operatorname|mathcal)\{([^}]*)\}", r"\1", text)
    text = re.sub(r"\\(?:rightarrow|to|implies|iff)", "->", text)
    text = re.sub(r"\\(?:leq|le)", "<=", text)
    text = re.sub(r"\\(?:geq|ge)", ">=", text)
    text = re.sub(r"\\(?:neq|ne)", "!=", text)
    text = re.sub(r"\\(?:cdot|times)", "*", text)
    text = re.sub(r"\\(?:infty)", "infinity", text)
    text = re.sub(r"\\(?:sum|prod|int|lim)", " ", text)
    # Remove remaining backslash commands
    text = re.sub(r"\\[a-zA-Z]+", " ", text)
    # Remove braces
    text = re.sub(r"[{}]", "", text)
    # Clean up whitespace
    text = re.sub(r"\s+", " ", text)
    return text.strip()


def _build_recon_queries(
    problem_statement: str,
    source: str,
    title: Optional[str],
    domain: Optional[str],
) -> List[Dict[str, str]]:
    """Build reconnaissance queries from problem metadata.

    Strategy: search the problem in natural language. Let web search
    find AoPS threads, blog posts, solutions. Let the LLM read the
    pages and extract the answer. Don't over-engineer query construction.

    Returns list of {"source": source_id, "query": search_string} dicts.
    """
    queries: List[Dict[str, str]] = []
    clean_stmt = _strip_latex(problem_statement)
    combined_text = f"{source or ''} {title or ''} {clean_stmt}"

    # 1. Competition query — if we detect IMO/USAMO/etc., use the short tag
    match = _COMPETITION_RE.search(combined_text)
    if match:
        comp_name = match.group(1).upper()
        year = match.group(2)
        prob_num = match.group(3) or ""
        comp_query = f"{comp_name} {year}"
        if prob_num:
            comp_query += f" Problem {prob_num}"
        queries.append({"source": "google", "query": f"{comp_query} solution answer"})
        queries.append({"source": "math_stackexchange", "query": comp_query})

    # 2. Direct natural-language search via Tavily — the core approach
    # Tavily returns extracted page content, no scraping needed
    # Use the full problem statement (Tavily handles long queries well)
    if clean_stmt:
        queries.append({"source": "tavily", "query": clean_stmt[:400]})

    # 3. Title search — if we have a title, search that too
    if title:
        queries.append({"source": "tavily", "query": f"{title} answer solution"})

    return queries


async def _llm_extract_answer(
    problem_statement: str,
    search_snippets: List[Dict[str, str]],
) -> Tuple[Optional[str], float]:
    """Use a cheap LLM call to extract the answer from search results.

    Returns (answer, confidence).
    """
    import httpx

    # Prefer OpenRouter (cheapest), fall back to OpenAI
    api_key = os.environ.get("OPENROUTER_API_KEY", "")
    if api_key:
        base_url = "https://openrouter.ai/api/v1"
        model = "anthropic/claude-haiku-4.5"
    else:
        api_key = os.environ.get("OPENAI_API_KEY", "")
        base_url = "https://api.openai.com/v1"
        model = "gpt-4o-mini"

    if not api_key:
        log.warning("No LLM API key available for answer extraction")
        return None, 0.0

    # Build the evidence block — give the LLM enough content to find answers
    evidence_lines = []
    for i, s in enumerate(search_snippets[:12]):
        title = s.get("title", "")
        body = s.get("body", "")[:2000]  # Enough to capture answer context
        if title or body:
            evidence_lines.append(f"[{i+1}] {title}\n{body}")

    if not evidence_lines:
        return None, 0.0

    evidence = "\n---\n".join(evidence_lines)

    prompt = f"""You are extracting the FINAL ANSWER to a specific math competition problem from web search results and fetched page content.

PROBLEM: {problem_statement[:800]}

SEARCH RESULTS AND PAGE CONTENT:
{evidence}

TASK: What is the final numerical answer to this problem?
- Look for phrases like "the answer is", "equals", "the smallest constant is", "c = ", "the value is", or any explicit statement of the answer.
- The answer may appear in ANY language (French, Chinese, etc.) — look for patterns like "c=4" or "$c=4$" in LaTeX.
- Competition problems (IMO, USAMO, Putnam) typically have a clean numerical answer.
- If you find the answer stated clearly in ANY result, extract it.
- Report JUST the numerical value (e.g., "4", "3/2", "42"), not "c = 4".
- If results discuss the problem but don't state an answer, say "none".
- If results are about a DIFFERENT problem, say "none".

Respond with ONLY a JSON object:
{{"answer": "<numerical value or 'none'>", "confidence": <0.0-1.0>, "reasoning": "<one sentence>"}}"""

    try:
        async with httpx.AsyncClient(timeout=15.0) as client:
            resp = await client.post(
                f"{base_url}/chat/completions",
                headers={
                    "Authorization": f"Bearer {api_key}",
                    "Content-Type": "application/json",
                },
                json={
                    "model": model,
                    "messages": [{"role": "user", "content": prompt}],
                    "max_tokens": 200,
                    "temperature": 0.0,
                },
            )
            resp.raise_for_status()
            data = resp.json()

        text = data["choices"][0]["message"]["content"].strip()
        # Parse JSON response
        import json
        # Handle markdown fences
        if "```" in text:
            text = text.split("```")[1]
            if text.startswith("json"):
                text = text[4:]
        parsed = json.loads(text)
        answer = parsed.get("answer", "none")
        confidence = float(parsed.get("confidence", 0.0))
        reasoning = parsed.get("reasoning", "")

        if answer.lower() == "none" or not answer:
            log.info("LLM extraction: no answer found (%s)", reasoning)
            return None, 0.0

        log.info("LLM extraction: answer=%s, confidence=%.2f (%s)", answer, confidence, reasoning)
        return answer, confidence

    except Exception as e:
        log.warning("LLM answer extraction failed: %s", e)
        return None, 0.0


def _regex_extract_answers(
    results: List[Dict[str, Any]],
) -> Tuple[Optional[str], List[str], float]:
    """Regex-based fallback: extract candidate answers from result text.

    Returns (known_answer, candidate_answers, confidence).
    """
    all_candidates: List[str] = []

    for source_result in results:
        if "error" in source_result:
            continue
        items = source_result.get("results", [])
        if isinstance(items, dict):
            items = [items]
        if not isinstance(items, list):
            continue

        for item in items:
            if not isinstance(item, dict):
                continue
            text_fields = [
                item.get("title", ""),
                item.get("abstract", ""),
                item.get("summary", ""),
                item.get("short_answer", ""),
                item.get("result", ""),
                item.get("page_text", ""),
            ]
            pods = item.get("pods", [])
            if isinstance(pods, list):
                for pod in pods:
                    if isinstance(pod, dict):
                        text_fields.append(pod.get("plaintext", ""))

            combined = " ".join(str(t) for t in text_fields if t)
            if not combined.strip():
                continue

            for pattern in _ANSWER_PATTERNS:
                for m in pattern.finditer(combined):
                    candidate = m.group(1).strip()
                    if candidate and len(candidate) <= 20:
                        try:
                            if "/" in candidate:
                                parts = candidate.split("/")
                                float(parts[0])
                                float(parts[1])
                            else:
                                float(candidate)
                            all_candidates.append(candidate)
                        except (ValueError, IndexError):
                            pass

    if not all_candidates:
        return None, [], 0.0

    counter = Counter(all_candidates)
    most_common = counter.most_common()
    best_answer = most_common[0][0]
    best_count = most_common[0][1]

    unique_candidates = list(counter.keys())
    if len(unique_candidates) == 1:
        confidence = min(0.5 + 0.2 * best_count, 0.95)
    elif best_count >= 2 and best_count > most_common[1][1]:
        confidence = 0.7
    else:
        confidence = 0.3

    known_answer = f"c = {best_answer}" if confidence >= 0.5 else None
    return known_answer, unique_candidates, confidence


async def _extract_answers(
    results: List[Dict[str, Any]],
    problem_statement: str = "",
) -> Tuple[Optional[str], List[str], float]:
    """Extract answers using Tavily's answer + LLM reading of content.

    Returns (known_answer, candidate_answers, confidence).
    """
    # Check Tavily's built-in answer first
    for source_result in results:
        tavily_answer = source_result.get("tavily_answer", "")
        if tavily_answer and len(tavily_answer) > 2:
            log.info("Tavily native answer: %s", tavily_answer[:200])

    # Collect all snippets for LLM
    snippets: List[Dict[str, str]] = []
    for source_result in results:
        if "error" in source_result:
            continue

        # Include Tavily's answer as a high-signal snippet
        tavily_answer = source_result.get("tavily_answer", "")
        if tavily_answer:
            snippets.append({"title": "Tavily AI Answer", "body": tavily_answer})

        items = source_result.get("results", [])
        if isinstance(items, dict):
            items = [items]
        if not isinstance(items, list):
            continue
        for item in items:
            if not isinstance(item, dict):
                continue
            # Combine snippet + fetched page + SE answer for maximum signal
            body_parts = []
            if item.get("abstract"):
                body_parts.append(item["abstract"])
            if item.get("best_answer"):
                body_parts.append(item["best_answer"])
            if item.get("page_text"):
                body_parts.append(item["page_text"][:3000])
            body = "\n".join(body_parts)[:3000]
            if body.strip():
                snippets.append({
                    "title": item.get("title", ""),
                    "body": body,
                })

    # LLM extraction — only path
    if snippets and problem_statement:
        llm_answer, llm_confidence = await _llm_extract_answer(problem_statement, snippets)
        if llm_answer and llm_confidence > 0.0:
            return llm_answer, [llm_answer], llm_confidence

    return None, [], 0.0


def _build_recon_briefing(
    results: List[Dict[str, Any]],
    known_answer: Optional[str],
    candidates: List[str],
) -> str:
    """Build a compact briefing text for the decomposer prompt."""
    lines: List[str] = []

    if known_answer:
        lines.append(f"LIKELY ANSWER: {known_answer}")

    if candidates:
        lines.append(f"Candidate values found: {', '.join(candidates)}")

    # Add key references
    for source_result in results:
        if "error" in source_result:
            continue
        source = source_result.get("source", "unknown")
        items = source_result.get("results", [])
        if isinstance(items, dict):
            items = [items]
        if not isinstance(items, list):
            continue

        for item in items[:2]:  # Max 2 per source
            if not isinstance(item, dict):
                continue
            title = item.get("title", "")
            abstract = item.get("abstract") or item.get("summary", "")
            if title:
                ref = f"[{source}] {title.strip()}"
                if abstract:
                    ref += f" — {abstract.strip()[:120]}"
                lines.append(ref)

    return "\n".join(lines) if lines else ""


_PROMISING_DOMAINS = {
    "artofproblemsolving.com", "web.evanchen.cc", "github.com",
    "gist.github.com", "mathoverflow.net", "math.stackexchange.com",
    "codeforces.com",
}

# Domains to skip fetching (PDFs, video, etc.)
_SKIP_DOMAINS = {"youtube.com", "youtu.be"}


async def _fetch_promising_pages(results: List[Dict[str, Any]]) -> None:
    """Fetch actual page content for top web search results.

    Mutates result items in-place, adding 'page_text' with extracted content.
    Search snippets rarely contain the actual answer — the answer is on the page.
    """
    import httpx

    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
                      "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    }

    async def _fetch_one(item: Dict[str, str]) -> None:
        link = item.get("link", "")
        if not link or link.endswith(".pdf"):
            return
        # Skip video sites
        if any(d in link for d in _SKIP_DOMAINS):
            return

        try:
            async with httpx.AsyncClient(follow_redirects=True, timeout=10.0) as client:
                resp = await client.get(link, headers=headers)
                if resp.status_code != 200:
                    return
                content_type = resp.headers.get("content-type", "")
                if "html" not in content_type and "text" not in content_type:
                    return
                html = resp.text

            # Strip tags and extract text
            text = re.sub(r"<script[^>]*>.*?</script>", "", html, flags=re.DOTALL)
            text = re.sub(r"<style[^>]*>.*?</style>", "", text, flags=re.DOTALL)
            text = re.sub(r"<[^>]+>", " ", text)
            text = re.sub(r"\s+", " ", text)
            # Decode entities
            text = text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
            text = text.replace("&#39;", "'").replace("&quot;", '"')
            text = text.replace("&nbsp;", " ").replace("&#x27;", "'")

            # Keep enough to find the answer — it may be deep in the page
            item["page_text"] = text[:8000]
            log.debug("Fetched %d chars from %s", len(text[:8000]), link[:60])

        except Exception as e:
            log.debug("Failed to fetch %s: %s", link[:60], e)

    # Collect fetchable items from ALL search results with links
    fetch_tasks = []
    seen_links = set()
    for source_result in results:
        items = source_result.get("results", [])
        if not isinstance(items, list):
            continue
        for item in items[:5]:  # Top 5 per query
            if isinstance(item, dict) and item.get("link"):
                link = item["link"]
                if link not in seen_links:
                    seen_links.add(link)
                    fetch_tasks.append(_fetch_one(item))

    if fetch_tasks:
        await asyncio.gather(*fetch_tasks, return_exceptions=True)


async def run_reconnaissance(req) -> Dict[str, Any]:
    """Run pre-decomposition reconnaissance for a problem.

    Searches for THIS EXACT PROBLEM's known answer before the decomposer
    creates an obligation graph.
    """
    # Load .env for API keys if not already in environment
    try:
        from pathlib import Path
        env_path = Path(__file__).resolve().parents[3] / ".env"
        if env_path.exists():
            for line in env_path.read_text().splitlines():
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    key, _, val = line.partition("=")
                    if key.strip() and key.strip() not in os.environ:
                        os.environ[key.strip()] = val.strip()
    except Exception:
        pass

    from ..research.router import research_search, SearchRequest

    problem_statement = getattr(req, "problem_statement", "")
    source = getattr(req, "problem_source", "") or ""
    title = getattr(req, "problem_title", None)
    domain = getattr(req, "domain", None) or ""

    queries = _build_recon_queries(problem_statement, source, title, domain)

    if not queries:
        return {
            "known_answer": None,
            "candidate_answers": [],
            "confidence": 0.0,
            "proof_strategies": [],
            "key_references": [],
            "briefing": "",
        }

    # Run all queries in parallel
    async def _run_query(q: Dict[str, str]) -> Dict[str, Any]:
        try:
            sub = SearchRequest(
                source=q["source"],
                query=q["query"],
                max_results=3,
            )
            result = await research_search(sub)
            return {"source": q["source"], "query": q["query"], **result}
        except Exception as e:
            log.warning("Reconnaissance: %s query failed: %s", q["source"], e)
            return {"source": q["source"], "query": q["query"], "error": str(e)}

    tasks = [_run_query(q) for q in queries]
    raw_results = await asyncio.gather(*tasks, return_exceptions=True)

    results: List[Dict[str, Any]] = []
    for q, raw in zip(queries, raw_results):
        if isinstance(raw, Exception):
            results.append({"source": q["source"], "query": q["query"], "error": str(raw)})
        else:
            results.append(raw)

    # === Page fetch pass ===
    # Search snippets often lack the actual answer. For web results with
    # promising titles (AoPS, solution notes, etc.), fetch the page and
    # scan its text for answer patterns.
    await _fetch_promising_pages(results)

    # Extract answers from results (LLM primary, regex fallback)
    known_answer, candidates, confidence = await _extract_answers(results, problem_statement)

    # Build briefing for decomposer
    briefing = _build_recon_briefing(results, known_answer, candidates)

    # Extract proof strategies (titles/abstracts that mention techniques)
    strategies: List[str] = []
    references: List[str] = []
    for r in results:
        if "error" in r:
            continue
        items = r.get("results", [])
        if isinstance(items, dict):
            items = [items]
        if not isinstance(items, list):
            continue
        for item in items[:2]:
            if not isinstance(item, dict):
                continue
            t = item.get("title", "")
            if t:
                references.append(f"[{r.get('source', '?')}] {t.strip()}")
            abstract = item.get("abstract") or item.get("summary", "")
            if abstract:
                # Look for technique mentions
                for kw in ["construction", "prove", "show that", "by contradiction",
                            "induction", "extremal", "pigeonhole", "generating function"]:
                    if kw in abstract.lower():
                        # Extract the sentence containing the technique
                        for sent in re.split(r"[.!?]", abstract):
                            if kw in sent.lower() and len(sent.strip()) > 20:
                                strategies.append(sent.strip()[:150])
                                break
                        break

    return {
        "known_answer": known_answer,
        "candidate_answers": candidates,
        "confidence": confidence,
        "proof_strategies": strategies[:5],
        "key_references": references[:5],
        "briefing": briefing,
    }
