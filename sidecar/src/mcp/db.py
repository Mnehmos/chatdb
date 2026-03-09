"""SQLite connection helpers for the MCP server.

Uses CHATDB_DB_PATH env var (default: chatdb.sqlite in working directory).
Opens a fresh connection per call — WAL mode allows safe concurrent reads
alongside the Tauri app's write connection.
"""
import os
import sqlite3
import uuid
from datetime import datetime, timezone


def db_path() -> str:
    return os.environ.get("CHATDB_DB_PATH", "chatdb.sqlite")


def get_db() -> sqlite3.Connection:
    conn = sqlite3.connect(db_path())
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode=WAL;")
    return conn


def new_id() -> str:
    return str(uuid.uuid4())


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def row_to_dict(row: sqlite3.Row) -> dict:
    return dict(row)


def rows_to_list(rows) -> list[dict]:
    return [dict(r) for r in rows]
