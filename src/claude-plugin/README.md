# rqlang plugin for Claude Code

Teaches Claude Code to write [rqlang](https://www.rqlang.com) `.rq` files correctly.

## Install

```
/plugin install rqlang@claude-plugins-official
```

Requires Node.js 18 or newer: the plugin starts the [`@rqlang/mcp`](../rq-mcp) server with `npx`.

## What it provides

| Kind | Name | Purpose |
|---|---|---|
| MCP server | `rq` | `validate_rq`, `lint_rq`, `list_requests`, `get_rq_reference` and the `generate_rq` prompt, from `@rqlang/mcp` |
| Skill | `rq-authoring` | The reference → list → draft → validate → lint loop Claude follows for any `.rq` work |
| Command | `/rqlang:generate` | Generates requests from a natural-language description |

## What it runs and sends

- The first time the MCP server starts, `npx` downloads `@rqlang/mcp` at the exact version pinned in `.mcp.json`, with its dependencies, from the public npm registry. Later starts reuse the npm cache.
- The server runs locally on Node.js. It reads the `.rq` files under the workspace folder, or under the path a tool is given, to resolve imports and list request names.
- To check that the secrets a file uses are defined, the server reads the `.env` file next to the checked files and the `RQ__*` environment variables. Their values stay on the machine.
- Drafts passed to `validate_rq` and `lint_rq` are checked in memory and never written to disk.
- The server never sends the HTTP requests described in `.rq` files, makes no other network calls and collects no data.

## Develop

Load the plugin from the checkout without installing it:

```bash
claude --plugin-dir ./src/claude-plugin
```

To try an unpublished server, point `.mcp.json` at a local build (`node ../rq-mcp/dist/server.js`) instead of `npx`, and do not commit that change.
