---
name: rqlang-syntax
description: Authoritative reference for the rqlang language (.rq files) — a DSL for managing and executing HTTP requests. Use this skill whenever working with .rq files, writing rqlang request examples, generating test fixtures, documenting rqlang syntax, building parser/linter/formatter logic for rqlang, or implementing the rqlang CLI or VS Code extension. Covers requests, endpoints, variables, environments, secrets, auth providers, attributes, functions, imports, and formatting conventions. Use proactively any time the user mentions rqlang, rq files, .rq, or HTTP request DSLs in the context of this project — do not rely on memory for rqlang syntax.
---

# rqlang Language Reference

This skill is the canonical reference for the rqlang DSL. The source of truth is <https://www.rqlang.com/docs/LANGUAGE_DEFINITION.html>. If anything here conflicts with that page, the page wins — flag the discrepancy and re-fetch.


## When to use this skill

Trigger whenever you encounter:
- Files with extension `.rq`
- Tasks that mention rqlang, rq language, "rq files", `rq` CLI, or the rqlang VS Code extension
- Writing, reading, generating, parsing, linting, formatting, syntax-highlighting, or documenting rqlang
- Building examples, test fixtures, or docs that include rqlang snippets
- Implementing features in the rqlang CLI (Rust) or extension (TypeScript) that touch language semantics

When in doubt — use it. Memory of rqlang is unreliable; this file is the reference.

---

## 1. Formatting & style

- **Identifiers**: snake_case. Applies to request names, `let` variable names, environment names, keys inside `env` blocks, and auth provider names. Allowed chars: `a–z`, `0–9`, `_`. Words separated by `_`.
- **Braces on the same line**: opening brace goes on the declaration line, not on a new line. Applies uniformly to `env`, `ep`, `auth`, and any other brace-block construct.

```rq
env local {
  base_url: "http://localhost:8080",
}
```

## 2. Comments

Two forms:

```rq
// Single-line comment, runs to end of line.
rq get("http://example.com"); // also valid at end of line

/* Block comment
   spans multiple
   lines */
rq get("http://example.com");
```

## 3. The `rq` statement

Declares a named HTTP request.

```rq
rq basic("http://localhost:8080/get");
```

- `rq` — keyword.
- `basic` — request name (snake_case). Used by CLI and VS Code extension to identify the request.
- The string in `(...)` is the URL expression.

### 3.1 HTTP method

Default is `GET`. Two ways to change it:

**(a) Via the `method` attribute:**
```rq
[method(POST)]
rq basic("http://localhost:8080");
```

**(b) Via a request name that matches a standard HTTP verb** (`get`, `post`, `put`, `delete`, `patch`, `head`, `options`). Example:
```rq
rq post("http://localhost:8080");   // interpreted as POST
```

If both are present, attributes override name-based inference.

### 3.2 Parameters

Up to three positional parameters in order: **URL** (required), **headers** (optional), **body** (optional).

#### Positional form

```rq
// URL only
rq basic("http://localhost:8080/get");

// URL + headers
rq get("http://localhost:8080/get", $[
  "header-1": "value-1",
  "header-2": "value 2",
]);

// URL + headers + JSON body
rq post(
  "http://localhost:8080/post-obj",
  $[ "X-Example-Header": "example-value" ],
  ${"greeting": "hello", "value": 123}
);

// URL + headers + string body
[method(POST)]
rq post_string("http://localhost:8080/post-string", $[
  "Content-Type": "text/plain"
], "hello world");
```

Header names are case-insensitive; the runtime normalizes them.

If no body is provided, an empty body is sent.

#### Named-parameter form

Supported names: `url`, `headers`, `body`. Each may appear at most once. Positional + named may be mixed when unambiguous.

```rq
rq named_params_request(
  url: "http://localhost:8080/get",
  headers: $[ "X-Test": "named-params" ],
  body: ${"test": "named_parameters"}
);

rq named_partial(url: "http://localhost:8080/simple");

rq mixed_params("http://localhost:8080/mixed", headers: $[
  "X-Mixed": "positional-url-named-headers",
]);
```

### 3.3 Body types

