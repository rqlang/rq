# rqlang

HTTP requests as code, written by Claude and checked by rq's own parser and linter.

```rust
rq get("https://rqlang.com");
```

Ask Claude for a request, an endpoint or a whole suite from an OpenAPI spec, or run the server's `generate_rq` prompt from the `/` menu. Works in Claude Code, and in Cowork when the session runs on your computer. Requires Node.js 18 or newer.

## The rq ecosystem

- [rqlang.com](https://www.rqlang.com): [getting started](https://www.rqlang.com/docs/GETTING_STARTED.html), [advanced example](https://www.rqlang.com/docs/ADVANCED_EXAMPLE.html), [AI-assisted authoring](https://www.rqlang.com/docs/AI_ASSISTED_AUTHORING.html)
- [VS Code extension](https://marketplace.visualstudio.com/items?itemName=rq-lang.rq-language): run requests from the editor
- [CLI](https://www.rqlang.com/docs/INSTALLATION.html): run them from the terminal and CI
- [`@rqlang/mcp`](https://www.npmjs.com/package/@rqlang/mcp): the MCP server behind this plugin, for any other assistant
- [GitHub](https://github.com/rqlang/rq)

## What it runs and sends

- The server is started by `scripts/npx.mjs`, which only runs `npx` with the arguments in `.mcp.json` (through the shell on Windows, where `npx` is a `.cmd` script).
- The first time the MCP server starts, `npx` downloads `@rqlang/mcp` at the exact version pinned in `.mcp.json`, with its dependencies, from the public npm registry. Later starts reuse the npm cache.
- The server runs locally on Node.js. It reads the `.rq` files under the workspace folder, or under the path a tool is given, to resolve imports and list request names.
- To check that the secrets a file uses are defined, the server reads the `.env` file in the workspace folder (the `workspace_path` a tool is given, or the session's working directory) and the `RQ__*` environment variables. Their values stay on the machine.
- Drafts passed to `validate_rq` and `lint_rq` are checked in memory and never written to disk.
- The server never sends the HTTP requests described in `.rq` files, makes no other network calls and collects no data.
