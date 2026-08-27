---
layout: default
title: Advanced Example
nav_order: 7
---

{% raw %}

# Advanced Example

[Getting Started](GETTING_STARTED.md) got you running a single request. This guide picks up where that left off and grows it into a small, real-world API suite — one feature per step. Every snippet is a complete `.rq` file, so paste it in and run it as you go.

We'll keep using [httpbin.org](https://httpbin.org) so there's nothing to set up.

## Step 1 — A first request

Start with a single request that fetches a list of users. We'll use [httpbin.org](https://httpbin.org) as the base — its `/anything/*` route accepts any path and echoes the request back, so it stands in for a real API while we build up.

```
rq get_users("https://httpbin.org/anything/users");
```

Run `get_users` from the Request Explorer to fire it off — you'll get a `200` with httpbin's echo of the call. That's our starting point — we'll grow it from here.

## Step 2 — Fetch a single user

Next, add a request that fetches one user by id. For now we'll hardcode the id right in the path:

```
rq get_users("https://httpbin.org/anything/users");
rq get_user("https://httpbin.org/anything/users/1");
```

Now you have two requests side by side: `get_users` for the list and `get_user` for user `1`. Run them independently from the explorer.

Hardcoding `1` works, but it's a value you'll want to change constantly — that's exactly what we'll fix next.

## Step 3 — Extract the id into a variable

Pull that `1` out into a variable with `let`, then interpolate it into the path with `{{ }}`:

```
let user_id = 1;

rq get_users("https://httpbin.org/anything/users");
rq get_user("https://httpbin.org/anything/users/{{user_id}}");
```

Now the id lives in one place. Change `user_id` once and `get_user` follows. Interpolation works anywhere in a string — the URL here, but headers and bodies too.