- **JSON object literal** introduced with `${...}`. When detected as JSON, an `Accept: application/json` header is auto-added if not already present.
- **Plain string literal** — sent as-is (typically `text/plain`).

## 4. Variables

Declared with `let`. Referenced directly or interpolated into strings.

```rq
let base_url = "http://localhost:8080";
rq test_bare_url(base_url);

let host = "http://127.0.0.1:8080";
rq get("{{host}}/path");
```

### 4.1 Data types

#### Strings (including multiline)

Newlines and indentation inside quotes are preserved verbatim.

```rq
let var_body = "Line 1
Line 2";

[method(POST)]
rq multiline_strings(
  url: "http://localhost:8080/post",
  headers: $[
    "Content-Type": "text/plain",
    "X-Multiline": "Line 1
Line 2",
  ],
  body: var_body
);
```

Escape sequences:

| Sequence | Char |
| --- | --- |
| `\"` | `"` |
| `\'` | `'` |
| `\\` | `\` |
| `\n` | newline |
| `\t` | tab |
| `\r` | carriage return |

#### Dictionaries with `$[ ... ]`

Used for headers and any string→string map. Entries are `"key": "value"`. Values support interpolation.

```rq
let default_headers = $[
  "Accept": "application/json",
  "X-App": "rq-demo",
];

rq with_headers("http://localhost:8080/get", default_headers);
```

#### JSON bodies with `${ ... }`

Keys are strings. Values can be numbers, strings, booleans, or nested JSON. String values inside `${...}` support interpolation.

```rq
let payload = ${"greeting": "hello", "value": 123};

[method(POST)]
rq send_json("http://localhost:8080/post-obj", $[], payload);
```

When a `${...}` body is used, `Accept: application/json` is auto-added if not already present.

### 4.2 Variable precedence

From highest to lowest:

1. **Execution-time variables** (passed via CLI `--var` or VS Code extension prompts)
2. **Secrets** (`.env` file + OS env vars)
3. **Environment** (`env name { ... }` blocks)
4. **File-level `let`**

Within a layer, **last definition wins**. Interpolation is applied after the merge. If a name is unresolved in **all** layers, the engine errors out rather than substituting an empty string.

## 5. Built-in functions

Functions are namespaced: `namespace.name(...)`. Invalid namespace/name or wrong arity causes an analysis-time error before any request runs.

### `random.guid()`
Generates a UUID v4 string.
```rq
let id = random.guid();
rq get("http://localhost:8080?id={{id}}");
```

### `datetime.now()` and `datetime.now(format)`
Without args: ISO-like timestamp (e.g. `2024-03-05T12:34:56.789+0100`).
With a format string: strftime-based, plus shortcuts `yyyy`, `MM`, `dd`, `HH`, `mm`, `ss`.

```rq
let d = datetime.now();
let f = datetime.now("yyyy-MM-dd");
rq get("http://localhost:8080?d={{d}}&f={{f}}");
```

### `io.read_file(path)`
Reads a text file **relative to the current `.rq` file** and returns its contents as a string.

```rq
rq sys_body(
  "http://localhost:8080/api/upload",
  $["Content-Type": "text/plain"],
  io.read_file("data.txt"),
);
```

Path may be an interpolated expression:
```rq
let base_filename = "data.txt";
let my_file = base_filename;
rq test_import_reference(
  "http://localhost:8080/api/upload",
  $["Content-Type": "text/plain"],
  io.read_file("{{my_file}}"),
);
```

## 6. Attributes

Annotations in square brackets placed immediately above an `rq` or `ep` statement.

Supported: `method`, `timeout`, `auth`, `required`.

```rq
[method(POST)]
rq basic("http://localhost:8080");

[timeout(10)]
rq slow("http://localhost:8080/get");

[auth("test_auth")]
rq secured("http://localhost:8080/protected");
```

### `method(VERB)`
Overrides the HTTP method regardless of request name. Supported verbs: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`.

### `timeout(seconds)`
Per-request timeout in seconds. Value must resolve to a number, otherwise validation fails before sending.

### `auth("provider_name")`
Attaches an auth provider (see §8). Resolving the name to an empty string disables auth for that request:
```rq
let auth_provider = "";
[auth("{{auth_provider}}")]
rq public_request("http://localhost:8080/public");   // sent without auth
```

