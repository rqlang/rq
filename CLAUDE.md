# RQ Project — Claude Instructions

## What is rq

`rq` is an open-source tool for managing and executing HTTP requests, composed of:
- **`cli/`** — Rust CLI binary (`rq`) — parser + HTTP client
- **`vscode-extension/`** — TypeScript VSCode extension
- **`tests/uat/`** — User acceptance tests
- **`docs/`** — Documentation
- **`deployment/`** — Release/deployment scripts

**Core invariant**: parsing (syntax layer) is strictly separate from execution (client layer). Parse once, reuse results. Never mix HTTP logic into parser code.

---

## Repository Structure

```
rq/
├── cli/                  # Rust crate — parser + HTTP client + CLI
│   ├── src/
│   │   ├── main.rs
│   │   ├── parser/       # Syntax layer (.rq format)
│   │   └── client/       # Execution layer (HTTP)
│   └── Cargo.toml
├── vscode-extension/     # TypeScript VSCode extension
│   ├── src/
│   └── package.json
├── tests/uat/            # UAT / integration tests
├── docs/                 # Documentation
├── deployment/           # Release scripts
└── CLAUDE.md
```

The VSCode extension communicates with the CLI as a subprocess — it does not re-implement parsing.

---

## Commands

### Rust CLI (`cli/`)

After **any** change to Rust code, run in this exact order:
```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test

# Run a specific test
cargo test <test_name>

# Build release binary
cargo build --release
```

### VSCode Extension (`vscode-extension/`)

```bash
npm install
npm run compile
npm run watch      # during development
npm run test
```

---

## Rust Code Style

- Public methods first in impl blocks: constructors → public → private
- **Do NOT add comments (`//`) or doc comments (`///`)** — names must be self-explanatory
- Methods ≤ ~50 lines; extract complex logic into well-named helpers
- Each method does one thing
- No `.unwrap()` in production code; use `?` operator
- Use `Result<T, Box<dyn std::error::Error>>` for public APIs with descriptive error messages

### Imports

- Group: `std` → external crates → internal modules
- Import types directly with `use`; avoid fully qualified paths in signatures

---

## TypeScript Code Style

- **Do NOT add comments, JSDoc, or docstrings**
- `camelCase` for functions/variables; `PascalCase` for classes, interfaces, types, enums
- `const` over `let`; never `var`
- `async/await` over raw Promises

---

## Testing

- The instance under test must be named `target`
- Each test covers exactly one behavior
- Test file mirrors source file location

### CLI Tests (`cli/tests/`)

```
cli/tests/
├── common/mod.rs          # Shared helpers: rq_cmd(), json_subset(), validate_json_response()
├── fixtures/              # Reusable certs, auth templates, shared .rq files
├── request/run/
│   ├── input/             # Input .rq files, organized by feature subdirectory
│   └── expected/          # Expected outputs (.json or .txt), mirroring input structure
├── auth/show/input/       # Input files for auth show command tests
├── env/list/{input,expected}/
├── request_run.rs         # Integration test harness (libtest-mimic, harness = false)
├── request_list.rs        # Unit tests for request list command
├── auth_show.rs           # Unit tests for auth show command
├── env_list.rs            # Unit tests for env list command
└── var_show.rs            # Unit tests for var show command
```

#### Integration tests — `request/run` directory pattern

Tests are auto-discovered from `tests/request/run/input/`. Each `.rq` file becomes one test. Expected output lives in `tests/request/run/expected/` at the same relative path.

**File naming conventions (encode test options in the filename):**

| Suffix | Meaning |
|---|---|
| `__code_N__` | Expect exit code N (default 0) |
| `__env_NAME__` | Use environment NAME |
| `__req_NAME__` | Run request NAME (use dot notation for endpoint requests: `ep_name.req_name`) |
| `__dir__` | Use directory source instead of single file |

Examples:
- `missing_var__code_2__.rq` → expects exit code 2, error text in `missing_var__code_2__.txt`
- `environments__env_local__.rq` → runs with env "local", validates `environments__env_local__.json`
- `endpoint_inheritance/simple__req_api_get__.rq` → runs request "api.get"

**Expected output files:**
- `.json` — JSON subset validation (supports `{{*}}` wildcard and `{{regex:...}}` patterns)
- `.txt` — Exact stderr/stdout text match (used for error cases)
- If both exist, `.json` takes precedence