See [Variables](LANGUAGE_DEFINITION.md#variables) for data types, interpolation, and precedence.

## Step 4 — Extract the base URL

Both requests repeat `https://httpbin.org/anything`. Pull that into its own variable too, and build each URL from it:

```
let base_url = "https://httpbin.org/anything";
let user_id = 1;

rq get_users("{{base_url}}/users");
rq get_user("{{base_url}}/users/{{user_id}}");
```

The base lives in one spot now. Point the whole suite at a different host by editing a single line — which is exactly what you'll want when you start switching between environments.

## Step 5 — Switch the base URL with environments

The same requests usually run against more than one host — a local server while you develop, the real API in production. **Environments** let you define `base_url` per target and switch between them at run time.

Swap the `let base_url` for two `env` blocks:

```
env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

let user_id = 1;

rq get_users("{{base_url}}/users");
rq get_user("{{base_url}}/users/{{user_id}}");
```

The requests don't change — they still read `{{base_url}}`. What changes is *where that value comes from*: whichever environment is active wins. Pick one from the environment selector in the Request Explorer (or pass `-e remote` on the CLI), then run. Choose `remote` to hit httpbin without anything running locally.

One thing to know: now that `base_url` lives only in the environments, you need one selected to run these requests. `user_id` is still a file-level `let`, so it's always there as a fallback.

See [Environments](LANGUAGE_DEFINITION.md#environments) for how environments interact with variables and secrets.

## Step 6 — Require the user id at run time

A fixed `user_id = 1` isn't much of a "get user" — you want to ask for a *different* user each time you run. Drop the `let` and mark the id as **required** with an attribute on `get_user`:

```
env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

rq get_users("{{base_url}}/users");

[required(user_id)]
rq get_user("{{base_url}}/users/{{user_id}}");
```

`[required(user_id)]` says the value must be supplied when the request runs. In VS Code the extension prompts you for `user_id` before firing; on the CLI you pass it with `--var user_id=42`. Leave it out and rq stops with a clear error instead of sending a broken URL.

Note the attribute lives only on `get_user` — `get_users` never touches `user_id`, so it runs untouched.

`required` is just one attribute — see [Attributes](LANGUAGE_DEFINITION.md#attributes) for the full set (`method`, `timeout`, `auth`, and more).

## Step 7 — Group them under an endpoint

Both requests still repeat `/users`, and in the explorer they sit as two unrelated entries. An **endpoint** (`ep`) fixes both: it holds the shared base URL, and its child requests only spell out the part that differs.

Wrap them in `ep users`:

```
env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

ep users("{{base_url}}/users", qs: "version=1") {
  rq list();

  [required(user_id)]
  rq get(user_id);
}
```

The endpoint owns `{{base_url}}/users` **and** a shared query string, `qs: "version=1"`. Both flow down to every child, which only spells out the part that differs:

- `list()` has no path, so it *is* the endpoint URL → `GET {{base_url}}/users?version=1`
- `get(user_id)` appends the id → `GET {{base_url}}/users/{{user_id}}?version=1`

That `qs` is inherited by **every** request in the block — you write `version=1` once on the endpoint instead of repeating it on each request, and any request you add later picks it up for free.

Notice `get(user_id)` passes the variable **directly**, without quotes or `{{ }}`. A bare identifier resolves to its value, and the endpoint joins it onto the base with a single `/` — so this is exactly equivalent to writing `get("/{{user_id}}")`. Reach for interpolation when the value is one piece of a larger string (`"/{{user_id}}/posts"`); use the bare form when the variable *is* the whole argument.

The `[required(user_id)]` attribute still works on the child request — it's only endpoints themselves that can't take `required`. In the Request Explorer the two now nest under a single `users` group, and the CLI addresses them with dot notation: `users.list` and `users.get`.

See [Endpoints](LANGUAGE_DEFINITION.md#endpoints) for endpoint parameters, the shared `qs`, and inherited attributes.

## Step 8 — POST, PUT, and DELETE with a body

Every request so far has been a `GET`. A real users API also creates, replaces, and removes — and the write requests need a payload. Two new ideas come together here: **choosing the HTTP method** and **sending a JSON body**.

rq decides the method in one of two ways:

- **By name** — a request named after a standard verb (`get`, `post`, `put`, `delete`, `patch`, `head`, `options`) uses that method automatically.
- **By attribute** — `[method(VERB)]` sets it explicitly, and overrides the name if both are present.

When neither applies, the method defaults to **GET**.

For the body, define a `${ ... }` JSON literal once as a variable and reuse it for both writes:

```
env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

let user_body = ${
  "id": "{{user_id}}",
  "name": "Ada Lovelace",
  "email": "ada@example.com",
  "role": "admin"
};

ep users("{{base_url}}/users", qs: "version=1") {
  rq list();

  [required(user_id)]
  rq get(user_id);

  [required(user_id)]
  rq post(body: user_body);

  [method(PUT)]
  [required(user_id)]
  rq upsert(user_id, body: user_body);

  [required(user_id)]
  rq delete(user_id);
}
```

How each request gets its method:

- `list` isn't a verb, so it falls back to the default — **GET**.
- `get`, `post`, and `delete` match verb names, so they're inferred as **GET**, **POST**, and **DELETE** with no attribute at all.
- `upsert` is a domain name — rq can't guess a method from it, so on its own it would default to GET. `[method(PUT)]` makes it a **PUT**.

And how the body works:

- `${ ... }` is a **JSON body literal**. When rq sees one it also adds an `Accept: application/json` header for you.
- **Interpolation works inside the body** — `"{{user_id}}"` is filled in at run time, just like in a URL. Note it only resolves inside *string* values, which is why `id` is quoted.
- `post` and `upsert` pass the body by name (`body: user_body`); `upsert` still takes the id positionally for its path, so it carries both.
- Because `user_body` references `{{user_id}}`, `post` needs that value too — so it picks up `[required(user_id)]`. Any request that touches the body inherits its variables.

Rule of thumb: name a request after its verb and rq infers the method; give it a domain name like `upsert` and you spell the method out with `[method(...)]`.

## Step 9 — Read the body from a file

Inlining JSON in a `let` is fine for a handful of fields, but real payloads grow, and you often want to keep them as plain `.json` files you can edit and validate on their own. The `io.read_file` built-in pulls a file's contents in as a string.

Move the payload into a file next to your `.rq` — say `user.json`:

```json
{
  "id": "{{user_id}}",
  "name": "Ada Lovelace",
  "email": "ada@example.com",
  "role": "admin"
}
```

Then read it instead of hand-writing the JSON:

```
env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

let user_body = io.read_file("user.json");

ep users("{{base_url}}/users", qs: "version=1") {
  rq list();

  [required(user_id)]
  rq get(user_id);

  [required(user_id)]
  rq post(body: user_body);

  [method(PUT)]
  [required(user_id)]
  rq upsert(user_id, body: user_body);

  [required(user_id)]
  rq delete(user_id);
}
```

A few things worth knowing:

- **The path is relative to the `.rq` file**, not your shell's working directory — so `"user.json"` lives right beside the request file.
- **Interpolation still runs on the contents.** Notice `user.json` contains `{{user_id}}` — rq reads the file, then resolves the variables inside it, so every run stamps in the current `user_id` exactly like the inline version did.
- **You lose the automatic JSON header.** A `${ ... }` literal tells rq the body is JSON and adds `Accept: application/json` for you; a file read is just a string, so if your API needs it, set the header yourself — e.g. `headers: $["Content-Type": "application/json"]` on `post` and `upsert`.

Only `user_body` changed — every request still says `body: user_body`, none the wiser that the bytes now come from disk.

`io.read_file` is just one of rq's built-in functions — there are others for things like generating IDs and timestamps. See [Functions](LANGUAGE_DEFINITION.md#functions) for the full list.

## Step 10 — Add a header

Requests and endpoints can attach HTTP headers with the `$[ ... ]` dictionary syntax — a set of `"key": "value"` pairs. Put one on the `users` endpoint and every request inside it sends it:

```
env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

let user_body = io.read_file("user.json");

ep users("{{base_url}}/users", headers: $["X-Client": "rq-advanced-example"], qs: "version=1") {
  rq list();

  [required(user_id)]
  rq get(user_id);

  [required(user_id)]
  rq post(body: user_body);

  [method(PUT)]
  [required(user_id)]
  rq upsert(user_id, body: user_body);

  [required(user_id)]
  rq delete(user_id);
}
```

Now `list`, `get`, `post`, `upsert`, and `delete` all send `X-Client: rq-advanced-example`. Header names are case-insensitive, and an individual request can pass its own `headers:` too — endpoint and request headers merge, with the request winning on a clash.

See [Headers](LANGUAGE_DEFINITION.md#parameters-url-headers-body) for the `$[ ... ]` syntax and passing headers positionally or by name.

## Step 11 — Add authentication

Most APIs won't answer without credentials. rq models this with an **auth provider**: declare it once, then attach it to a request — or a whole endpoint — with `[auth(...)]`.

Add a bearer-token provider and hang it off the `users` endpoint:

```
auth token_auth(auth_type.bearer) {
  token: "{{api_token}}",
}

env local {
  base_url: "http://localhost:8080",
  api_token: "local-token-123",
}

env remote {
  base_url: "https://httpbin.org/anything",
  api_token: "remote-token-456",
}

let user_body = io.read_file("user.json");

[auth("token_auth")]
ep users("{{base_url}}/users", headers: $["X-Client": "rq-advanced-example"], qs: "version=1") {
  rq list();

  [required(user_id)]
  rq get(user_id);

  [required(user_id)]
  rq post(body: user_body);

  [method(PUT)]
  [required(user_id)]
  rq upsert(user_id, body: user_body);

  [required(user_id)]
  rq delete(user_id);
}
```

How it works:

- `auth token_auth(auth_type.bearer)` declares a **bearer** provider. Its `token` is sent as an `Authorization: Bearer <token>` header on every request that uses it.
- `[auth("token_auth")]` on the endpoint applies it to **all** children — `list`, `get`, `post`, `upsert`, and `delete` each send the header. Put the attribute on a single `rq` instead if only one request needs it.
- The `token` reads from `{{api_token}}` — and since `api_token` now lives in the `env` blocks, each environment sends its own: pick `local` and it uses the local token, pick `remote` and the credential switches with it.

Bearer is the simplest provider — rq also supports OAuth2 flows (client credentials, authorization code, and more). See [Auth](AUTH.md) for the full set and how to configure each.

One caveat: the tokens are still sitting in the file. Per-environment values like this are a great fit for `env` blocks, but real credentials shouldn't be committed at all — that's what **secrets** are for, and it's where we go next.

## Step 12 — Move the token into a secret

Environments gave each target its own token, but those tokens are still committed in the `.rq` file. **Secrets** fix that: they're variables loaded from *outside* the file, so credentials never land in source control.

The easiest source is a `.env` file next to your `.rq`. Give it one token per environment:

```
ENV__LOCAL__API_TOKEN=local-token-123
ENV__REMOTE__API_TOKEN=remote-token-456
```

The `ENV__<NAME>__` prefix scopes each secret to one environment: `ENV__LOCAL__API_TOKEN` becomes `api_token` when `local` is active, `ENV__REMOTE__API_TOKEN` when `remote` is. (A plain `API_TOKEN=…` with no prefix would apply to every environment.)

Now drop `api_token` from the `env` blocks — the secret takes over:

```
auth token_auth(auth_type.bearer) {
  token: "{{api_token}}",
}

env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

let user_body = io.read_file("user.json");

[auth("token_auth")]
ep users("{{base_url}}/users", headers: $["X-Client": "rq-advanced-example"], qs: "version=1") {
  rq list();

  [required(user_id)]
  rq get(user_id);

  [required(user_id)]
  rq post(body: user_body);

  [method(PUT)]
  [required(user_id)]
  rq upsert(user_id, body: user_body);

  [required(user_id)]
  rq delete(user_id);
}
```

Nothing else moves — the auth provider still reads `{{api_token}}`. Secrets sit **above** environments and file-level `let`s in rq's precedence order, so `{{api_token}}` now resolves from `.env` automatically, per active environment.

A couple of notes:

- **Add `.env` to your `.gitignore`.** The whole point is keeping tokens out of the repo.
- **Keys are UPPER_SNAKE_CASE by convention** and exposed lowercase inside `.rq` (`api_token`); lookups are case-insensitive.
- Prefer not to use a file? The same secret works as an OS environment variable: `RQ__ENV__LOCAL__API_TOKEN=…`.

See [Secrets](LANGUAGE_DEFINITION.md#secrets) for the full rules on sources, naming, and precedence.

## Step 13 — A second endpoint

APIs rarely have just one resource. Add a `widgets` endpoint alongside `users` — read-only for now, with three `GET` requests: `list`, `get` by id, and `search_by_name`.

```
auth token_auth(auth_type.bearer) {
  token: "{{api_token}}",
}

env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

let user_body = io.read_file("user.json");

[auth("token_auth")]
ep users("{{base_url}}/users", headers: $["X-Client": "rq-advanced-example"], qs: "version=1") {
  rq list();

  [required(user_id)]
  rq get(user_id);

  [required(user_id)]
  rq post(body: user_body);

  [method(PUT)]
  [required(user_id)]
  rq upsert(user_id, body: user_body);

  [required(user_id)]
  rq delete(user_id);
}

[auth("token_auth")]
ep widgets("{{base_url}}/widgets", qs: "version=1") {
  rq list();

  [required(widget_id)]
  rq get(widget_id);

  [required(widget_name)]
  rq search_by_name("/search?name={{widget_name}}");
}
```

The three widget requests are all `GET` — none of the names (`list`, `search_by_name`) or the verb-named `get` need a `[method(...)]`:

- `list()` → `GET {{base_url}}/widgets?version=1`
- `get(widget_id)` → `GET {{base_url}}/widgets/{{widget_id}}?version=1`
- `search_by_name("/search?name={{widget_name}}")` → `GET {{base_url}}/widgets/search?name={{widget_name}}&version=1` — the request's own `?name=` query and the endpoint's `qs` ride along together.

One thing should bug you: `{{base_url}}` and `qs: "version=1"` are now written **twice**, once per endpoint. That duplication is a smell — for the moment we'll live with it, but it's exactly what we'll factor out next.

## Step 14 — Extract a templated endpoint

Kill the duplication with a **templated endpoint**. Declare a bodyless parent that holds the shared config, then have each endpoint extend it with `<parent>` and add only its own path:

```
auth token_auth(auth_type.bearer) {
  token: "{{api_token}}",
}

env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

let user_body = io.read_file("user.json");

[auth("token_auth")]
ep api(url: "{{base_url}}", qs: "version=1");

ep users<api>("/users", headers: $["X-Client": "rq-advanced-example"]) {
  rq list();

  [required(user_id)]
  rq get(user_id);

  [required(user_id)]
  rq post(body: user_body);

  [method(PUT)]
  [required(user_id)]
  rq upsert(user_id, body: user_body);

  [required(user_id)]
  rq delete(user_id);
}

ep widgets<api>("/widgets") {
  rq list();

  [required(widget_id)]
  rq get(widget_id);

  [required(widget_name)]
  rq search_by_name("/search?name={{widget_name}}");
}
```

What changed:

- `[auth("token_auth")] ep api(url: "{{base_url}}", qs: "version=1");` is a **template** — a bodyless endpoint that carries nothing but shared config: the base URL, the `qs`, and the auth provider.
- `ep users<api>(...)` and `ep widgets<api>(...)` extend it. Each inherits `api`'s base URL, its `qs`, **and** its auth, then appends its own path — so `{{base_url}}`, `version=1`, and `[auth("token_auth")]` are each written exactly **once**.
- Every resolved URL is identical to before: `users.list` → `GET {{base_url}}/users?version=1`, `widgets.get` → `GET {{base_url}}/widgets/{{widget_id}}?version=1`, and so on.

Because both endpoints use the same provider, the auth attribute rides on the template too: declare `[auth("token_auth")]` once on `api`, and every endpoint that extends it is authenticated automatically — add a third `<api>` endpoint later and it's covered for free. A child that needs *different* auth can still override it with its own `[auth(...)]`.

See [Templated endpoints](LANGUAGE_DEFINITION.md#templated-endpoints) for the full inheritance rules.

## Step 15 — Split across files with imports

Everything so far has lived in a single `.rq` file — convenient for a walkthrough, and perfectly fine to keep that way. But as a project grows you'll often want to break it up, and `import` lets you: it pulls in everything from another `.rq` file — variables, environments, auth providers, endpoints — so you can organize by concern.

Move the shared setup into `shared.rq`:

```
auth token_auth(auth_type.bearer) {
  token: "{{api_token}}",
}

env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

[auth("token_auth")]
ep api(url: "{{base_url}}", qs: "version=1");
```

Then give each resource its own file that imports it:

```
// users.rq
import "shared";

let user_body = io.read_file("user.json");

ep users<api>("/users", headers: $["X-Client": "rq-advanced-example"]) {
  rq list();

  [required(user_id)]
  rq get(user_id);

  [required(user_id)]
  rq post(body: user_body);

  [method(PUT)]
  [required(user_id)]
  rq upsert(user_id, body: user_body);

  [required(user_id)]
  rq delete(user_id);
}
```

```
// widgets.rq
import "shared";

ep widgets<api>("/widgets") {
  rq list();

  [required(widget_id)]
  rq get(widget_id);

  [required(widget_name)]
  rq search_by_name("/search?name={{widget_name}}");
}
```

Both files see `api`, `token_auth`, and the environments as if they were declared inline — the path is relative to the importing file, and the `.rq` extension is optional. Nothing about the behavior changes; it's the same suite, just organized into files that mirror your API.

See [Imports](LANGUAGE_DEFINITION.md#imports) for path resolution and nesting rules.

## Wrapping up

That's the tour. You started with a one-line request and grew it into a full API suite: variables and environments, run-time-required inputs, endpoints with shared query strings and headers, every HTTP method, JSON bodies read from files, bearer auth backed by per-environment secrets, templated endpoints that kill duplication, and a multi-file layout via imports.

For the complete reference on any of these, see the [Language Definition](LANGUAGE_DEFINITION.md).

{% endraw %}