### `required(var_name)`
Declares that `var_name` must be supplied at runtime. May appear multiple times for multiple required vars.
```rq
[required(user_id)]
rq get("http://localhost:8080/users/{{user_id}}");

[method(POST)]
[required(user_name)]
[required(user_role)]
rq create(body: ${"name": "{{user_name}}", "role": "{{user_role}}"});
```

Enforcement: CLI errors with a documented exit code; VS Code extension prompts the user.

### Attributes on `ep` vs `rq`

| Attribute | `ep` | `rq` |
| --- | --- | --- |
| `timeout` | yes | yes |
| `auth` | yes | yes |
| `method` | **no** | yes |
| `required` | **no** | yes |

Using `method` or `required` on an `ep` is a parse error.

## 7. Environments

Named groups of variable values, activated at runtime.

```rq
env local {
  base_url: "http://localhost:8080",
}

env dev {
  api_url: "https://dev.api.com",
}

env production {
  api_url: "https://api.com",
}

rq test("{{base_url}}/test");
```

- Multiple environments may coexist in one file.
- `env`-block values override file-level `let` for the same name.
- If a variable resolves only from an environment and none is active, execution errors out.

## 8. Secrets

Secrets are variables loaded **from outside** the `.rq` file. Higher precedence than env/let, lower than execution-time vars.

### Sources

#### `.env` file (next to the source file or in its directory tree)
```
API_KEY=secret-123
API_URL=https://api.example.com

ENV__LOCAL__API_KEY=local-secret
```

- Plain `KEY=VALUE` → applies to all environments.
- `ENV__<ENV_NAME>__<VAR>=...` → applies only when `<ENV_NAME>` is the active environment, overriding the generic key.

#### OS environment variables
- `RQ__NAME=VALUE` → secret `NAME` for all environments.
- `RQ__ENV__<ENV_NAME>__<NAME>=VALUE` → secret `NAME` for environment `<ENV_NAME>`.

Names are normalized to lowercase / snake_case when exposed inside `.rq` files. Lookups are case-insensitive. By convention, write the OS/`.env` keys in UPPER_SNAKE_CASE.

Usage inside `.rq`:
```rq
rq get("{{api_url}}/status", $[
  "Authorization": "Bearer {{api_key}}",
]);
```

## 9. Endpoints (`ep`)

An `ep` groups multiple `rq` requests under a shared base URL and shared defaults.

```rq
let user_id = 123;

ep users("http://localhost:8080/api/users") {
  rq list();                          // GET http://localhost:8080/api/users
  rq get("/{{user_id}}");             // GET http://localhost:8080/api/users/123
}
```

### Parameters
Same as `rq` **minus body**: `url`, `headers`, `qs` (query string). Apply as defaults to child requests; child-level headers and query params merge with the endpoint defaults.

```rq
let u = "http://localhost:8080";
let h = $["X-Test": "true"];
let q = "foo=bar";

ep e1(u, h, q) {
  rq get("/get");
}

ep e2(url: u, headers: h, qs: q) {
  rq get("/get");
}

ep api("http://localhost:8080/api", qs: "api-version=1") {
  rq get("/users");                   // /api/users?api-version=1
}
```

### Attributes inside endpoints
`timeout` and `auth` on `ep` apply as defaults to all child requests; child `rq` can override.

```rq
[timeout(20)]
ep users("http://localhost:8080/api/users") {
  [timeout(10)]
  rq list();                          // overrides to 10s

  [auth("user_token")]
  rq get("/{{user_id}}");             // inherits 20s timeout
}
```

### Templated endpoints
An endpoint can extend another with `<base>` syntax, inheriting its config.

```rq
let user_id = 123;
ep base(url: "http://localhost:8080", headers: $["X-Base": "1"], qs: "v=1");

ep users<base>("/users") {
  rq get("/{{user_id}}");
}

ep widgets<base>("/widgets") {
  rq list();
  rq list_by_user("/by-user/{{user_id}}");
}
```

## 10. Auth providers

Declared at the top level. Referenced from requests/endpoints via `[auth("name")]`.

