"""Tests for pre-decomposition reconnaissance agent."""
import pytest
from unittest.mock import AsyncMock, patch
from src.agents.reconnaissance import (
    _build_recon_queries,
    _regex_extract_answers,
    run_reconnaissance,
)


# ── Query building ───────────────────────────────────────────

def test_build_recon_queries_imo():
    """Competition source like 'IMO 2025 P3' produces competition-specific queries."""
    queries = _build_recon_queries(
        problem_statement="Determine the smallest constant c...",
        source="IMO 2025 P3",
        title=None,
        domain="number_theory",
    )
    # Should have a competition query
    comp_queries = [q for q in queries if "IMO" in q["query"] and "2025" in q["query"]]
    assert len(comp_queries) >= 1, f"Expected competition query, got {queries}"
    # Should include a Google query (web search for competitions)
    google_queries = [q for q in queries if q["source"] == "google"]
    assert len(google_queries) >= 1, f"Expected Google query for competition, got {queries}"
    # Should also have at least one object-based query
    assert len(queries) >= 2


def test_build_recon_queries_usamo():
    """USAMO source is also detected."""
    queries = _build_recon_queries(
        problem_statement="Find the value of...",
        source="USAMO 2024 Problem 5",
        title=None,
        domain="algebra",
    )
    comp_queries = [q for q in queries if "USAMO" in q["query"] and "2024" in q["query"]]
    assert len(comp_queries) >= 1


def test_build_recon_queries_generic():
    """No competition source → only object-based and computational queries."""
    queries = _build_recon_queries(
        problem_statement="Let f: N -> N be a bonza function. Determine the smallest real constant c such that f(n) <= cn.",
        source="user",
        title=None,
        domain="number_theory",
    )
    # Should NOT have competition queries
    comp_queries = [q for q in queries if any(k in q["query"] for k in ["IMO", "USAMO", "Putnam"])]
    assert len(comp_queries) == 0
    # Should have at least one query about the mathematical objects
    assert len(queries) >= 1


# ── Answer extraction (regex path) ────────────────────────────

def test_extract_answers_single_source():
    """Single source with clear answer → extracts it with moderate confidence."""
    results = [
        {
            "source": "arxiv",
            "results": [
                {
                    "title": "Solution to IMO 2025 Problem 3",
                    "abstract": "We show that the answer is c = 4, achieved by a specific bonza function construction.",
                }
            ],
        }
    ]
    known, candidates, confidence = _regex_extract_answers(results)
    assert "4" in (known or ""), f"Expected known_answer containing '4', got {known}"
    assert confidence > 0.0


def test_extract_answers_multiple_agree():
    """Multiple sources agreeing → high confidence."""
    results = [
        {
            "source": "arxiv",
            "results": [{"title": "IMO 2025 P3", "abstract": "The answer is c = 4."}],
        },
        {
            "source": "semantic_scholar",
            "results": [{"title": "Bonza functions", "abstract": "optimal constant c = 4"}],
        },
    ]
    known, candidates, confidence = _regex_extract_answers(results)
    assert known is not None
    assert "4" in known
    assert confidence >= 0.7


def test_extract_answers_conflicting():
    """Two sources with different answers → both listed as candidates, lower confidence."""
    results = [
        {
            "source": "arxiv",
            "results": [{"title": "Paper A", "abstract": "The answer is c = 3 for this variant."}],
        },
        {
            "source": "semantic_scholar",
            "results": [{"title": "Paper B", "abstract": "The answer is c = 4 for the original."}],
        },
    ]
    known, candidates, confidence = _regex_extract_answers(results)
    assert len(candidates) >= 2
    assert confidence < 0.7  # Conflicting → low confidence


def test_extract_answers_empty():
    """No results → known_answer=None, confidence=0.0."""
    results = [
        {"source": "arxiv", "results": []},
        {"source": "semantic_scholar", "error": "timeout"},
    ]
    known, candidates, confidence = _regex_extract_answers(results)
    assert known is None
    assert len(candidates) == 0
    assert confidence == 0.0


def test_extract_answers_no_math():
    """Results without mathematical answers → no false positives."""
    results = [
        {
            "source": "arxiv",
            "results": [
                {
                    "title": "A survey of functional equations",
                    "abstract": "We review recent progress in functional equations over integers.",
                }
            ],
        }
    ]
    known, candidates, confidence = _regex_extract_answers(results)
    # Should not extract a random number as the answer
    assert confidence < 0.5


# ── Full reconnaissance (mocked) ─────────────────────────────

@pytest.mark.asyncio
async def test_full_reconnaissance_empty():
    """When all sources return nothing, response is valid and empty."""
    class MockReq:
        problem_statement = "Prove that x^2 >= 0 for all real x."
        problem_source = "user"
        problem_title = None
        domain = "algebra"
        sources = ["arxiv", "semantic_scholar"]

    with patch("src.research.router.research_search", new_callable=AsyncMock) as mock_search:
        mock_search.return_value = {"results": [], "total_results": 0}
        result = await run_reconnaissance(MockReq())

    assert result["known_answer"] is None
    assert result["confidence"] == 0.0
    assert isinstance(result["candidate_answers"], list)
    assert isinstance(result["briefing"], str)
