# Language Definition

This document describes the rq language. It is a work in progress and will evolve as the language stabilizes.

## Formatting & Style

rq has a small set of formatting conventions intended to keep files readable and consistent:

- **Identifiers in snake_case**: Request names, variable names (`let`), environment names (`env`), and keys inside `env` blocks must use snake_case: all lowercase, alphanumeric characters (`a`–`z`, `0`–`9`) and underscores (`_`), with words separated by `_`.
- **Braces on the same line**: Blocks such as `env` and `ep` open their braces on the same line as the declaration:

	```rq
	env local {
	  base_url: "http://localhost:8080",
	}
	```

	This style applies uniformly across the language for any construct that uses braces.

## Comments

rq supports two types of comments:

1. **Single-line comments**: Start with `//` and extend to the end of the line.

	```rq
	// This is a single-line comment
	rq get("http://example.com"); // Comment at the end of a line
	```

2. **Block comments**: Enclosed in `/*` and `*/`. They can span multiple lines.

	```rq
	/* This is a comment
	   that spans multiple
	   lines */
	rq get("http://example.com");
	```

## The `rq` Statement

An `rq` statement declares a named HTTP request and specifies how it should be executed (URL, headers, body, etc.).

At its simplest, a request looks like this:

```rq
// A basic GET request
rq basic("http://localhost:8080/get");
```

Conceptually:

- `rq` is the keyword that introduces a request.
- `basic` is the **request name**. It is how the request is identified in tools like the CLI or the VS Code extension.
- The string in parentheses is the **URL expression** that determines where the request is sent.

When executed, this request performs an HTTP `GET` to `http://localhost:8080/get`.

### HTTP Method

By default, the HTTP method is **GET**. You can change the method in two ways:

1. **Via an attribute** attached to the request:

   ```rq
   [method(POST)]
   rq basic("http://localhost:8080");
   ```

   This sends an HTTP `POST` to `http://localhost:8080/`.

2. **Via the request name when it matches a standard method name** (e.g. `get`, `post`, etc.). For example, the following is interpreted as a `POST` request, even without an explicit attribute:

   ```rq
   rq post("http://localhost:8080");
   ```