```rq
auth my_auth(auth_type.bearer) {
  token: "{{api_key}}",
}

[auth("my_auth")]
rq get_protected("https://api.example.com/protected");
```

- `auth` — keyword.
- Provider name in snake_case.
- Type identifier from `auth_type.<kind>`.
- Block lists config fields. Field values can be string literals or identifiers (an identifier `foo` is interpreted as `{{foo}}` and resolved through the variable precedence chain).

### Supported types

#### `auth_type.bearer`
- Required: `token` → sent as `Authorization: Bearer <token>`.

#### `auth_type.oauth2_client_credentials`
- Required: `client_id`, `token_url`.
- Optional: `client_secret`, `scope`, `cert_file`, `cert_password`.
- Two modes:
  - **Client secret**: provide `client_secret` (and optionally `scope`).
  - **Certificate**: provide `cert_file` (and optionally `cert_password`, `scope`); omit `client_secret`.

#### `auth_type.oauth2_authorization_code`
- Required: `client_id`, `authorization_url`, `token_url`.
- Optional: `client_secret`, `redirect_uri`, `scope`, `code_challenge_method` (`S256` default, or `plain`), `use_state`.
- Default `redirect_uri` (when omitted): `vscode://rq-lang.rq-language/oauth-callback`.
- For non-interactive use (CLI), pass a precomputed token via the reserved runtime variable `auth_token` to skip the interactive flow.

#### `auth_type.oauth2_implicit`
- Required: `client_id`, `authorization_url`.
- Optional: `redirect_uri`, `scope`.
- Same `auth_token` runtime variable override as authorization_code.

All auth types are validated at parse time. Missing required fields, unexpected fields, or invalid values (e.g. empty bearer token) cause errors before any request executes.

## 11. Imports

Pulls in everything from another `.rq` file: requests, variables, environments, auth providers, endpoints.

```rq
// base.rq
let user_id = 123;
auth my_auth(auth_type.bearer) {
  token: "{{api_key}}",
}
```

```rq
// main.rq
import "base";

let host = "http://localhost:8080";

[auth("my_auth")]
rq get("{{host}}/users/{{user_id}}");
```

Rules:
- Path resolved relative to the importing file.
- Extension optional: `import "shared";` and `import "shared.rq";` both work.
- Nested imports supported; **circular imports are not**.
- Think of imports as textual merge into one logical file, still subject to duplicate-name validation (e.g. duplicate request or auth names error out).

---

## Quick syntax cheat sheet

| Construct | Syntax |
| --- | --- |
| Request | `rq name(url, headers?, body?);` |
| Named params | `rq name(url: "...", headers: $[...], body: ${...});` |
| Headers literal | `$[ "K": "V", ... ]` |
| JSON body literal | `${ "k": "v", "n": 123 }` |
| Variable | `let name = value;` |
| Interpolation | `"{{name}}"` |
| Environment | `env name { key: "value", }` |
| Endpoint | `ep name(url, headers?, qs?) { rq ...; }` |
| Endpoint template | `ep child<parent>("/path") { ... }` |
| Auth provider | `auth name(auth_type.kind) { field: value, }` |
| Import | `import "other";` |
| Attribute | `[name(arg)]` above `rq` or `ep` |
| Comment | `// line` or `/* block */` |

## Things that cause errors (parse / analysis time, before any request)

- Identifiers not in snake_case where required.
- Duplicate request, endpoint, or auth provider names (including across imports).
- Circular imports.
- `method` or `required` attributes on `ep`.
- Unknown function namespace, unknown function name, or wrong arity (e.g. `datetime.now(a, b)`).
- Auth provider missing required fields, with unexpected fields, or with an empty bearer `token`.
- Unresolved variable name (not in any precedence layer).
- `timeout(...)` value that does not resolve to a number.
- More than one `url`, `headers`, or `body` named parameter on a single `rq`.

## Source

Authoritative document: <https://www.rqlang.com/docs/LANGUAGE_DEFINITION.html>
Related docs: <https://www.rqlang.com/docs/CLI.html>, <https://www.rqlang.com/docs/VSCODE_EXTENSION.html>
