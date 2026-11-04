---
layout: default
title: MCP Server
nav_order: 9
---

{% raw %}

# MCP Server

`rq-mcp` is an [MCP](https://modelcontextprotocol.io) server that teaches AI assistants to write `.rq` files correctly, and lets them run the requests a workspace already has. The VS Code extension registers it automatically for Copilot, and any other MCP client — Claude Code, Claude Desktop, Cursor, Windsurf, Codex, Gemini CLI, Zed — runs it from npm as [`@rqlang/mcp`](https://www.npmjs.com/package/@rqlang/mcp).

For a hands-on walkthrough, start with [AI-Assisted Authoring](AI_ASSISTED_AUTHORING.md). This page is the reference: what each tool does, what the checks enforce, and what they do not.

## Why it exists

Ask a model for an HTTP request file and it falls back on what it has seen most: JSON bodies as quoted strings, hand-written `Authorization` headers, `{param}` placeholders from other templating languages. All wrong in rqlang, and expensive for one reason — **most of them parse**. A single-braced `{user_id}` is valid rqlang; it just sends that text literally and requests the wrong URL.

The server closes the gap from both ends: it gives the assistant rq's grammar reference and style guide up front, then checks each draft with the same parser, analyzer and linter behind the editor's diagnostics and `rq check`.

## Setup

**VS Code**: install the [extension](VSCODE_EXTENSION.md). The server is registered on activation, with nothing to configure.

**Any other MCP client** launches it over stdio with Node.js 18 or newer:

```bash
npx -y @rqlang/mcp
```

**Claude Code**: install the [plugin](#claude-code-plugin), which also teaches Claude when to use the server:

```text
/plugin install rqlang --marketplace rqlang/claude-plugin
```

Claude Code and Codex can also add the server alone from the command line:

```bash
claude mcp add rq -- npx -y @rqlang/mcp
codex mcp add rq -- npx -y @rqlang/mcp
```

Claude Desktop, Cursor, Windsurf, Gemini CLI and other clients configured through a JSON file take the same command under `mcpServers`:

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

## How it runs

The server is JavaScript running the WebAssembly build of rq — the same parser, analyzer and linter the CLI uses. There is no native binary and no platform-specific download. In VS Code it runs on the Node.js the editor ships with; elsewhere, on the Node.js that `npx` finds.

It works relative to the folder it runs in: the first workspace folder in VS Code, the folder the client was started from elsewhere. Relative paths, `import` statements and `.env` files resolve from there the way they do on disk, and every tool that takes a path accepts an absolute one to point somewhere else.

## Claude Code plugin

The `rqlang` plugin for Claude Code, published from [`src/claude-plugin`](https://github.com/rqlang/rq/tree/main/src/claude-plugin) to [`rqlang/claude-plugin`](https://github.com/rqlang/claude-plugin), bundles this server with an `rq-authoring` skill that tells Claude to use it whenever `.rq` files come up. The plugin has no server code of its own:

- It starts the server through `scripts/npx.mjs`, which only runs `npx` with the arguments in `.mcp.json` (through `cmd.exe` on Windows, where `npx` is a `.cmd` script).
- The first time the server starts, `npx` downloads `@rqlang/mcp` at the exact version pinned in `.mcp.json`, with its dependencies, from the public npm registry. Later starts reuse the npm cache. Each rq release pins its own version.
- The server runs locally on Node.js. It reads the `.rq` files under the workspace folder, or under the path a tool is given, to resolve imports and list request names.
- To check that the secrets a file uses are defined, it reads the `.env` file in the workspace folder (the `workspace_path` a tool is given, or the session's working directory) and the `RQ__*` environment variables. Their values stay on the machine.
- Drafts passed to `validate_rq` and `lint_rq` are checked in memory and never written to disk.
- It never sends the HTTP requests described in `.rq` files, makes no other network calls and collects no data.

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

Enumerates every named request reachable from `path` (defaulting to the workspace folder), returning each request's name, method, URL (not interpolated), endpoint and file, plus the text of its [`doc.description`](LANGUAGE_DEFINITION.md#docdescription-attribute) and of its endpoint's. An assistant calls this before generating so new names don't collide — and so it can tell a genuinely new entity from one that already has requests elsewhere. It is also how it finds the request to run: asked to "show me all the users", it matches those words against the descriptions, names and URLs.

### `run_request`

Sends an existing request and returns its response: `{ status: "sent", request: {request, method, url, environment, headers}, response: {status, elapsed_ms, headers, body, body_truncated} }`. The request goes through the same pipeline as `rq request run` and the extension's **Run**, so it carries the same URL, headers, body, auth and timeout.

| Parameter | Purpose |
|---|---|
| `name` | Request name as `list_requests` reports it, `users/list` or `users.list`. Required |
| `file` | The `file` `list_requests` reported, to pick it when several files define the same name. It must be inside `workspace_path` (symlinks are followed before checking). A request found without `file` is checked the same way, so a link in the workspace to a file outside it is refused either way |
| `workspace_path` | Folder whose `.env` secrets apply; defaults to the folder the server runs in |
| `env` | Environment to run against |
| `variables` | The values the request's `[required(...)]` attributes ask for, plus `auth_token`. Any other name is rejected, so a call cannot point a request at another host or change what it sends |
| `confirmation_token` | The token from a held request, once the user has agreed to run it |
| `redirect_url` | The address the browser ended on after a sign-in, pasted by the user, when a previous call returned `redirect_url_required: true` |

**Confirmation.** A request marked [`[confirm]`](LANGUAGE_DEFINITION.md#confirm-attribute) — on the request, its endpoint or the environment — needs the user's consent, as it does in the CLI and the extension. So does every write (`POST`, `PUT`, `PATCH`, `DELETE`), marked or not, because nobody is watching the assistant pick it. Unmarked reads run straight away:

- When the client supports [elicitation](https://modelcontextprotocol.io/specification/2025-11-25/client/elicitation), the server asks the user directly ("Run POST http://… against the `production` environment?") and sends the request only on a yes. A no returns `{ status: "declined", sent: false }`. A question left unanswered for five minutes returns `{ status: "confirmation_timeout", sent: false }`, and a tool call the client cancels sends nothing, even if the user says yes afterwards.
- Otherwise nothing is sent and the tool returns `{ status: "confirmation_required", sent: false, request, confirmation_token }`. The assistant must ask the user and call again with the same arguments plus the token. A token works once, for that exact request (same name, file, workspace, environment and variables, and the request unchanged in its file), within five minutes. The server keeps the request as it was prepared when it held it, sign-in token included, and sends exactly that, so a `let`, environment value or `.env` secret changed before the retry cannot send the approved call somewhere else. Canceling the tool call while the request is in flight aborts it.

The tool is annotated as destructive, so clients that gate tools on annotations ask before calling it at all.

**What comes back.** The values of `Authorization`, `Proxy-Authorization`, `Cookie`, `Set-Cookie`, `X-Api-Key` and `X-Auth-Token` are replaced by `[redacted]`, and so is any secret value (from `.env` or an `RQ__` environment variable) of six characters or more found in the URL or another header. Shorter values, such as a port or a retry count, are left alone so they do not garble every header that contains them. The confirmation question the user answers shows the whole URL, so they see where the request goes; the `request` the assistant gets back stays redacted. A body longer than 50,000 characters is cut there with `body_truncated: true`, and the server stops reading the response at that point. The instructions tell the assistant to treat the body as untrusted data, never as instructions. Response bodies are returned as received, so a secret an API echoes back is not redacted.

**Auth.** Bearer tokens and OAuth2 client credentials (with a secret or a certificate) work as they do everywhere else. The flows that need the user to sign in in a browser, authorization code and implicit, depend on the auth's `redirect_uri`. The server never opens a browser itself.

- **`http://localhost:PORT/…` or `http://127.0.0.1:PORT/…`** (authorization code): the server waits for the browser's redirect on a local server on that port, for five minutes.
  - When the client supports URL [elicitation](https://modelcontextprotocol.io/specification/2025-11-25/client/elicitation), the server asks it to show the user the sign-in page, and tells it when the sign-in finished so it can close that page. Once they sign in, the same call sends the request.
  - Otherwise nothing is sent and the tool returns `{ status: "authorization_required", sent: false, request, authorization_url, redirect_url_required: false }`. The assistant shows the URL to the user and calls again with the same arguments once they say they signed in. A call made while the sign-in is still pending returns the same URL.
- **Any other address** (authorization code or implicit): the user signs in and pastes back the address the browser ended on, as in the VS Code extension.
  - When the client supports form elicitation, the server shows the sign-in URL in a dialog with a field for that address, so the address never reaches the assistant. Once they paste it, the same call sends the request.
  - Otherwise the tool returns `authorization_required` with `redirect_url_required: true`, and the assistant calls again with the address the user pasted as `redirect_url`. This route only takes the authorization code flow with PKCE or state on, so a forged or stolen address does not get a token. The implicit flow, whose address carries the token itself, and an auth that turns off both PKCE and state need the dialog.
  - Either way the address must start with the `redirect_uri`, its `state` must match the sign-in, and the code is exchanged with that sign-in's PKCE verifier. An address passed as `redirect_url` that does not start with the `redirect_uri` is rejected and the sign-in keeps waiting for the right one, for five minutes. A `redirect_url` that no sign-in waits for, because it already finished, nobody passed the address within those five minutes or the server restarted, is not used. If the server already holds a token for that auth, as on the retry of a write confirmed after a pasted sign-in, it uses that token; otherwise the tool returns `authorization_failed` saying so, and starts no sign-in.
- **`vscode://`**, the default when `redirect_uri` is missing, only works in the extension: such a request fails with an error.

Every sign-in URL the user has to open themselves, in the dialog or through the assistant, is a short link such as `http://127.0.0.1:53817/a8f3…`, so a client that shows it on one line cannot cut off its `state` or PKCE challenge. A server on a random loopback port answers that one path, for a request to that exact host, with a redirect to the full authorization URL, and closes when the sign-in finishes or after five minutes. It receives nothing from the browser.

A user who refuses a sign-in gets `{ status: "declined", sent: false }`. A sign-in that fails, or an auth the server cannot sign in to, returns `{ status: "authorization_failed", sent: false, request, message }`, where the message says what went wrong and what to do: an address without `state` (most likely a cut-off sign-in link), one from another sign-in, one without a code, a provider that refused the sign-in or the code exchange, a redirect port in use, a sign-in nobody finished within five minutes. A sign-in that fails after the tool already returned `authorization_required`, such as a provider that sends the browser back with an error, is reported by the next call for that auth, within five minutes, instead of a new sign-in link. The sign-in comes before the confirmation, so a write that needs both asks to sign in first. The server keeps tokens in memory only, per auth, the file that defines it and environment, so two workspaces never share one. It refreshes them when the provider issued a refresh token and forgets them when it stops. It shares no tokens with the VS Code extension. Tokens are redacted from the headers it reports. Any request can still be run by passing a token as the `auth_token` variable.

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

- **It runs only what is already written.** `run_request` sends a request that exists in a `.rq` file; the assistant cannot make one up on the fly. It asks the user before every write and before every request marked `[confirm]`.
- **It validates rqlang, not your API.** A file that passes both tools is well-formed and idiomatic. Whether the path exists, the payload matches the schema, or the token carries the right scope is still yours to verify — run the request and read the response.
- **It does not import formats.** There is no OpenAPI, Swagger or Postman importer anywhere in rq. Generating requests from a spec works because the *assistant* reads the file and the server checks the result, which is why it works for any format the assistant can read.
- **It does not make an assistant deterministic.** The rules constrain the shape of the output, not the naming or the reasoning behind it. Read what lands before you commit it.

## See also

- [AI-Assisted Authoring](AI_ASSISTED_AUTHORING.md) — the same ground as a ten-minute tutorial
- [VS Code Extension](VSCODE_EXTENSION.md) — the rest of what the extension provides
- [Language Definition](LANGUAGE_DEFINITION.md) — the grammar the server serves to assistants

{% endraw %}
