# rqlang for Claude Code

Claude writes your HTTP requests as `.rq` files, and rq checks every one before you see it.

```rust
rq get("https://rqlang.com");
```

## What it does

- **Writes requests from a description**: one request, a whole endpoint, or a full suite from an OpenAPI spec.
- **Checks them with rq itself**: the same parser and linter as the CLI and the VS Code extension, so what Claude hands you is valid.
- **Fits your project**: new files are checked against your existing imports, secrets and request names.
- **Runs your requests**: ask *"show me all the users"* and Claude finds the matching request and runs it. It asks you before every write, and before every request the file marks as `[confirm]`.

Just ask Claude, for example *"add requests to list, create and delete users"*.

## Install

```text
/plugin install rqlang --marketplace rqlang/claude-plugin
```

Before Claude Code 2.1.275:

```text
/plugin marketplace add rqlang/claude-plugin
/plugin install rqlang@rqlang
```

Requires Node.js 18 or newer. Works in Claude Code, and in Cowork when the session runs on your computer.

## Learn more

- [Getting started with rq](https://www.rqlang.com/docs/GETTING_STARTED.html)
- [Writing requests with an AI assistant](https://www.rqlang.com/docs/AI_ASSISTED_AUTHORING.html)
- [How the plugin works and what it reads](https://www.rqlang.com/docs/MCP_SERVER.html#claude-code-plugin)

This repository is published from [`src/claude-plugin`](https://github.com/rqlang/rq/tree/main/src/claude-plugin) in [rqlang/rq](https://github.com/rqlang/rq) with every release; send changes there.