**To add an integration test:**
1. Create `tests/request/run/input/{feature}/{name}{__options__}.rq`
2. Create `tests/request/run/expected/{feature}/{name}{__options__}.json` (or `.txt` for errors)
3. Run: `cargo test --test request_run`

#### Unit tests — per-command `.rs` files

```rust
mod common;
use common::rq_cmd;

#[test]
fn test_request_list_text() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args(["request", "list", "-s", "tests/request/run/input/basic.rq"])
        .output()?;
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)?.contains("expected_value"));
    Ok(())
}
```

- Use `rq_cmd()` from `common` — resolves to `target/debug/rq`
- Return `Result<(), Box<dyn std::error::Error>>`
- Name: `test_{command}_{scenario}_{format}`
- Cover both success and error paths; both text and JSON output formats

**To add a unit test:** add a `#[test]` function to the existing `tests/{command}.rs` file. Only create a new file if testing a brand new command.

#### Manual fixture tests (complex scenarios)

For tests that require environment variables, `.env` files, or special fixture directories, add manual tests directly in `request_run.rs` using `Command::new()`:

```rust
#[test]
fn test_request_secrets() {
    let output = Command::new(env!("CARGO_BIN_EXE_rq"))
        .args([...])
        .env("RQ__SECRET_VALUE", "from_env")
        .current_dir("tests/request/run/fixtures/secrets")
        .output()
        .unwrap();
    // assertions
}
```

#### Test helpers in `common/mod.rs`

| Helper | Use |
|---|---|
| `rq_cmd()` | `Command` pointing to `target/debug/rq` |
| `json_subset(expected, actual)` | Assert expected JSON is subset of actual; supports `{{*}}` and `{{regex:...}}` |
| `validate_json_response(stdout, path)` | Parse response envelope, validate body against `.json` file |
| `validate_pure_json_response(stdout, path)` | Validate raw JSON output against `.json` file |

#### Updating tests when changing CLI commands

When a CLI command changes (new flag, changed output, new error):
1. If output format changed → update matching `.json`/`.txt` expected files
2. If new flag or option added → add new input `.rq` + expected file pair
3. If new error condition → add `__code_N__` test with `.txt` expected
4. Always run the full suite: `cargo test --test request_run && cargo test`

---

## .rq Language Reference

This is the full syntax reference for `.rq` files. The parser in `cli/src/parser/` must handle all of these constructs.

### Identifiers

All identifiers (request names, variable names, env names, keys) use **snake_case**: lowercase, alphanumeric + underscores.

### Comments

```
// single-line comment
/* block comment
   multi-line */
```

### Basic request (`rq` statement)

```
rq request_name(url);
rq request_name(url, headers);
rq request_name(url, headers, body);
```

- Default method is **GET**.
- If the request name matches an HTTP verb (`get`, `post`, `put`, `delete`, `patch`, `head`, `options`), that method is used.
- Named parameters are supported: `url:`, `headers:`, `body:`.
- Positional and named can be mixed (URL positional + named headers is common).

```
// Positional
rq get("http://localhost:8080/users");

// With headers
rq get("http://localhost:8080/users", [
  "Accept": "application/json",
]);

// With JSON body (positional)
rq post(
  "http://localhost:8080/users",
  ["Content-Type": "application/json"],
  ${"name": "Alice", "age": 30}
);

// Named parameters
rq create_user(
  url: "http://localhost:8080/users",
  headers: ["X-App": "rq"],
  body: ${"name": "Alice"}
);

// Mixed
rq get_user("http://localhost:8080/users", headers: ["Accept": "application/json"]);
```

### Headers

Square-bracket map of string key → string value:
```
["Header-Name": "value", "Other": "value2"]
```

### Body types

- **JSON**: `${"key": "value", "num": 123}` — automatically adds `Accept: application/json` if not present.
- **String**: `"plain text body"` — sent as-is.

### Variables (`let`)

```
let base_url = "http://localhost:8080";
let headers = ["Accept": "application/json"];
let payload = ${"name": "Alice"};

rq get("{{base_url}}/users", headers);
```

Interpolation syntax: `{{variable_name}}` inside strings.

**Precedence (highest → lowest):**
`execution-time variables` > `secrets` > `environment` > `file let`

Within a layer, last definition wins. Missing variables are errors (no silent fallback).

### Attributes

Placed immediately above an `rq` statement:

