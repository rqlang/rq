---
layout: default
title: CLI
nav_order: 6
---

# CLI

The `rq` CLI is the command-line interface for working with `.rq` files. It lets you discover and run requests, inspect environments and authentication configurations, and is the main way to integrate rq into scripts and CI.

If you haven't installed rq yet, see the [INSTALLATION guide](INSTALLATION.md) first.

At a high level:

- `rq` without subcommands runs a request (`request run`).
- `rq request` manages requests (list, show, run).
- `rq env` lists and inspects environments found in `.rq` files.
- `rq auth` lists and inspects auth providers.
- `rq check` validates `.rq` files without executing requests.

All subcommands accept a global `-d, --debug` flag that writes a diagnostic trace to stderr (see [Debug logging](#debug-logging)).

### Debug logging

Every command accepts `-d, --debug`. It writes a trace to stderr, in the style of `curl -v`, while stdout keeps the normal output. Each line starts with the time elapsed since rq started:

```text
[     5ms] * rq 0.7.0 (macos aarch64)
[     6ms] * Command: rq request run -s api -e local -v token=***
[     6ms] * Working directory: /Users/ana/project
[     6ms] * Source: api (directory)
[     7ms] * Found 3 .rq file(s) in api
[    20ms] * Parsed /Users/ana/project/api/users.rq (imports: /Users/ana/project/api/_shared.rq)
[    21ms] * Environment 'local' for /Users/ana/project/api/users.rq: base_url, api_key
[    21ms] * Secrets from api/.env: api_key
[    21ms] * Secrets from RQ__ environment variables: none
[    22ms] * Running 1 request(s) from /Users/ana/project/api/users.rq
[    22ms] * Variable base_url from env:local
[    22ms] * Variable token from cli
[    23ms] * Applying auth 'tok' (bearer)
[    23ms] > POST http://localhost:8080/users
[    23ms] > authorization: ***
[    29ms] < 201 Created (6 ms)
[    29ms] < content-type: application/json
[    30ms] * Finished with exit code 0
```

- Lines starting with `*` describe what rq is doing: which files it found and parsed, which environment and secret sources it used, where each variable used by a request comes from (`cli`, `request`, `endpoint`, `secret`, `env:<name>` or `let`), OAuth2 token requests, and the `check` summary.
- Lines starting with `>` show the request exactly as it is sent, after variables and auth are resolved, and lines starting with `<` show the response status and headers.
- The last line reports the exit code and, when the command fails, the error.

Secret values never appear in the trace. Secrets and variables are listed by name, `-v` values in the command line are shown as `NAME=***`, header values whose name contains `authorization`, `cookie`, `token`, `secret`, `key` or `password` are masked, and so are body fields whose key contains a secret marker.

### Reporting a bug

Run the failing command again with `-d` and save the trace to a file:

```bash
rq request run -s api -e local -d 2> rq-debug.log
```

Attach `rq-debug.log` to the issue. It already includes the rq version, the platform and the exact command. Secret values are masked, but review the file before sharing it, since URLs, file paths and non-secret values are kept as they are.

## Global usage

```bash
rq [OPTIONS] [COMMAND]
```

Commands:

- `env` – Manage environments.
- `auth` – Manage authentication.
- `request` – Manage requests.
- `check` – Validate `.rq` files.

If you call `rq` without a subcommand, it behaves like `rq request run` with the same arguments.

Global options:

- `-d, --debug` – Enable debug logging.
- `-V, --version` – Print CLI version.
- `-h, --help` – Show help.

Unless otherwise noted, most commands share these common flags:

- `-s, --source <SOURCE>` – Path to a `.rq` file or directory (defaults to current directory).
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (defaults to `text`, case-insensitive).

## Managing requests: `rq request`

The `request` subcommand lets you list, inspect, and run requests defined in `.rq` files.

```bash
rq request [OPTIONS] <COMMAND>
```

Commands:

- `list` – List requests.
- `show` – Show request details.
- `run` – Run a request.

### `rq request list`

List all requests discovered under a file or directory.

```bash
rq request list [OPTIONS]
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- In `text` mode, prints `Requests found:` followed by request names (for example `- basic`, `- users/list`), or `No requests found`.
- In `json` mode, prints a JSON array of objects with a single `name` field (for example `{"name": "users/list"}`). Use `rq request show` for the details of a request.

Example:

```bash
rq request list -s tests/request/run/input
rq request list -s tests/request/run/input -o json
```

### `rq request show`

Show detailed information about a single request.

```bash
rq request show [OPTIONS]
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-n, --name <NAME>` – Name of the request to show (required). If the request is defined inside an endpoint, use `<endpoint>/<request>` or `<endpoint>.<request>` (for example `users/list` or `users.list`).
- `-e, --env <ENVIRONMENT>` – Environment name to resolve variables and env-specific settings.
- `--no-var-interpolation` – Skip variable interpolation and show raw values.
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- Resolves the specified request (including endpoint context if applicable).
- In `text` mode, prints `name`, `method`, `url`, `headers`, the optional `body`, `timeout` and `auth` (as `name (type)`), and `location` as `file:line:column`.
- In `json` mode, prints the same fields as an object: `name`, `method`, `url`, `headers`, optional `body`, `timeout` and `auth` (`{"name", "type"}`), plus `file`, `line` and `column`.

Example:

```bash
rq request show -s tests/request/run/input -n basic
rq request show -s tests/request/run/input -n basic -e local -o json
```

### `rq request run`

Run one or more requests from `.rq` files.

```bash
rq request run [OPTIONS]
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-n, --name <NAME>` – Name of the request to run. If omitted, every request in the source runs. If the request is defined inside an endpoint, use `<endpoint>/<request>` or `<endpoint>.<request>` (for example `users/list` or `users.list`).
- `-e, --env <ENVIRONMENT>` – Environment name.
- `-v, --variable <NAME=VALUE>` – Override variables at runtime (can be provided multiple times).
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- Uses the same variable precedence described in the language definition, with `-v NAME=VALUE` providing the highest-precedence overrides.
- In `text` mode, prints one block per request: a line with the request name, method and URL, a status line with the reason phrase and elapsed time, and the response body. JSON bodies are indented without reordering their keys. Response headers are shown only with `-d`:

  ```text
  basic  GET http://localhost:8080/get
  200 OK · 5 ms

  {
    "status": "ok"
  }
  ```
- In `json` mode, prints a JSON structure with the full execution result(s), including response status, headers, body, and elapsed time in milliseconds.

Examples:

```bash
# Run a single request in a file
rq request run -s tests/request/run/input/basic.rq -n basic

# Run using an environment and a CLI variable override
rq request run -s tests/request/run/fixtures/cli_override/override.rq -e local -v color=red

# Default invocation (same as `rq request run`)
rq -s tests/request/run/input/basic.rq -n basic
```

Error handling:

- If `--source` points to a non-existent path, the command exits with code `2` and prints `Path does not exist`.
- If a variable override does not follow `NAME=VALUE`, or the variable name is invalid, the command fails with clear validation messages.

#### Required variables

If a request declares one or more `[required(var_name)]` attributes (see [Language Definition — `required` attribute](LANGUAGE_DEFINITION.md#required-attribute)), the CLI validates that every required variable has been supplied at runtime via `-v` before sending the request. `let` bindings, environment blocks, and secrets do not satisfy a `required` declaration.

If one or more required variables are missing, the CLI exits with code `3` and prints:

```
Error: Validation error: Required variable(s) not set: var_name
```

To supply the missing values, pass them with `-v`:

```bash
rq request run -s api.rq -n users/create -v user_name=Alice -v user_role=admin
```

## Managing environments: `rq env`

The `env` subcommand helps you discover available environments in your `.rq` files.

```bash
rq env [OPTIONS] <COMMAND>
```

Commands:

- `list` – List environments.

### `rq env list`

List environment names defined across `.rq` files.

```bash
rq env list [OPTIONS]
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- Recursively scans the given path for `.rq` files and collects all environment names (from `env <name> { ... }` blocks).
- In `text` mode, prints a short list prefixed with `Environments found:` or a message like `No environments found` for empty results.
- In `json` mode, prints a JSON array of objects with a single `name` field.

Examples:

```bash
rq env list -s tests/env/list/input/simple.rq
rq env list -s tests/env/list/input -o json

# Using the current directory as source
cd tests/request/run/input
rq env list
```

Error handling:

- A non-existent `--source` path causes the command to exit with code `2` and an error mentioning `Path does not exist`.

## Managing auth providers: `rq auth`

The `auth` subcommand lets you list and inspect authentication configurations declared in your `.rq` files.

```bash
rq auth [OPTIONS] <COMMAND>
```

Commands:

- `list` – List auth configurations.
- `show` – Show details for a specific auth configuration.

All `rq auth` commands accept `-d, --debug`.

### `rq auth list`

List all auth providers defined across `.rq` files.

```bash
rq auth list [OPTIONS]
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- In `text` mode, prints `Auth configurations found:` followed by provider names (for example `- bearer_auth`), or `No auth configurations found`.
- In `json` mode, prints a JSON array of objects with a single `name` field. Use `rq auth show` for the type and fields of a provider.
- For empty directories, prints `No auth configurations found`.

Examples:

```bash
rq auth list -s tests/request/run/input
rq auth list -s tests/request/run/input -o json
```

### `rq auth show`

Show the full configuration of a single auth provider.

```bash
rq auth show [OPTIONS] --name <NAME>
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-n, --name <NAME>` – Name of the auth configuration (required).
- `-e, --env <ENVIRONMENT>` – Environment name to resolve environment-specific overrides for that auth provider.
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- Resolves the auth provider (e.g. `bearer_auth`, `github_oauth`) and shows its type and fields.
- In `text` mode, prints a human-readable summary like:
	- `name: bearer_auth`
	- `type: bearer`
	- `token: ...`
- In `json` mode, prints an object with keys:
	- `name` – Provider name.
	- `type` – Provider type (`bearer`, `oauth2_authorization_code`, etc.).
	- `environment` – Optional, when `-e/--env` is provided.
	- `fields` – Map of field names to values (for example `client_id`, `authorization_url`, `token_url`).
	- `file`, `line`, `column` – Where the provider is declared.

Examples:

```bash
rq auth show -s tests/request/run/input -n bearer_auth
rq auth show -s tests/request/run/input -n github_oauth -o json
rq auth show -s tests/request/run/input -n local_auth -e local
```

Error handling:

- If the named auth provider does not exist, the command fails with an error mentioning that the auth configuration was not found.

## Validating files: `rq check`

Parse and validate `.rq` files without executing any requests, and run the [lint rules](LINT_RULES.md) over them. Errors fail the check; lint findings are reported as warnings.

```bash
rq check [OPTIONS]
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-e, --env <ENVIRONMENT>` – Environment name to use for variable resolution.
- `-o, --output <OUTPUT>` – Output format: `text` (default) or `json`.
- `--deny-warnings` – Exit with code `1` when lint reports any warning.

Behavior:

- In `text` mode, prints one line per error as `file:line:column: message`, then one line per warning as `file:line:column: warning[rule]: message` followed by an indented `help:` line when the rule suggests a fix. It ends with the error and warning counts, or `No errors found`.
- In `json` mode, outputs an `errors` array and a `warnings` array. Each error contains `message` and, when known, `file`, `line` and `column`. Each warning contains `file`, `line`, `column`, `rule`, `message` and, when the rule has one, `suggested_fix`.
- Errors that are not tied to a position (for example, a file that cannot be read) are reported with their message only.
- Files that fail to parse are not linted; fix their errors first.
- Cross-file rules see every `.rq` file under the source directory, or under the file's own directory when `--source` is a file.
- Exits with code `1` if any errors are found, or if any warnings are found with `--deny-warnings`; exits with code `0` otherwise.

Example:

```bash
rq check -s src/
rq check -s src/api.rq -e local
rq check -s src/ -o json
rq check -s src/ --deny-warnings
```

Example output (text):

```text
src/api.rq:5:3: unexpected token
src/users.rq:2:13: warning[empty_url_string]: Request `list` passes an empty URL string. ...
  help: Replace `rq list("")` with `rq list()`.

1 error, 1 warning found
```

Example output (json, no findings):

```json
{
  "errors": [],
  "warnings": []
}
```

Example output (json, with errors and warnings):

```json
{
  "errors": [
    {
      "file": "src/api.rq",
      "line": 5,
      "column": 3,
      "message": "unexpected token"
    }
  ],
  "warnings": [
    {
      "file": "src/users.rq",
      "line": 2,
      "column": 13,
      "rule": "empty_url_string",
      "message": "Request `list` passes an empty URL string. ...",
      "suggested_fix": "Replace `rq list(\"\")` with `rq list()`."
    }
  ]
}
```

## Output formats

Across all commands, the `-o, --output` flag controls how results are printed:

- `text` – Human-readable, stable but meant for terminals.
- `json` – Machine-readable, designed for scripting and automated checks.

The value is case-insensitive, so `--output json` and `--output JSON` are equivalent. Invalid values cause a clear clap error indicating the allowed values.

When integrating rq into other tools or CI, prefer `--output json` so you can parse responses reliably.

### JSON conventions

Every command follows the same rules in `json` mode:

- Keys are `snake_case`.
- `list` commands return only names, as an array of `{"name": ...}` objects; `show` commands return the full detail of one item. Both output modes carry the same content.
- Source locations are reported as `file`, `line` and `column`. `file` is an absolute path, and `line` and `column` start at 1, so `file:line:column` points at the same place an editor shows.
- Optional fields are omitted when they have no value, instead of being printed as `null`.
- Results go to stdout and always end with a newline.
- Errors go to stderr as `{"error": {"type": ..., "message": ..., "file": ..., "line": ..., "column": ...}}`, where the location fields are present only when known.
- Warnings that do not stop the command (for example, a file that fails to parse while listing a directory) go to stderr with the same shape under a `warning` key.
