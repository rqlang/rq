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
- `rq ep` lists and inspects endpoints.
- `rq var` lists and inspects variables.
- `rq check` validates `.rq` files without executing requests.

All subcommands accept a global `-d, --debug` flag to enable debug logging.

## Global usage

```bash
rq [OPTIONS] [COMMAND]
```

Commands:

- `env` – Manage environments.
- `auth` – Manage authentication.
- `request` – Manage requests.
- `ep` – Manage endpoints.
- `var` – Manage variables.
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

- In `text` mode, prints a human-readable list with entries like `name: basic`, `file: tests/request/run/input/basic.rq`.
- In `json` mode, prints a JSON array; each item contains at least `name` and `file`, and requests defined inside endpoints include endpoint context (for example `endpoint: api`, `name: api/get`).

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
- In `text` mode, prints fields like URL, method, headers, optional body, and associated auth provider.
- In `json` mode, prints a JSON object containing `Request`, `URL`, `Method`, `Headers`, optional `Body`, and optional `Auth` metadata.

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
- `-n, --name <NAME>` – Name of the request to run. If omitted and multiple requests exist, the CLI will usually fail and ask you to be explicit. If the request is defined inside an endpoint, use `<endpoint>/<request>` or `<endpoint>.<request>` (for example `users/list` or `users.list`).
- `-e, --env <ENVIRONMENT>` – Environment name.
- `-v, --variable <NAME=VALUE>` – Override variables at runtime (can be provided multiple times).
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- Uses the same variable precedence described in the language definition, with `-v NAME=VALUE` providing the highest-precedence overrides.
- In `text` mode, prints the HTTP status and a formatted view of the response.
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

## Managing environments: `rq env`

The `env` subcommand helps you discover available environments in your `.rq` files.

```bash
rq env [OPTIONS] <COMMAND>
```

Commands:

- `list` – List environments.
- `show` – Show environment details.

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
- In `json` mode, prints a JSON array of environment names.

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

### `rq env show`

Show details for a single environment.

```bash
rq env show [OPTIONS] --name <NAME>
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-n, --name <NAME>` – Name of the environment to show (required).
- `--no-var-interpolation` – Skip variable interpolation and show raw values.
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Examples:

```bash
rq env show -s tests/env/list/input/simple.rq -n local
rq env show -s tests/env/list/input/simple.rq -n local -o json
```

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

- In `text` mode, prints a list of auth provider names (for example `bearer_auth`, `github_oauth`).
- In `json` mode, prints a JSON array of provider names.
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
	- `Auth Configuration: bearer_auth`
	- `Type: bearer`
	- `token: ...`
- In `json` mode, prints an object with keys:
	- `Auth Configuration` – Provider name.
	- `Type` – Provider type (`bearer`, `oauth2_authorization_code`, etc.).
	- `Environment` – Optional, when `-e/--env` is provided.
	- `Fields` – Map of field names to values (for example `client_id`, `authorization_url`, `token_url`).

Examples:

```bash
rq auth show -s tests/request/run/input -n bearer_auth
rq auth show -s tests/request/run/input -n github_oauth -o json
rq auth show -s tests/request/run/input -n local_auth -e local
```

Error handling:

- If the named auth provider does not exist, the command fails with an error mentioning that the auth configuration was not found.

## Managing endpoints: `rq ep`

The `ep` subcommand lets you discover and inspect endpoints defined in `.rq` files.

```bash
rq ep [OPTIONS] <COMMAND>
```

Commands:

- `list` – List endpoints.
- `show` – Show endpoint details.
- `refs` – Find all references to an endpoint.

All `rq ep` commands accept `-d, --debug`.

### `rq ep list`

List all endpoints discovered under a file or directory.

```bash
rq ep list [OPTIONS]
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- In `text` mode, prints `Endpoints found:` followed by endpoint names, or `No endpoints found in .rq files`.
- In `json` mode, prints a JSON array of endpoint entries.

Examples:

```bash
rq ep list -s src/
rq ep list -s src/ -o json
```

### `rq ep show`

Show details for a single endpoint.

```bash
rq ep show [OPTIONS] --name <NAME>
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-n, --name <NAME>` – Name of the endpoint to show (required).
- `--no-var-interpolation` – Skip variable interpolation and show raw values.
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Examples:

```bash
rq ep show -n users
rq ep show -n users -o json
```

### `rq ep refs`

Find all references to an endpoint across `.rq` files.

```bash
rq ep refs [OPTIONS] --name <NAME>
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-n, --name <NAME>` – Name of the endpoint to find references for (required).
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- Prints `References found:` followed by locations, or `No references found`.

Examples:

```bash
rq ep refs -n users
rq ep refs -n users -o json
```

## Managing variables: `rq var`

The `var` subcommand lets you discover and inspect variables defined in `.rq` files.

```bash
rq var [OPTIONS] <COMMAND>
```

Commands:

- `list` – List variables.
- `show` – Show variable details.
- `refs` – Find all references to a variable.

All `rq var` commands accept `-d, --debug`.

### `rq var list`

List all variables discovered under a file or directory.

```bash
rq var list [OPTIONS]
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-e, --env <ENVIRONMENT>` – Environment name to filter environment-specific variables.
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- In `text` mode, prints `Variables found:` followed by variable names, or `No variables found in .rq files`.
- In `json` mode, prints a JSON array of variable entries.

Examples:

```bash
rq var list -s src/
rq var list -s src/ -e local -o json
```

### `rq var show`

Show details for a single variable.

```bash
rq var show [OPTIONS] --name <NAME>
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-n, --name <NAME>` – Name of the variable to show (required).
- `-e, --env <ENVIRONMENT>` – Environment name to resolve environment-specific values.
- `--no-var-interpolation` – Skip variable interpolation and show raw values.
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Examples:

```bash
rq var show -n base_url
rq var show -n base_url -e local -o json
```

### `rq var refs`

Find all references to a variable across `.rq` files.

```bash
rq var refs [OPTIONS] --name <NAME>
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-n, --name <NAME>` – Name of the variable to find references for (required).
- `-o, --output <OUTPUT>` – Output format: `text` or `json` (default: `text`).

Behavior:

- Prints `References found:` followed by locations, or `No references found`.

Examples:

```bash
rq var refs -n base_url
rq var refs -n base_url -o json
```

## Validating files: `rq check`

Parse and validate `.rq` files without executing any requests.

```bash
rq check [OPTIONS]
```

Options:

- `-s, --source <SOURCE>` – Path to the `.rq` file or directory (default: `.`).
- `-e, --env <ENVIRONMENT>` – Environment name to use for variable resolution.

Behavior:

- Always outputs JSON with a single `errors` array.
- Each error entry contains `file`, `line`, `column`, and `message`.
- If no errors are found, `errors` is an empty array.
- Exits with code `1` if any errors are found; exits with code `0` on success.

Example:

```bash
rq check -s src/
rq check -s src/api.rq -e local
```

Example output (no errors):

```json
{
  "errors": []
}
```

Example output (with errors):

```json
{
  "errors": [
    {
      "file": "src/api.rq",
      "line": 5,
      "column": 3,
      "message": "unexpected token"
    }
  ]
}
```

## Output formats

Across all commands, the `-o, --output` flag controls how results are printed (except `rq check`, which always outputs JSON):

- `text` – Human-readable, stable but meant for terminals.
- `json` – Machine-readable, designed for scripting and automated checks.

The value is case-insensitive, so `--output json` and `--output JSON` are equivalent. Invalid values cause a clear clap error indicating the allowed values.

When integrating rq into other tools or CI, prefer `--output json` so you can parse responses reliably.
