---
name: rq-authoring
description: Write, edit and review rqlang (.rq) HTTP request files. Use whenever the user mentions rq, rqlang or .rq files, asks to add or change requests, endpoints, environments, variables, secrets or auth in a .rq file, or wants an API described as .rq requests.
---

# Authoring rqlang files

rqlang syntax is not reliably known from memory. The `rq` MCP server bundled with this plugin is the source of truth; do not infer syntax from trial and error or by searching the filesystem for documentation.

## Workflow

1. Before writing or refactoring any `.rq` file, call `get_rq_reference` with `doc="language-definition"` and with `doc="idioms"`.
2. Before adding requests, call `list_requests` on the target folder so new names do not collide with existing ones.
3. Draft the snippet. Keep one `ep` per file, named after the endpoint.
4. Call `validate_rq` with the file's name as `path`. Fix and repeat until it returns `ok: true`.
5. Call `lint_rq` with the same `path`. Apply the suggested fixes and repeat until it returns `ok: true`.

When the output spans several files, run steps 4 and 5 once per file.

## Rules

- Some lint rules are cleared by moving code into another file; never delete content the user asked for just to silence a diagnostic.
- Never add an `rq-lint-ignore` comment unless the user asks for that suppression.
- The server authors and checks files; it never sends requests. Do not suggest `rq` CLI commands to run them unless the user asks how.
