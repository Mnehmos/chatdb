"""Research API configuration — manages keys and rate limit state."""
import os
import time
import sqlite3
from typing import Optional, Dict
from pathlib import Path

# Default DB path — same as sidecar/MCP convention
_DB_PATH = os.environ.get("CHATDB_DB_PATH", "chatdb.sqlite")


def _get_conn() -> sqlite3.Connection:
    """Get a connection to the ChatDB database for key lookups."""
    return sqlite3.connect(_DB_PATH)


def get_api_key(service: str) -> Optional[str]:
    """Resolve an API key: DB first, then environment variable fallback.

    Service names: semantic_scholar, openalex, wolfram_alpha,
                   huggingface, ncbi, unpaywall_email
    """
    # 1. Try DB lookup
    try:
        conn = _get_conn()
        row = conn.execute(
            "SELECT key_value FROM research_api_keys WHERE service = ?1 AND active = 1",
            (service,),
        ).fetchone()
        conn.close()
        if row and row[0]:
            return row[0]
    except Exception:
        pass  # Table might not exist yet

    # 2. Environment variable fallback
    env_map = {
        "semantic_scholar": "SEMANTIC_SCHOLAR_API_KEY",
        "openalex": "OPENALEX_EMAIL",
        "wolfram_alpha": "WOLFRAM_APP_ID",
        "huggingface": "HUGGINGFACE_TOKEN",
        "ncbi": "NCBI_API_KEY",
        "unpaywall_email": "UNPAYWALL_EMAIL",
        "stackexchange": "STACKEXCHANGE_API_KEY",
        "tavily": "TAVILY_API_KEY",
    }
    env_key = env_map.get(service)
    if env_key:
        return os.environ.get(env_key)
    return None


class RateLimiter:
    """Simple token-bucket rate limiter per service."""

    def __init__(self):
        self._buckets: Dict[str, list] = {}
        self._limits = {
            "arxiv": (1, 3.0),       # 1 request per 3 seconds
            "semantic_scholar": (10, 1.0),  # 10 req/s with key, 1 req/s without
            "oeis": (1, 2.0),
            "crossref": (50, 1.0),
            "unpaywall": (10, 1.0),
            "openalex": (10, 1.0),
            "wolfram_alpha": (2, 1.0),
            "huggingface": (10, 1.0),
            "loogle": (1, 2.0),
            "pubmed": (3, 1.0),
            "google": (1, 2.0),  # Be polite — 1 req per 2 seconds
            "stackexchange": (5, 1.0),  # 5 req/s — generous with API key
            "tavily": (5, 1.0),  # 5 req/s
        }

    async def acquire(self, service: str):
        """Wait until a request slot is available."""
        max_req, window = self._limits.get(service, (5, 1.0))
        if service not in self._buckets:
            self._buckets[service] = []

        bucket = self._buckets[service]
        now = time.monotonic()

        # Purge old timestamps
        bucket[:] = [t for t in bucket if now - t < window]

        if len(bucket) >= max_req:
            wait = window - (now - bucket[0])
            if wait > 0:
                import asyncio
                await asyncio.sleep(wait)
            bucket[:] = [t for t in bucket if time.monotonic() - t < window]

        bucket.append(time.monotonic())


# Singleton
rate_limiter = RateLimiter()
