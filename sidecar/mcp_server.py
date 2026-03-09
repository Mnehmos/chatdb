"""ChatDB MCP Server entry point.

Launched by Claude Desktop/Code as a subprocess (stdio transport):

  claude_desktop_config.json:
  {
    "mcpServers": {
      "chatdb": {
        "command": "python",
        "args": ["sidecar/mcp_server.py"],
        "env": {
          "CHATDB_DB_PATH": "/absolute/path/to/chatdb.sqlite",
          "CHATDB_SIDECAR_URL": "http://localhost:9743",
          "CHATDB_CONTROL_URL": "http://localhost:9744"
        }
      }
    }
  }

Environment variables:
  CHATDB_DB_PATH      Path to chatdb.sqlite (default: chatdb.sqlite in cwd)
  CHATDB_SIDECAR_URL  Python sidecar URL (default: http://localhost:9743)
  CHATDB_CONTROL_URL  Tauri control API URL (default: http://localhost:9744)
"""
import sys
import os

# Ensure src/ is importable when running from the sidecar/ directory
_here = os.path.dirname(os.path.abspath(__file__))
if _here not in sys.path:
    sys.path.insert(0, _here)

from src.mcp import chatdb_mcp

if __name__ == "__main__":
    chatdb_mcp.run()
