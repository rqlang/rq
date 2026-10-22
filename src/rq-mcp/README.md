# @rqlang/mcp

An [MCP](https://modelcontextprotocol.io) server that teaches AI assistants to write [rqlang](https://www.rqlang.com) `.rq` files correctly. It works with any MCP client that can launch a local server: Claude Code, Claude Desktop, Cursor, Windsurf, VS Code, Codex, Gemini CLI, Zed and others.

It authors and checks `.rq` files; it never sends a request. Everything runs locally on Node.js 18 or newer, with the rqlang parser compiled to WebAssembly, so there is no native binary to install.

## Setup

The server speaks MCP over stdio. Every client launches it with the same command:

```bash
npx -y @rqlang/mcp
```

Start the client from your project folder: tools that take no path use the folder the server runs in.

**Claude Code**

```bash
claude mcp add rq -- npx -y @rqlang/mcp
```

**Codex CLI**

```bash
codex mcp add rq -- npx -y @rqlang/mcp
```

**Claude Desktop, Cursor, Windsurf, Gemini CLI** and other clients configured through a JSON file (`claude_desktop_config.json`, `.cursor/mcp.json`, `~/.codeium/windsurf/mcp_config.json`, `~/.gemini/settings.json`, …):

```json
{
  "mcpServers": {
    "rq": {
      "command": "npx",
      "args": ["-y", "@rqlang/mcp"]
    }
  }
}
```

**VS Code**: install the [rq extension](https://marketplace.visualstudio.com/items?itemName=rq-lang.rq-language), which registers this same server for Copilot with nothing to configure.

## What it provides

| Kind | Name | Purpose |
|---|---|---|
| Tool | `validate_rq` | Parses and analyzes a snippet against the files around it; returns syntax and semantic diagnostics with line and column |
| Tool | `lint_rq` | Checks a snippet against the rqlang idioms; returns rule-tagged diagnostics with a suggested fix |
| Tool | `list_requests` | Lists the requests already defined under a path, so new ones do not collide |
| Tool | `get_rq_reference` | Returns the rqlang grammar reference or the idioms guide |
| Resource | `rqlang://docs/language-definition` | The rqlang grammar reference |
| Resource | `rqlang://docs/idioms` | The rqlang idioms guide |
| Prompt | `generate_rq` | Drives the generate → validate → lint → iterate loop for a natural-language intent |

Drafts passed to `validate_rq` and `lint_rq` are checked in memory and never written to disk. Imports resolve against `workspace_path`, or against the folder the server runs in when it is omitted.

## License

MIT
