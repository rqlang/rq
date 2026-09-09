# rqlang Idioms & Style Guide

Style preferences (apply unless the user asks otherwise):

- Prefer the `[required(name)]` attribute over declaring a `let name = …` upfront when the value is supplied at runtime.
- Only introduce an `ep` block when two or more requests share a base URL, auth, or headers. A single standalone request should be a top-level `rq` with the full URL — do not wrap a lone request in an `ep`. **When adding a new request, first call `list_requests` to see what exists; if there is already a request for the same entity (same noun in the URL path), refactor those siblings into a shared `ep` together with the new request rather than appending another top-level `rq`.**
- Put the base URL directly on the `ep`. Avoid splitting it into a base `ep` extended via `ep child<base>(…)` when only one endpoint would extend it — a one-consumer chain is indirection with no payoff.
- **Anything two or more endpoints extending the same template declare identically belongs on the template.** If `ep users<base>("/users", qs: "v=1")` and `ep widgets<base>("/widgets", qs: "v=1")` both carry the same `qs` — or the same `[auth("...")]` — move it up: `[auth("token_auth")] ep base(url: "{{base_url}}", qs: "v=1");`. Children inherit it (the template's `qs` is prepended to theirs, and its auth applies unless a child overrides), so every endpoint extending the template gets it for free, including ones added later. That is the whole point of the template. Keep a setting on a child only when that child genuinely needs a different value from its siblings.
- **Never re-declare on a child what the template already gives it.** A template's `qs` is *prepended* to the child's, not replaced: with `ep base(qs: "v=1")`, writing `ep users<base>("/users", qs: "v=1")` sends `?v=1&v=1` — the parameter goes out twice. Writing `qs: "v=2"` is worse: you get `?v=1&v=2`, because a child cannot override an inherited query parameter, only add to it. If one child needs a different value, take the parameter off the template and declare it per child. A repeated `[auth("...")]` is harmless but pointless — a child needs its own only when it authenticates differently.
- **Once two or more endpoints share a base URL or an auth provider, extract a template endpoint.** Declare `ep base(url: "…", headers: …);` in a shared file, `import` it, and extend it: `ep users<base>("/users")`, `ep widgets<base>("/widgets")`. This is exactly what the extension form is for — the base URL, shared headers and auth get declared once instead of being repeated in every file. Do not copy `{{base_url}}/…` or the same `[auth("…")]` into endpoint after endpoint.
- Put environment-specific values (base URLs, tokens, hostnames) in `env` blocks instead of hard-coded literals.
- **Never hand-write an `Authorization` header.** Authentication is a first-class artifact: declare `auth api_auth(auth_type.bearer) { token: "{{api_token}}", }` once and attach it with `[auth("api_auth")]` on the `ep` or the `rq`. Do not write `headers: $["Authorization": "Bearer {{token}}"]` — rq sends the header for you, `rq auth show` can inspect the provider, and switching to an OAuth2 flow later touches only the `auth` block. The attribute rides on a template endpoint too, so `[auth("api_auth")] ep base(url: "{{base_url}}");` authenticates every endpoint that extends it.
- **Never put a credential literal in a `.rq` file.** `.rq` files are committed; tokens, client secrets, passwords and API keys are not. Put the value in a `.env` file next to the source (`API_TOKEN=…`, or `ENV__LOCAL__API_TOKEN=…` to scope it to one environment) and reference it as `{{api_token}}`. Secrets outrank environments and file-level `let` bindings in the precedence chain, so the reference resolves at run time with nothing else to change. This applies to `auth` fields, `let` bindings, `env` values, header values and request bodies alike.
- **One endpoint per file, named after the endpoint.** `ep users` lives in `users.rq`, `ep widgets` in `widgets.rq`. When a file ends up with two or more `ep` blocks, split it: move each endpoint into its own file and `import` shared definitions from a common file. This keeps each domain easy to find and stops a single file from growing into the whole API.
- When two or more `.rq` files would share the same `env`, `auth`, or `let` definitions, extract the shared pieces into a dedicated file (e.g. `shared.rq` or `envs.rq`) and `import` it from each consumer. Do not duplicate `env` or `auth` blocks across files. **Always use relative import paths** (e.g. `import "shared";`, `import "common/envs";`, `import "../shared";`) — never absolute paths like `"/Users/..."` or `"/etc/..."`, even though the parser accepts them. Absolute paths make the file non-portable across machines and break the project as soon as someone else checks it out. The `.rq` extension is optional.
- Inside an `ep` block, name requests after the verb alone — `list` for GET on the collection, `get` for GET on a single resource, plus `post`, `put`, `patch`, `delete`. The endpoint name already supplies the noun, so do not repeat it: write `rq list()`, not `rq get_widgets()`. Add a descriptive name (with `[method(VERB)]` if needed) only when two requests under the same `ep` share a verb (e.g. `rq create_one` next to `rq create_from_csv`, both POSTing). Outside of an `ep`, use a descriptive name (the noun belongs in the request name).
- For a path parameter that the caller supplies at runtime, pass it as a bare identifier in URL position with `[required(name)]`, rather than declaring a `let` and interpolating with `{{name}}` in a string URL.
- **Interpolation is always double-braced: `{{name}}`.** A single-braced `{name}` is not interpolation — rqlang sends it as literal text, so `rq get("/{widget_id}")` really requests the path `/{widget_id}` and fails against the API. Nothing about single braces is a syntax error, so the parser will not catch it for you. For a URL path parameter prefer the `[required(name)]` + bare identifier form above; use `{{name}}` elsewhere.
- **A query string shared by every request in an `ep` goes on the `ep`, not on each `rq`.** `ep` takes a third parameter, `qs`: `ep users<base>("/users", qs: "v=1")` appends `?v=1` to every child, including the ones whose URL is a path parameter (`/users/1?v=1`). Do not write `rq list("?v=1")` on request after request — and note that **`rq` has no `qs` parameter at all**: its positional arguments are `url`, `headers`, `body`. Passing `rq get(user_id, $["v": "1"])` sends the HTTP header `v: 1` and no query string, which parses cleanly and silently hits the wrong URL. A query parameter that genuinely belongs to one request only is the exception — write it into that request's URL string.
- **Never pass an empty URL string.** Inside an `ep` the endpoint already supplies the URL, so a child request that adds nothing takes no URL argument at all: write `rq list();`, never `rq list("");`.
- For write actions, include a body. Default pattern: `body: io.read_file("<entity>-<verb>.json")` — a JSON fixture next to the .rq file named after the entity and verb (e.g. `users-post.json`, `users-put.json`, `users-patch.json`) so the user has a clear place to edit the payload. POST, PUT, and PATCH should generally have a body; DELETE typically should not. Omit the body only if the user explicitly says the request needs none.
- **JSON body syntax: always use the `${...}` prefix, never a quoted string.** For inline JSON, write `body: ${"name": "alice"}` or `body: ${}` for an empty object. NEVER write `body: "{}"` or `body: "{\"name\": \"alice\"}"` — those send a string body, not JSON, and will break the receiving API. The `${...}` form also auto-adds the `Accept: application/json` header.

## Examples

Single request for an entity — no `ep` needed, descriptive name carries the noun:

```
rq get_widget("http://localhost:8080/widgets/1");
```

Multiple requests for the same entity — refactor into a shared `ep` with verb-only names and bodies on write actions:

```
ep users("http://localhost:8080/users") {
    rq list();

    [required(user_id)]
    rq get(user_id);

    rq post(body: io.read_file("users-post.json"));

    [required(user_id)]
    rq put(user_id, body: io.read_file("users-put.json"));

    [required(user_id)]
    rq patch(user_id, body: io.read_file("users-patch.json"));

    [required(user_id)]
    rq delete(user_id);
}
```

Multi-file split — shared env/auth in one file, domain endpoints in their own files. Use this when more than one `.rq` file would otherwise duplicate the same env or auth block.

shared.rq — the env plus a template endpoint carrying everything the endpoints have in common:

```
env local {
    base_url: "http://localhost:8080",
}

ep base(url: "{{base_url}}");
```

users.rq:

```
import "shared";

ep users<base>("/users") {
    rq list();

    [required(user_id)]
    rq get(user_id);
}
```

widgets.rq:

```
import "shared";

ep widgets<base>("/widgets") {
    rq list();

    [required(widget_id)]
    rq get(widget_id);
}
```

Each endpoint names only its own path segment; the host lives in exactly one place. Had there been only one endpoint, `ep users("{{base_url}}/users")` with no template would be the right shape — the template earns its keep from the second consumer onward.

A query string shared by every request — declared once on the `ep` with `qs`:

```
import "shared";

ep users<base>("/users", qs: "v=1") {
    rq list();

    [required(user_id)]
    rq get(user_id);

    rq post(body: io.read_file("users-post.json"));

    [required(user_id)]
    rq delete(user_id);
}
```

Every child request comes out with `?v=1` — `…/users?v=1` for `list` and `post`, `…/users/1?v=1` for `get` and `delete`.

Avoid this shape:

```
ep users<base>("/users") {
    rq list("?v=1");                        // repeated on every request instead of qs on the ep
    rq post("?v=1", body: ${});             // same

    [required(user_id)]
    rq get(user_id, $["v": "1"]);           // WRONG: rq's 2nd positional arg is headers, not qs —
                                            // this sends the header `v: 1` and no query string
}
```

Shared `qs` and auth belong on the template, not repeated on each child:

```
// shared.rq
[auth("token_auth")]
ep base(url: "{{base_url}}", qs: "v=1");

// users.rq          // widgets.rq
ep users<base>("/users") {          ep widgets<base>("/widgets") {
    rq list();                          rq list();
}                                   }
```

Both come out authenticated and versioned — `…/users?v=1`, `…/widgets?v=1`. Avoid repeating the setting on every child:

```
// users.rq
[auth("token_auth")]
ep users<base>("/users", qs: "v=1") {       // duplicated on widgets.rq too — hoist to base
```

And once it is on the template, do not repeat it on the child as well:

```
// shared.rq
ep base(url: "{{base_url}}", qs: "v=1");

// users.rq
ep users<base>("/users", qs: "v=1") {       // sends ?v=1&v=1 — the child inherits it already
ep users<base>("/users", qs: "v=2") {       // sends ?v=1&v=2 — a child cannot override, only append
```

Authenticated endpoints — the credential comes from `.env`, the header comes from the `auth` provider:

.env (git-ignored, sits next to the `.rq` file):

```
API_TOKEN=ghp_realtokenvalue
ENV__PROD__API_TOKEN=ghp_prodtokenvalue
```

shared.rq:

```
env local {
    base_url: "http://localhost:8080",
}

auth api_auth(auth_type.bearer) {
    token: "{{api_token}}",
}

[auth("api_auth")]
ep base(url: "{{base_url}}");
```

`api_token` is never written in the `.rq` file, and no endpoint extending `base` has to mention auth again.

Avoid these two shapes:

```
// hand-rolled auth header: use an auth provider and [auth("...")]
ep base(url: "{{base_url}}", headers: $["Authorization": "Bearer {{token}}"]);

// credential literal in a committed file: move it to .env and use {{api_token}}
auth api_auth(auth_type.bearer) {
    token: "ghp_realtokenvalue",
}
```

Avoid this shape. Every line below is wrong for the reason marked on it — copy the shape above instead, not this one:

```
let user_id = "1";                      // use [required(user_id)] instead of a let
ep users_base("http://localhost:8080/users");
ep users<users_base>() {                // base-ep extension: put the URL on the ep directly
    rq get_users("");                   // empty URL string: write rq list();
    rq get_user("/{{user_id}}");        // duplicated noun + let-interpolation: write [required(user_id)] rq get(user_id);
    rq post_user("");                   // empty URL string, duplicated noun, and no body
    rq put_user("/{{user_id}}");        // duplicated noun, and no body
    rq patch_user("/{{user_id}}");      // duplicated noun, and no body
    rq delete_user("/{{user_id}}");     // duplicated noun
}
```

Never write single-braced interpolation. This parses cleanly and is silently wrong at runtime:

```
rq get("/{widget_id}");                 // sends the literal path /{widget_id}
```

Also avoid: duplicating the same `env local { base_url: … }` block across `users.rq` and `widgets.rq` instead of extracting it to a shared file.