```
[method(POST)]
rq create("http://localhost:8080/users");

[timeout(10)]
rq slow_endpoint("http://localhost:8080/heavy");

[auth("my_auth")]
rq protected("http://localhost:8080/secure");

// Multiple attributes
[method(PUT)]
[timeout(5)]
[auth("token")]
rq update("http://localhost:8080/users/1");
```

Supported methods: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`.

### Environments (`env`)

```
env local {
  base_url: "http://localhost:8080",
  api_key: "dev-key",
}

env production {
  base_url: "https://api.example.com",
  api_key: "{{prod_key}}",
}

rq get("{{base_url}}/users");
```

Multiple environments per file. Active environment selected at runtime (CLI flag or VSCode UI). Missing environment when a variable is only in `env` → error.

### Secrets

Loaded from `.env` file next to the `.rq` file, or OS environment variables.

`.env` file format:
```bash
API_KEY=secret-123
ENV__LOCAL__API_KEY=local-secret   # only for 'local' environment
```

OS environment variables:
```bash
RQ__API_KEY=value                  # all environments
RQ__ENV__LOCAL__API_KEY=value      # only 'local' environment
```

Variable names normalized to snake_case internally.

### Built-in functions

```
let id  = random.guid();            // UUID v4
let ts  = datetime.now();           // ISO timestamp: 2024-03-05T12:34:56.789+0100
let day = datetime.now("yyyy-MM-dd");
let raw = io.read_file("data.txt"); // path relative to .rq file
```

### Endpoints (`ep`)

Group related requests under a base URL with shared headers/query string:

```
let user_id = 123;

ep users("http://localhost:8080/api/users", ["X-App": "rq"], "v=1") {
  rq list();                    // GET /api/users?v=1
  rq get("/{{user_id}}");       // GET /api/users/123?v=1
  [method(POST)]
  rq create(body: ${"name": "Alice"});
}

// Named parameters
ep api(url: "http://localhost:8080/api", headers: ["X-App": "rq"], qs: "v=1") {
  rq list("/users");
}
```

Child requests inherit base URL, headers, and query string. Child attributes override endpoint-level attributes.

### Endpoint templates (inheritance)

```
ep base(url: "http://localhost:8080", headers: ["X-Base": "1"], qs: "v=1");

ep users<base>("/users") {
  rq list();
  rq get("/{{user_id}}");
}

ep widgets<base>("/widgets") {
  rq list();
}
```

### Auth providers

```
auth my_bearer(auth_type.bearer) {
  token: "{{api_token}}",
}

auth my_oauth(auth_type.oauth2_client_credentials) {
  client_id: "{{client_id}}",
  client_secret: "{{client_secret}}",
  token_url: "{{token_url}}",
  scope: "read write",              // optional
}

auth my_pkce(auth_type.oauth2_authorization_code) {
  client_id: "{{client_id}}",
  authorization_url: "{{auth_url}}",
  token_url: "{{token_url}}",
  scope: "read",                    // optional
  code_challenge_method: "S256",    // optional, default S256
}

auth implicit_flow(auth_type.oauth2_implicit) {
  client_id: "{{client_id}}",
  authorization_url: "{{auth_url}}",
}

[auth("my_bearer")]
rq protected("http://localhost:8080/secure");

// Conditional auth — empty string disables auth
let auth_provider = "";
[auth("{{auth_provider}}")]
rq maybe_auth("http://localhost:8080/public");
```

Auth types: `auth_type.bearer`, `auth_type.oauth2_client_credentials`, `auth_type.oauth2_authorization_code`, `auth_type.oauth2_implicit`.

All fields validated at parse time — missing required fields fail before any request is sent.

### Imports

```
import "shared";        // resolves to shared.rq in same directory
import "shared.rq";     // explicit extension
```

Imports are transitive. Circular imports are not supported. Duplicate request/auth names across imported files are errors.

---

## Working on a Task

1. Identify which component is affected: `cli/` and/or `vscode-extension/`
2. Identify the specific files to modify before writing any code
3. For `cli/` changes: run `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test`
4. For `vscode-extension/` changes: run `npm run compile && npm run test`

---

## What NOT to Do

- Do not add comments to code
- Do not use `.unwrap()` in Rust production paths
- Do not mix parsing and execution logic
- Do not modify `deployment/` unless the task explicitly requires it
- Do not create new files without checking if the logic fits in an existing module first
