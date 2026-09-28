---
layout: default
title: AI-Assisted Authoring
nav_order: 8
---

{% raw %}

# AI-Assisted Authoring

Ten minutes, eight prompts. Copilot writes the `.rq` files; rq checks every draft before it lands.

You need the [VS Code extension](INSTALLATION.md) and nothing else. It ships with an MCP server that gives the assistant rq's grammar and style rules, then validates whatever it writes.

## Check it is working

Ask Copilot Chat what rq tools it has. You should see `validate_rq`, `lint_rq`, `list_requests` and `get_rq_reference`. If not, open any `.rq` file to activate the extension.

## 1. A first request

> Add a request that lists users from `https://httpbin.org/anything/users`.

```
rq get_users("https://httpbin.org/anything/users");
```

One line. Asked for "an API file", most assistants hand back an endpoint block and two environments for a single call — rq's rules say a lone request is a plain `rq` with the full URL.

## 2. A second request, same entity

> Now fetch a single user by id.

`list_requests` reports the `get_users` already there, so this is the second request against `/users`, not a blank page. It restructures instead of appending:

```
ep users("https://httpbin.org/anything/users") {
  rq list();

  [required(user_id)]
  rq get(user_id);
}
```

The base URL lives in one place, requests are named after the verb, and the id is a runtime input rather than a `let`.

## 3. Writes, and a mistake it catches

> Add create and delete.

A first draft usually writes the body the way most API docs show it — `body: "{}"`. That parses, and sends the *string* `{}`. The linter stops it:

```
json_body_as_string
  Inline body is a quoted string that looks like JSON. At runtime this is sent as
  a string, not JSON, and will break receiving APIs.
```

What lands instead:

```
ep users("https://httpbin.org/anything/users") {
  rq list();

  [required(user_id)]
  rq get(user_id);

  rq post(body: io.read_file("users-post.json"));

  [required(user_id)]
  rq delete(user_id);
}
```

The payload goes beside it in `users-post.json`. `delete` correctly has none.

## 4. A second endpoint

> Add a widgets endpoint: list, get by id, and search by name.

Two `ep` blocks in one file is a style error, and two endpoints repeating a base URL is another. Both are cleared by moving code, so you get three files.

`shared.rq`:

```
ep base(url: "https://httpbin.org/anything");
```

`users.rq`:

```
import "shared";

ep users<base>("/users") {
  rq list();

  [required(user_id)]
  rq get(user_id);

  rq post(body: io.read_file("users-post.json"));

  [required(user_id)]
  rq delete(user_id);
}
```

`widgets.rq`:

```
import "shared";

ep widgets<base>("/widgets") {
  rq list();

  [required(widget_id)]
  rq get(widget_id);

  [required(widget_name)]
  rq search_by_name("/search?name={{widget_name}}");
}
```

Ask for `widgets` on its own and you get no template: `base_ep_extension` rejects a chain with a single consumer. It appears exactly when a second endpoint justifies it.

## 5. Environments

> It has to run against localhost too.

Only `shared.rq` changes:

```
env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

ep base(url: "{{base_url}}");
```

`users.rq` and `widgets.rq` are untouched — they extend `base`, so both follow whichever environment is active. Pick one in the Request Explorer, or pass `-e remote` on the CLI.

## 6. Auth

> The API needs a bearer token.

The reflex is an `Authorization` header on the endpoint, which draws `manual_auth_header` and, for the token itself:

```
hardcoded_secret
  `authorization` is set to a literal credential. `.rq` files are committed to
  version control, so the secret leaks with the repository.
```

`shared.rq` gets a provider instead:

```
env local {
  base_url: "http://localhost:8080",
}

env remote {
  base_url: "https://httpbin.org/anything",
}

auth api_auth(auth_type.bearer) {
  token: "{{api_token}}",
}

[auth("api_auth")]
ep base(url: "{{base_url}}");
```

and the credentials a git-ignored `.env`:

```
ENV__LOCAL__API_TOKEN=local-token-123
ENV__REMOTE__API_TOKEN=remote-token-456
```

The endpoint files again do not change. The attribute rides on the template, so everything extending it is authenticated, including endpoints you add later. See [Auth](AUTH.md) for the OAuth2 providers.

## 7. A whole suite from an OpenAPI spec

Drop an `openapi.json` into the workspace:

> Read `openapi.json` and create the rq files for it.

**rq has no OpenAPI importer** — no command, no tool. The assistant reads the spec with its own file access and the server checks the result, which is why the source format barely matters: a Postman collection or a page of `curl` commands goes the same way.

A three-path Petstore spec with a `bearerAuth` scheme lands on the same shape steps 4–6 arrived at by hand. `shared.rq`:

```
env production {
  base_url: "https://api.petstore.example.com/v1",
}

auth bearer_auth(auth_type.bearer) {
  token: "{{api_token}}",
}

[auth("bearer_auth")]
ep base(url: "{{base_url}}");
```

`pets.rq`:

```
import "shared";

ep pets<base>("/pets") {
  [required(limit)]
  rq list("?limit={{limit}}");

  [required(pet_id)]
  rq get(pet_id);

  rq post(body: io.read_file("pets-post.json"));

  [required(pet_id)]
  rq delete(pet_id);
}
```

`owners.rq`:

```
import "shared";

ep owners<base>("/owners") {
  [required(owner_id)]
  rq get(owner_id);
}
```

The base URL comes from `servers`, the provider from `securitySchemes`, one file per resource from rq. The token is not in the spec, so it arrives as a placeholder for you to fill in:

```
ENV__PRODUCTION__API_TOKEN=replace-me
```

Read the request names and paths before committing. The checks guarantee well-formed rqlang, not that the spec described your API correctly.

## 8. Clean up what you already have

> `orders.rq` and `invoices.rq` both repeat the base URL and the auth attribute. Clean that up.

The linter sees the whole workspace, not just the open file, so duplication *between* files is detected rather than guessed at. You get the shared parts extracted into a template and both endpoints rewritten to extend it.

## What it won't do

- **Run anything.** No tool fires a request — use the [Request Explorer](VSCODE_EXTENSION.md#request-explorer) or the [CLI](CLI.md).
- **Check your API.** Valid rqlang in the house style is no guarantee the path exists or the payload fits the schema.
- **Repeat itself.** Names vary between runs. The shape is guaranteed; your intent is not.

## Where to go next

- [MCP Server](MCP_SERVER.md) — every tool, every style rule, and what the checks do and don't guarantee
- [Advanced Example](ADVANCED_EXAMPLE.md) — the same ground by hand, with the reasoning behind each construct
- [Language Definition](LANGUAGE_DEFINITION.md) — the complete grammar

{% endraw %}
