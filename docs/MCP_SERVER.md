---
layout: default
title: MCP Server
nav_order: 9
---

{% raw %}

# MCP Server

The rq extension bundles `rq-mcp`, an [MCP](https://modelcontextprotocol.io) server that teaches AI assistants to write `.rq` files correctly. It is registered automatically on activation — nothing to install or configure.

For a hands-on walkthrough, start with [AI-Assisted Authoring](AI_ASSISTED_AUTHORING.md). This page is the reference: what each tool does, what the checks enforce, and what they do not.

## Why it exists

Ask a model for an HTTP request file and it falls back on what it has seen most: JSON bodies as quoted strings, hand-written `Authorization` headers, `{param}` placeholders from other templating languages. All wrong in rqlang, and expensive for one reason — **most of them parse**. A single-braced `{user_id}` is valid rqlang; it just sends that text literally and requests the wrong URL.

The server closes the gap from both ends: it gives the assistant rq's grammar reference and style guide up front, then checks each draft with the same parser, analyzer and linter behind the editor's diagnostics and `rq check`.

## How it runs

The server is bundled as JavaScript and launched on the same Node.js that VS Code itself runs on, reusing the WebAssembly build of rq the extension already loads. There is no native binary and no platform-specific download — a single package works everywhere the extension does.

It runs with your first workspace folder as its working directory, so relative paths, `import` statements and `.env` files resolve the way they do on disk.

## Tools

### `validate_rq`

Parses and analyzes a draft against the real parser and analyzer, returning `{ ok, diagnostics[] }` with severity, message, line and column.

| Parameter | Purpose |
|---|---|
| `source` | The draft. Required. Never written to disk |
| `path` | Logical filename used in diagnostics |
| `workspace_path` | Directory the draft is checked against — needed for `import` statements and `.env` secrets |
| `env` | Environment to select for variable resolution |

`workspace_path` matters more than it looks. Without it, a draft that imports a sibling file reports the import as an error, and a file whose variables live in `env` blocks reports them as unresolved unless `env` names one.

### `lint_rq`

Applies the idiom and style rules, returning `{ ok, diagnostics[] }` where each finding carries a `rule` id and, where the fix is mechanical, a `suggested_fix`. Takes `source`, `path` and `workspace_path`.

Given a `workspace_path` the lint also sees the *other* `.rq` files in the workspace, which is what lets it flag a draft that duplicates a base URL or an auth provider already declared next door.

### `list_requests`

Enumerates every named request reachable from `path` (defaulting to the workspace folder), returning each request's name, endpoint and file. An assistant calls this before generating so new names don't collide — and so it can tell a genuinely new entity from one that already has requests elsewhere.

### `get_rq_reference`

Returns the full text of one of two documents: `language-definition` (the complete grammar) or `idioms` (the style rules in prose). Both are also published as resources — `rqlang://docs/language-definition` and `rqlang://docs/idioms` — for clients that let a model read those directly.

## The `generate_rq` prompt

The server also exposes a prompt that drives the whole flow for a stated intent: read both reference documents, list existing requests, decide the file layout *before* drafting, then validate and lint each file separately until both come back clean.

## The loop

The order is not arbitrary. `validate_rq` answers "is this rqlang?"; `lint_rq` answers "is it written the way rq expects?". A draft can pass the first and fail the second badly enough to reach production, so both run, in that order, until clean.

An assistant's output is not deterministic: two runs of the same prompt pick different names and explain themselves differently. The shape is reliable, because the rules enforce it:

- The file parses, and every variable it references resolves.
- One `ep` per file, named after the endpoint.
- Requests inside an `ep` are named after the verb alone.
- JSON bodies are `${ ... }` literals or `.json` fixtures — never quoted strings.
- Credentials come from `.env`, never from the `.rq` file.
- Interpolation is `{{name}}`, always double-braced.

If a generated file breaks one of those, the assistant hasn't finished the loop. Ask it to run `lint_rq` again.

One rule is cleared by *moving* code rather than editing it: `multiple_endpoints_per_file` wants the file split into one file per `ep`, not an endpoint deleted. The server's instructions say so explicitly — content you asked for should never disappear to silence a diagnostic.

## The mistakes only the linter catches

Both of the snippets below are **valid rqlang**. `validate_rq` returns `ok` on each, and each sends the wrong request.

```
rq get("/{widget_id}");
```

```
single_brace_interpolation
  `{widget_id}` uses single braces, which rqlang does not treat as interpolation.
  The text `{widget_id}` is sent literally, so the request silently targets the
  wrong URL. Use `{{widget_id}}`, or — for a URL path parameter — pass
  `widget_id` as a bare identifier with the `[required(widget_id)]` attribute.
```

```
[required(user_id)]
rq get(user_id, $["v": "1"]);
```

```
query_param_as_header
  `rq get` passes `$["v": "1"]` in its second positional argument. For `rq` the
  positional arguments are `url`, `headers`, `body` — there is no `qs` parameter
  — so rqlang sends this as the HTTP header `v: 1` and the query string `?v=1`
  never reaches the server. Only `ep` takes `qs`.
```

A request that goes out with a stray header instead of a query parameter returns `200` and the wrong body. This is the class of bug the idiom rules exist for, and the reason a clean parse is not the finish line.

## Style rules

| Rule | What it wants |
|---|---|
| `top_level_rq_should_be_ep` | Two top-level requests share a base URL — group them in an `ep` |
| `duplicated_noun_in_ep` | Request inside `ep users` is named `get_users` — use the verb alone |
| `empty_url_string` | `rq list("")` — inside an `ep` write `rq list()` |
| `json_body_as_string` | `body: "{}"` sends a string — use `body: ${}` |
| `redundant_content_type_on_json_body` | rq derives `Content-Type` from a JSON body |
| `missing_body_on_write` | A POST/PUT/PATCH with no payload |
| `multiple_endpoints_per_file` | Split into one `ep` per file |
| `duplicated_ep_base` / `duplicated_ep_config` | Two endpoints repeat a base URL, headers or auth — extract a template |
| `base_ep_extension` | A template with only one consumer — inline it |
| `manual_auth_header` | Hand-built `Authorization` — use an `auth` provider |
| `hardcoded_secret` | A credential literal in a committed file — move it to `.env` |
| `single_brace_interpolation` | `{name}` is literal text — use `{{name}}` |
| `query_param_as_header` | A query parameter passed where headers go |
| `duplicated_request_qs` | The same query parameter written into sibling requests — put it on the `ep` as `qs` |
| `let_default_instead_of_required` | A `let` standing in for a runtime input — use `[required(...)]` |
| `absolute_import_path` | `import "/Users/..."` — use a relative path |
| `invalid_lint_suppression` | An `rq-lint-ignore` comment naming an unknown rule or no rule |
| `unused_lint_suppression` | An `rq-lint-ignore` comment that silences nothing |

The rules are picky in both directions. `duplicated_ep_base` asks you to extract a template once two endpoints share a base URL — and `base_ep_extension` rejects that same template while only one endpoint extends it. A template earns its keep from the second consumer onward, not before.

A finding you have decided to keep is silenced with an `rq-lint-ignore` comment — see [Suppressing lint findings](LANGUAGE_DEFINITION.md#suppressing-lint-findings). The assistant is told never to add one on its own: it fixes what the linter reports, and suppresses a rule only when you ask it to.

These are the same findings the editor shows in the Problems panel as warnings under the `rq lint` source, whether or not an assistant is involved — see [Idiom linting](VSCODE_EXTENSION.md#idiom-linting). Setting `rq.lint.enabled` to `false` silences them in the editor; the MCP server checks regardless.

## Guarantees and limits

- **It authors, it does not execute.** No tool fires a request, and the server will not propose CLI commands for running one unless you ask how. See the [CLI](CLI.md) for that.
- **It validates rqlang, not your API.** A file that passes both tools is well-formed and idiomatic. Whether the path exists, the payload matches the schema, or the token carries the right scope is still yours to verify — run the request and read the response.
- **It does not import formats.** There is no OpenAPI, Swagger or Postman importer anywhere in rq. Generating requests from a spec works because the *assistant* reads the file and the server checks the result, which is why it works for any format the assistant can read.
- **It does not make an assistant deterministic.** The rules constrain the shape of the output, not the naming or the reasoning behind it. Read what lands before you commit it.

## See also

- [AI-Assisted Authoring](AI_ASSISTED_AUTHORING.md) — the same ground as a ten-minute tutorial
- [VS Code Extension](VSCODE_EXTENSION.md) — the rest of what the extension provides
- [Language Definition](LANGUAGE_DEFINITION.md) — the grammar the server serves to assistants

{% endraw %}