The exact resolution rules and supported attributes will be documented in a dedicated [Attributes](#attributes) section. For now, you can think of `rq` as always producing a concrete HTTP method for each request.

### Parameters: URL, Headers, Body

An `rq` statement can take up to three main parameters:

1. **URL** (required)
2. **Headers** (optional)
3. **Body** (optional)

These parameters can be passed **positionally** or using **named parameters**.

#### Positional form

The positional form is concise and works well for simple cases:

```rq
// URL only
rq basic("http://localhost:8080/get");

// URL + headers
rq get("http://localhost:8080/get", [
	"header-1": "value-1",
	"header-2": "value 2",
]);

// URL + headers + JSON body (object)
rq post(
	"http://localhost:8080/post-obj",
	[
		"X-Example-Header": "example-value",
	],
	${"greeting":"hello","value":123}
);

// URL + headers + string body
rq post("http://localhost:8080/post-string", [
	"Content-Type": "text/plain"
], "hello world");
```

Semantics in these examples:

- The first argument is always the URL expression.
- The second argument, when present, is a **headers map**. Each entry is a header name/value pair. Header names are case-insensitive; the runtime will typically normalize them.
- The third argument, when present, is the **body**. It can be:
	- A JSON-like object literal introduced with `${...}` (sent as JSON; when JSON content is detected, an `Accept: application/json` header is automatically added if not already present).
  - A plain string literal (sent as-is, usually with `text/plain`).

If no body is provided, an empty body is sent.

#### Named-parameter form

For more complex requests, you can use named parameters, which make the role of each argument explicit and allow omitting any of them:

```rq
// Fully named
rq named_params_request(
	url: "http://localhost:8080/get",
	headers: [
		"X-Test": "named-params",
	],
	body: ${"test": "named_parameters"}
);

// Only URL as named parameter
rq named_partial(url: "http://localhost:8080/simple");

// Mix: positional URL + named headers
rq mixed_params("http://localhost:8080/mixed", headers: [
	"X-Mixed": "positional-url-named-headers",
]);
```

Rules for named parameters:

- Supported names for `rq` are currently: `url`, `headers`, and `body`.
- Each of these parameters may appear **at most once** in a given request.
-- You may mix positional and named arguments, but the effective meaning must be unambiguous. A common pattern is positional `url` plus named `headers` and/or `body`.

## Variables

rq supports variables that you can define with `let` and then reuse in `rq` statements for URLs, headers, and bodies.

```rq
let base_url = "http://localhost:8080";

// Uses the value of base_url ("http://localhost:8080")
rq test_bare_url(base_url);

// Direct string literal
rq test_bare_path("http://localhost:8080/api/test");
```

Variables can also be combined with interpolation syntax inside strings to build URLs and other values:

```rq
let host = "http://127.0.0.1:8080";

rq get("{{ host }}");

let inline = "yes";
let h = [
	"Accept": "application/json",
	"X-Inline": "{{ inline }}",
];

rq inline_mix("{{host}}/get", h);
```

In these examples:

- `let` defines variables whose values can be referenced later.
- `{{ ... }}` performs inline interpolation of the variable into a string.
- You can use variables directly (e.g. `test_bare_url(host)`) or inside interpolated strings (e.g. `"{{host}}"`).

### Data types

Variables can hold several kinds of values that appear throughout rq files: strings, header-style dictionaries, and JSON bodies.

#### Strings (including multiline)

The most common values in rq are strings, used in URLs, headers, bodies, and environment values.

String literals can span multiple lines. Newlines and indentation inside the quotes are preserved in the final value, which makes it convenient to work with multiline content:

```rq
let var_body = "Line 1
Line 2";

[method(POST)]
rq multiline_strings(
	url: "http://localhost:8080
	/post",
	headers: [
		"Content-Type": "text/plain",
		"X-Multiline": "Line 1
Line 2",
		"X-Indented": "Line 1
		Line 2",
		"X-Var": "{{var_body}}",
	],
	body: "Line 1
Line 2",
);
```

In this example the line breaks in `var_body`, the URL, headers, and body are all kept as-is when the request is sent.

#### Dictionaries with `[...]`

The `headers` parameter in `rq` and `ep` uses a dictionary-like literal written with square brackets and `"key": "value"` pairs. These values can also be stored in variables:

```rq
let default_headers = [
	"Accept": "application/json",
	"X-App": "rq-demo",
];

rq with_headers("http://localhost:8080/get", default_headers);
```

These dictionaries are typically used for HTTP headers, but the structure is general: a map from string keys to string values, where values can also include interpolations like `"{{inline}}"`.

#### JSON bodies with `${...}`

For request bodies, rq supports JSON object literals introduced with `${...}`. You can pass them directly as the `body` parameter or assign them to variables:

```rq
let payload = ${"greeting": "hello", "value": 123};

rq send_json(
	"http://localhost:8080/post-obj",
	[],
	payload,
);
```

Keys must be strings, and values can be numbers, strings, booleans, or nested JSON structures. String values inside `${...}` also support interpolation, so you can write entries like `"token": "{{api_token}}"`.

When the system detects a JSON body defined with `${...}`, it will automatically add an `Accept: application/json` header to the request if that header is not already present.

Variables follow an override model: the same name can be defined in several places, and higher-precedence sources overwrite lower-precedence ones.

**Precedence summary (from highest to lowest):**

`execution-time variables` **>** `secrets` **>** `environment` **>** `file let`

If a variable name cannot be found in **any** of these sources, the engine will treat it as an error and fail the request rather than silently falling back to an empty value.

Environments and secrets are described in detail in the later sections [Environments](#environments) and [Secrets](#secrets).

In more detail, rq combines variables from these layers, in **increasing precedence**:

1. **Local file definitions (`let`)**: Defaults defined in the current file (including block-local and surrounding `let` bindings).
2. **Environment values**: The active **environment** (for example, values defined in `env local { ... }`) can override file-level `let` bindings with the same name (see [Environments](#environments)).
3. **Secrets**: Configured **secret providers** can override both environment values and file-level definitions (see [Secrets](#secrets)).
4. **Execution-time variables**: **Runtime variables** passed to the execution engine (for example, via the CLI or VS Code extension) have the highest precedence and can override all previous sources.

Within each layer, if a variable with the same name is defined multiple times, the **last definition wins**. Interpolation (e.g. `"{{host}}"`) is applied after this merge, using the final value obtained after all overrides.

## Functions

rq includes a small set of built-in functions that you can call in expressions (for example in `let` bindings, URLs, headers, or bodies). Functions are namespaced using the form `namespace.name(...)`.

The currently supported namespaces and functions are:

- `random.guid()`
- `datetime.now()` and `datetime.now(format)`
- `io.read_file(path)`

### `random.guid()`

Generates a new random GUID/UUID v4 as a string:

```rq
let id = random.guid();
rq get("http://localhost:8080?id={{id}}");
```

### `datetime.now()`

Returns the current local date-time as a formatted string.

- Without arguments, it produces an ISO-like timestamp (e.g. `2024-03-05T12:34:56.789+0100`).
- With a `format` string, it uses a simplified pattern based on `strftime`, with a few shortcuts like `yyyy`, `MM`, `dd`, `HH`, `mm`, and `ss` that are internally mapped to the appropriate `strftime` specifiers.

```rq
let d = datetime.now();
let f = datetime.now("yyyy-MM-dd");

rq get("http://localhost:8080?d={{d}}&f={{f}}");
```

### `io.read_file()`

Reads the contents of a text file relative to the current `.rq` file and returns it as a string:

The path argument can be relative; its base context is always the directory of the current `.rq` file.

```rq
// data.txt lives next to this .rq file
rq sys_body(
	"http://localhost:8080/api/upload",
	["Content-Type": "text/plain"],
	io.read_file("data.txt"),
);
```

You can also pass interpolated file names:

```rq
let base_filename = "data.txt";
let my_file = base_filename;

rq test_import_reference(
	"http://localhost:8080/api/upload",
	["Content-Type": "text/plain"],
	io.read_file("{{my_file}}"),
);
```

Unknown function namespaces or names, or invalid arguments (for example calling `datetime.now` with more than one argument) will result in errors during analysis before any request is executed.

## Attributes

Attributes are annotations written in square brackets that modify how a request behaves. They are placed immediately above an `rq` statement:

```rq
[method(POST)]
rq basic("http://localhost:8080");

[timeout(10)]
rq get("http://localhost:8080/get");

[auth("test_auth")]
rq secured("http://localhost:8080/protected");
```

The following attributes are currently supported:

- `method`
- `timeout`
- `auth`

### `method` attribute

The `method` attribute overrides the HTTP method used for a request, regardless of the request name:

```rq
[method(POST)]
rq basic("http://localhost:8080");
```

Even though the request is named `basic`, it will be sent as an HTTP `POST`. This attribute is useful when you want descriptive request names that are not tied to the HTTP verb, or when you need to override the method inferred from the name.

The currently supported standard HTTP methods are:

- `GET`
- `POST`
- `PUT`
- `DELETE`
- `PATCH`
- `HEAD`
- `OPTIONS`

### `timeout` attribute

The `timeout` attribute sets a per-request timeout (in seconds) for the HTTP call:

```rq
[timeout(10)]
rq get("http://localhost:8080/get");
```

If the request does not complete within the configured timeout, the execution engine will treat it as a timeout error. The value passed to `timeout(...)` must be a valid number (or an expression that resolves to one); otherwise the engine will fail with a validation error before sending the request.

### `auth` attribute

The `auth` attribute associates an authentication configuration with a request. Its exact behavior and supported providers are described in the [Auth](#auth) section.

## Environments

Environments allow you to group variable values under a named context (such as `local`, `dev`, or `production`) and then run the same `.rq` file against different backends or settings without changing the file itself.

### Declaring environments

You declare an environment with the `env` keyword, followed by the environment name and a block of key–value pairs.

```rq
env local {
	base_url: "http://localhost:8080",
}

rq test("{{base_url}}/test");
```

In this example:

- `local` is the **environment name**.
- Inside the braces, each `name: value` entry defines a variable that is available when the `local` environment is active.
- The `rq test` request uses `{{base_url}}`, which will be resolved from the active environment (or overridden by a higher-precedence source, such as secrets or execution-time variables).

You can declare multiple environments in the same file:

```rq
env dev {
	api_url: "https://dev.api.com",
}

env staging {
	api_url: "https://staging.api.com",
}

env production {
	api_url: "https://api.com",
}

rq test("{{api_url}}/test");
```

The rq tools (CLI and VS Code extension) can list available environment names and let you choose which one to activate when running requests.

**If no environment is provided and a variable can only be resolved from an environment, the system will return an error because the variable cannot be found.**

### How environments interact with variables

As described in [Variables](#variables), environments participate in the variable precedence chain:

- Values defined inside an `env` block override file-level `let` bindings with the same name.
- Secrets and execution-time variables can then override those environment values.

This makes it natural to define sensible defaults with `let`, then specialize them per environment using `env`, and finally apply sensitive or deployment-specific overrides via secrets and runtime parameters.

## Secrets

Secrets in rq are variables that **do not live in the `.rq` file itself**, but are injected from external sources. They are meant for values you typically do not want to commit to source control, such as API keys, tokens, or passwords.

Secrets participate in the same variable system as `let` and `env`:

- They are resolved using the same interpolation syntax (`{{name}}`).
- They have higher precedence than environments and file-level `let` bindings, but lower precedence than execution-time variables (see [Variables](#variables)).

### Where secrets come from

rq loads secrets from two main places associated with the source file being executed:

1. A **`.env` file** located next to the source file (or in its directory tree).
2. **Operating system environment variables**.

Both sources are merged following the same "last definition wins" rule used elsewhere.

#### `.env` files

A `.env` file is a simple `KEY=VALUE` file. For example:

```bash
API_KEY=secret-123
API_URL=https://api.example.com

ENV__LOCAL__API_KEY=local-secret
```

Semantics:

- Plain `KEY=VALUE` entries define secrets that apply to **all environments**.
- Entries prefixed with `ENV__<ENV_NAME>__` (for example `ENV__LOCAL__API_KEY`) define secrets that apply **only to that environment** and override the general value for the same key.

The choice of `.env` and the `ENV__<ENV_NAME>__<VAR>` convention is intentional:

- `.env` files are the de facto standard for storing configuration and secrets next to code, and most common `.gitignore` templates already exclude them from version control.
- The `KEY=VALUE` format is deliberately simple but restrictive in the characters and structure it supports. Encoding both the environment name and the variable name into a single key using `ENV__<ENV_NAME>__<VAR>` works within those constraints while still allowing per-environment overrides without changing the `.rq` language syntax.

Inside rq files you can then use these names with interpolation:

```rq
rq get("{{api_url}}/status", [
	"Authorization": "Bearer {{api_key}}",
]);
```

When running with the `local` environment active, `ENV__LOCAL__API_KEY` (if present) will override the generic `API_KEY` value.

#### OS environment variables

rq also reads from the process environment using a dedicated naming convention:

- `RQ__NAME=VALUE` defines a secret named `NAME` for **all environments**.
- `RQ__ENV__ENV__NAME=VALUE` defines a secret named `NAME` for a specific environment `ENV`.

Environment keys and OS environment variables are typically written in **UPPERCASE** for readability and alignment with common conventions. Internally, rq normalizes variable names to lowercase when resolving them so that lookups are case-insensitive and behave consistently across `.env` files, OS variables, and other sources. **When these variables are exposed inside rq files, the system will convert their names to snake_case following the recommended style for identifiers.**

These variables are treated like entries from a `.env` file, but live in the OS environment instead of on disk. They are especially useful in CI/CD systems, secret managers, or local shells where you do not want to create or commit a `.env` file.

### How secrets interact with other variables

As described in [Variables](#variables), secrets sit above environments and file-level `let` bindings in the same precedence chain summarized earlier:

`secrets` **>** `environment` **>** `file let`

This lets you keep non-sensitive defaults in `.rq` files or environments, and move actual credentials into `.env` files or OS variables, knowing that the secret value will override the less-sensitive default at runtime.

## Endpoints

In rq, an endpoint represents a concrete HTTP endpoint in your API (for example `/users`). Inside an endpoint block you define one or more `rq` requests that represent the different **actions** you can perform against that HTTP endpoint (such as listing, creating, or deleting resources), all sharing a common base configuration.

At its simplest, an endpoint looks like this:

```rq
let user_id = 123;

ep users("http://localhost:8080/api/users") {
    rq list();
	rq get("/{{user_id}}");
}
```

Here:

- `users` is the endpoint name.
- The first argument is the **base URL**.
- The block contains two `rq` statements that define requests relative to the endpoint:
  - `rq list()` resolves to `http://localhost:8080/api/users` (a GET request to the base URL).
  - The `rq get("/{{user_id}}")` request is defined inside the endpoint and is resolved relative to the base URL, resulting in a call to `http://localhost:8080/api/users/123`.

### Endpoint parameters: URL, headers, query string

An endpoint can take the same kinds of parameters as an `rq` request (except body), but they apply as **defaults** to all child requests:

```rq
let u = "http://localhost:8080";
let h = ["X-Test": "true"];
let q = "foo=bar";

ep e1(u, h, q) {
	rq get("/get");
}

ep e2(url: u, headers: h, qs: q) {
	rq get("/get");
}
```

Semantics:

- `url` sets the base URL used to resolve child request paths.
- `headers` defines headers that are added to every child request.
- `qs` appends query string parameters to all child requests.

Child requests can add more headers or query parameters; these are merged with the endpoint defaults.

You can also pass query string defaults using named parameters:

```rq
ep api("http://localhost:8080/api", qs: "api-version=1") {
	rq get("/users");
}
```

This produces a request to `/api/users?api-version=1`.

### Attributes inside endpoints

The `rq` requests defined inside an endpoint block support the same attributes as any other `rq` statement (such as `timeout` or `auth`). These attributes are attached to each action and are evaluated together with the endpoint configuration.

```rq
[timeout(20)]
ep users("http://localhost:8080/api/users") {
	[timeout(10)]
	rq list();

	[auth("user_token")]
	rq get("/{{user_id}}");
}
```

In this example, the endpoint `users` defines a base timeout of `20` seconds. The `rq list` action overrides that timeout with `10` seconds, while `rq get` does not specify a timeout and therefore uses the endpoint-level timeout of `20` seconds. Both actions still share the same base URL, and `get` also applies the `auth` configuration.

### Templated endpoints

Endpoints can be used as **templates** and extended by other endpoints using a simple templated-like syntax:

```rq
let user_id = 123;
ep base(url: "http://localhost:8080", headers: ["X-Base": "1"], qs: "v=1");

ep users<base>("/users") {
	rq get(user_id);
}

ep widgets<base>("/widgets") {
	rq list();
    rq list_by_user("/by-user/{{user_id}}");
}
```

In this pattern:

- `base` defines a reusable endpoint template with the common base URL, headers, and query string used across your API.
- `users<base>("/users")` extends `base`, keeping its configuration and adding the `/users` path segment; the `rq get` action then operates on the `/users` endpoint.
- `widgets<base>("/widgets")` does the same for the `/widgets` endpoint, where the `rq list` action uses the same base configuration but a different path.

Endpoint inheritance allows you to factor out common base URLs and headers while still customizing subsets of requests.

## Auth

Authentication in rq is configured through **auth providers**. An auth provider describes how to obtain credentials (for example a bearer token or an OAuth2 access token) and can then be attached to any request using the `[auth("name")]` attribute.

At a high level, you:

1. Declare one or more auth providers with the `auth` keyword.
2. Reference them from requests (or endpoint actions) using the `auth` attribute.

### Declaring auth providers

An auth provider is declared at the top level of an `.rq` file:

```rq
auth my_auth(auth_type.bearer) {
	token: "{{api_token}}",
}

[auth("my_auth")]
rq get_protected("https://api.example.com/protected");
```

Structure:

- `auth` is the keyword.
- `my_auth` is the **auth provider name** (snake_case, like other identifiers).
- `auth_type.bearer` selects the type of authentication.
- The block `{ ... }` lists configuration fields required by that auth type.

Field values can be:

- A string literal, e.g. `"my-secret-token"`.
- An identifier, e.g. `token_url: mock_url_var`, which is interpreted as `"{{mock_url_var}}"` and resolved using the same variable precedence rules as the rest of the language.

### Supported auth types

rq currently supports several auth types, each with its own set of required and optional fields.

#### Bearer token

- **Type identifier**: `auth_type.bearer`
- **Required fields**:
	- `token`: The bearer token value that will be sent as `Authorization: Bearer <token>`.
- **Optional fields**: none.

In practice you will usually supply `token` via variables and secrets rather than hard-coding it in the file.

#### OAuth2 client credentials

- **Type identifier**: `auth_type.oauth2_client_credentials`
- **Required fields**:
	- `client_id`: OAuth2 client identifier.
	- `token_url`: URL of the token endpoint.
- **Optional fields**:
	- `client_secret`: Client secret used to authenticate with the token endpoint.
	- `scope`: Space-separated list of scopes to request.
	- `cert_file`: Path to a certificate file for certificate-based client authentication.
	- `cert_password`: Password for the certificate file, if needed.

This flow always performs a `client_credentials` grant against `token_url` and then uses the returned access token as a bearer token on protected requests. You can use it in two main modes:

- **Client secret mode**: provide `client_secret` (and optionally `scope`). rq will authenticate the client using `client_id` + `client_secret`.
- **Certificate mode**: provide `cert_file` (and optionally `cert_password` and `scope`) but omit `client_secret`. rq will authenticate the client using the configured certificate instead of a shared secret.

#### OAuth2 authorization code

- **Type identifier**: `auth_type.oauth2_authorization_code`
- **Required fields**:
	- `client_id`: OAuth2 client identifier.
	- `authorization_url`: URL where the user authorizes the client.
	- `token_url`: URL of the token endpoint used to exchange the authorization code for an access token.
- **Optional fields**:
	- `client_secret`: Client secret, when required by the authorization server.
	- `redirect_uri`: Redirect URI registered for the client. If omitted, rq will default to `vscode://rq.rq-language/oauth-callback` to integrate with the VS Code extension flow (see the explanation in [VSCODE_EXTENSION.md](VSCODE_EXTENSION.md#default-redirect-uri)).
	- `scope`: Space-separated list of scopes to request.
	- `code_challenge_method`: PKCE code challenge method. If omitted, rq defaults to `S256`.
	- `use_state`: Whether to use the `state` parameter for CSRF protection.

This flow is designed to work with interactive authorization (via the VS Code extension). For non-interactive scenarios (for example, running the same `.rq` file from the CLI), you can provide a precomputed bearer token at runtime using the reserved variable `auth_token`; when present, rq will use this token directly instead of performing the interactive flow.

#### OAuth2 implicit

- **Type identifier**: `auth_type.oauth2_implicit`
- **Required fields**:
	- `client_id`: OAuth2 client identifier.
	- `authorization_url`: URL where the user authorizes the client and receives an access token directly.
- **Optional fields**:
	- `redirect_uri`: Redirect URI registered for the client. If omitted, rq will default to `vscode://rq.rq-language/oauth-callback`.
	- `scope`: Space-separated list of scopes to request.

This flow is also primarily intended for interactive use through the VS Code extension. As with the authorization code flow, if you provide `auth_token` as a runtime variable, rq will use that token directly and skip the interactive step, which is useful when you share the same authentication setup between VS Code (interactive) and CLI (non-interactive) use.

All auth types are validated at parse time: missing required fields, unexpected fields, or clearly invalid values (for example an empty bearer token) will cause rq to fail with a clear error before sending any request.

## Imports

As your rq files grow, it is often useful to split them into smaller, focused files: one with common variables and endpoints, others with domain-specific requests, and so on. The `import` statement lets you **reuse definitions across files** without copying them.

### Basic import

An import pulls in everything defined in another `.rq` file: requests, variables, environments, auth providers, and endpoints.

```rq
// base.rq
let user_id = 123;

auth my_auth(auth_type.bearer) {
    token: "{{api_token}}",
}
```

```rq
// main.rq
import "base";

let host = "http://localhost:8080";

[auth("my_auth")]
rq get("{{host}}/users/{{user_id}}");
```
In this example there are two files:

- `base.rq`, which defines shared pieces: the variable `user_id` and the auth provider `my_auth`.
- `main.rq`, which imports `base`, defines its own variable `host`, and declares a request that uses both `host` and the imported `user_id` and `my_auth`.

This makes it easy to keep shared configuration (hosts, common headers, auth providers, base endpoints, etc.) in one place, while keeping individual `.rq` files small and focused on a specific use case or API area.

### Import paths

The import path is resolved **relative to the current file**:

- You can import with an explicit extension: `import "shared.rq";`.
- Or rely on the implicit `.rq` extension: `import "shared";`.

In both cases rq will look for a file named `shared.rq` in the same directory as the importing file (or along the resolved relative path if you use subfolders).

Imports can be nested: if an imported file itself contains `import` statements, those files and their definitions will also be loaded and merged. Circular imports are not supported.

From the point of view of the language model, you can think of imports as **textually merging** the imported files into a single logical rq file before executing any requests, with duplicate definitions still subject to the usual validation rules (for example, duplicate request or auth names will cause errors).