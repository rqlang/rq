---
layout: default
title: AI-Assisted Authoring
nav_order: 4
---

{% raw %}

# AI-Assisted Authoring

Ten minutes in the chat. Copilot writes the `.rq` files, rq checks every draft before it lands, and then Copilot runs them for you — one at a time, or chained into a procedure you trigger with one sentence.

You need the [VS Code extension](INSTALLATION.md) and nothing else. It ships with an MCP server that gives the assistant rq's grammar and style rules, validates whatever it writes, and runs the requests your workspace already has.

Working with Claude Code instead? Install the [rqlang plugin](MCP_SERVER.md#claude-code-plugin). With Cursor or another assistant, add the same server with `npx -y @rqlang/mcp` (see [Setup](MCP_SERVER.md#setup)). The prompts below work the same way.

**Your results will differ.** An assistant does not answer the same way twice: request names, how files are split and the order it works in can change between runs and between assistants. The checks guarantee the shape — valid rqlang that follows rq's style rules — not the exact text below, which shows one typical run.

## Check it is working

Ask Copilot Chat what rq tools it has. You should see `validate_rq`, `lint_rq`, `list_requests`, `run_request` and `get_rq_reference`. If not, open any `.rq` file to activate the extension.

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

  rq post(body: io.read_json("users-post.json"));

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

  rq post(body: io.read_json("users-post.json"));

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

This one starts from scratch, and so do the steps after it: create a new, empty folder, open it in VS Code and save [`petstore.json`](examples/petstore.json) in it. It is a small Petstore spec whose server is httpbin, so the requests it produces actually run.

> Read `petstore.json` and create the rq files for it.

**rq has no OpenAPI importer** — no command, no tool. The assistant reads the spec with its own file access and the server checks the result, which is why the source format barely matters: a Postman collection or a page of `curl` commands goes the same way.

Three paths and a `bearerAuth` scheme land on the same shape steps 4–6 arrived at by hand. `shared.rq`:

```
env production {
  base_url: "https://httpbin.org/anything",
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

  rq post(body: io.read_json("pets-post.json"));

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

httpbin accepts any token, so the placeholder is enough for the next steps; against a real API, put the real one there. Read the request names and paths before committing. The checks guarantee well-formed rqlang, not that the spec described your API correctly.

## 8. Run them from the chat

Still in the Petstore folder:

> Show me the first 10 pets.

The assistant calls `list_requests`, picks `pets/list` and sends it with `run_request`, `limit` set to `10`, through the same pipeline as the Request Explorer and the CLI: same URL, headers, auth and timeout. It shows which call it made, then answers from the body:

```
GET https://httpbin.org/anything/pets?limit=10 → 200 (143 ms)
```

httpbin has no pets: it echoes your request back, so the body shows what was sent, bearer token included. Against a real API you would get the list.

Follow-ups reuse what is there:

> Now get pet 42.

That is `pets/get` with `pet_id` set to `42`, the value its `[required(pet_id)]` asks for. The assistant can override any variable the request uses for one call, never the request itself: it runs only what is already written in a `.rq` file.

Writes wait for you:

> Delete pet 42.

A `DELETE` changes data, so nothing is sent until you agree. VS Code asks you directly, showing the method, the URL and the environment, as does any client that supports MCP elicitation; in the others the assistant asks you in the chat. `POST`, `PUT` and `PATCH` work the same way, and so does any request, endpoint or environment marked [`[confirm]`](LANGUAGE_DEFINITION.md#confirm-attribute) — mark `env production` once and every call against it asks first. Bearer tokens come from your `.env`; OAuth2 sign-ins in the browser are covered in [`run_request`](MCP_SERVER.md#run_request).

## 9. Chain requests into procedures

A single request answers a question. A sequence of them, mixed with whatever else the assistant can do — read files, compare results, open an issue, write a report — automates a procedure:

> Create a pet, fetch it back and check it matches what was sent, then delete it.

The assistant runs `pets/post`, takes the id from the response, passes it to `pets/get` and `pets/delete`, and reports each call's summary line. The post and the delete are writes, so each still waits for your yes. httpbin stores nothing, so the id it hands back is the one `pets-post.json` sent, echoed under `json`; a real API would return the one it assigned.

Save that procedure once as a skill and it becomes one sentence. A skill is a `SKILL.md` file in a folder named after it: `.github/skills/pets-smoke-test/` for Copilot in VS Code, `.claude/skills/pets-smoke-test/` for Claude Code (VS Code reads that one too). The assistant picks it from its `description`, or you call it as `/pets-smoke-test`:

```markdown
---
name: pets-smoke-test
description: Smoke-test the pets API in an environment. Use after a deploy, or when asked whether pets work.
---

1. Run `pets/post` in the environment the user names.
2. Take the pet's `id` from its response and run `pets/get` with it as `pet_id`.
3. Check that the returned pet matches `pets-post.json`, the body `pets/post` sends.
4. Run `pets/delete` with the same `pet_id`.
5. Report each call's summary line. Stop at the first call that does not return a 2xx and say which one.
```

> Smoke-test pets on production.

The requests stay in `.rq` files, versioned and reviewed with the code, and runnable on their own from the editor or the CLI; the skill only says in what order to call them and what to do with each answer. A change to the API is a change to one `.rq` file, and every procedure that uses it follows.

## What it won't do

- **Invent requests.** It runs only requests already in a `.rq` file, and asks before every write and every request marked `[confirm]`.
- **Check your API.** Valid rqlang in the house style is no guarantee the path exists or the payload fits the schema.

## Where to go next

- [MCP Server](MCP_SERVER.md) — every tool, every style rule, and what the checks do and don't guarantee
- [Advanced Example](ADVANCED_EXAMPLE.md) — the same ground by hand, with the reasoning behind each construct
- [Language Definition](LANGUAGE_DEFINITION.md) — the complete grammar

{% endraw %}
