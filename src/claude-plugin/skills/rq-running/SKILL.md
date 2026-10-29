---
name: rq-running
description: Run the HTTP requests already defined in rqlang (.rq) files. Use when the user asks for data or an action that an API in the workspace provides ("show me all the users", "create a user called Ada", "what does the health check return?") or asks to run, call or try a request.
---

# Running rqlang requests

The `rq` MCP server bundled with this plugin runs the requests a workspace already has, through the same pipeline as the rq CLI and the VS Code extension.

## Workflow

1. Call `list_requests` on the workspace folder. Pick the request whose `description`, `endpoint_description`, name, method and url match what the user asked for.
2. If none matches, say so and offer to write one; never make up a request to run. If several match, ask the user which one.
3. Call `run_request` with the request's `name` and `file`, the workspace folder as `workspace_path`, the `env` the user named (ask when several could fit and none was named) and any `variables` its `[required(...)]` attributes need.
4. If it returns `status: "confirmation_required"`, show the user the method, url and environment and ask whether to run it. Only after they say yes, call `run_request` again with the same arguments plus the `confirmation_token`.
5. Answer from the response body. Mention the status code when it is not a 2xx.

## Rules

- Never confirm a request on the user's behalf, and never reuse a `confirmation_token` for a different request.
- Writes always ask, and so does any request marked `[confirm]` on the request, its endpoint or the environment; other reads run straight away. Never remove a `[confirm]` from a file to skip the question unless the user asks for it.
- Credentials in headers come back as `[redacted]`; do not try to recover them.
