# RQ Project — Claude Instructions

## What is rq

`rq` is an open-source tool for managing and executing HTTP requests.

**Core invariant**: syntax layer is strictly separate from execution layer. Parse once, reuse results. Never mix HTTP logic into parser code.

---

## Repository Structure

```
rq/
├── Cargo.toml              # Workspace: members = ["src/cli", "src/rq-lib"]
├── src/
│   ├── cli/                # Thin CLI binary — commands only, no business logic
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   └── commands/   # auth, check, env, ep, request, shared, var
│   │   └── tests/          # Integration + unit tests
│   ├── rq-lib/             # Core library — all parsing + HTTP execution
│   │   └── src/
│   │       ├── syntax/     # Syntax layer: tokenizer, parser, AST
│   │       ├── client/     # HTTP execution layer
│   │       ├── auth/       # Auth providers (bearer, oauth2, etc.)
│   │       └── native/     # Platform-specific implementations
│   └── rq-wasm/            # WASM build of rq-lib (not in workspace)
├── src/vscode-extension/   # TypeScript VSCode extension
├── tests/uat/              # UAT / integration tests
├── docs/
└── deployment/
```

The VSCode extension talks to `rq-wasm` in-process (worker), not a CLI subprocess — it does not re-implement parsing.

---

## Where things live

**Core library — `src/rq-lib/src/`**
- `syntax/` — tokenizer, parser, AST, semantic analysis
  - `tokenizer.rs`, `token.rs`, `keywords.rs`, `reader.rs` — lexing
  - `parsers/` — per-construct: `request.rs`, `endpoint.rs`, `environment.rs`, `variable.rs`, `import.rs`, `attributes.rs`, `auth.rs`
  - `rq_file.rs` — top-level parsed-file model
  - `resolve.rs` — cross-file symbol/reference resolution
  - `analysis.rs` — semantic validation (powers `check`)
  - `variable_context.rs` — variable precedence/scoping
  - `auth/` — auth config AST (bearer, oauth2 variants)
  - `functions/` — built-ins, grouped by category (`datetime/`, `io/`, `random/`, `sys/`)
  - `secrets.rs`, `fs.rs` — syntax-side hooks into platform traits
- `client/` — HTTP execution (`mod.rs`, `models.rs`)
- `auth/` — runtime auth providers: `bearer.rs`, `oauth2_*.rs`, `auth_provider.rs`
- `native/` — native (non-WASM) impls of `fs`, `http`, `secrets`
- `http.rs`, `paths.rs`, `error.rs`, `logger.rs`, `version.rs`, `lib.rs`

**CLI — `src/cli/src/`**
- `main.rs` — entry point, arg dispatch
- `commands/` — one file per top-level command: `auth.rs`, `check.rs`, `env.rs`, `ep.rs`, `request.rs`, `var.rs`, plus `shared.rs`, `validators.rs`
- `core/` — CLI-only helpers: `formatter.rs`, `exit_code.rs`, `error.rs`, `logger.rs`, `version.rs`

**WASM bindings — `src/rq-wasm/src/`**
- `bindings.rs` — JS-facing API surface (add here when exposing new `rq-lib` calls)
- `fs.rs`, `http.rs`, `secrets.rs` — WASM impls of `native/` traits
- `wasm/` — built artifacts; do not edit

**VSCode extension — `src/vscode-extension/src/`**
- `extension.ts` — activation
- `wasmHost.ts`, `wasmWorker.ts` — runs `rq-wasm` in a worker
- `rqClient.ts` — typed wrapper over `wasmCall`; all extension code talks to rq through here
- `commands/` — one file per palette command (`runRequest.ts`, `selectEnvironment.ts`, `openEndpoint.ts`, …)
- `language/` — LSP-style providers: `completionProvider.ts`, `diagnosticsProvider.ts`, `hoverProvider.ts`, `definitionProvider.ts`, `referenceProvider.ts`, `renameProvider.ts`, `formattingProvider.ts`, `signatureHelpProvider.ts`
- `auth/` — OAuth flows in TS (PKCE, implicit, refresh, callback handlers)
- `requestExplorer.ts`, `configurationExplorer.ts` — tree views
- `ui/webviewGenerator.ts` — webview HTML

## Where to add new features

| Change | Where |
|---|---|
| New syntax / keyword | `rq-lib/src/syntax/keywords.rs` + matching `parsers/*.rs` + AST node |
| New semantic check / diagnostic | `rq-lib/src/syntax/analysis.rs` |
| New built-in function | `rq-lib/src/syntax/functions/<category>/` + register in `functions/mod.rs` |
| New auth provider | AST in `syntax/auth/`, runtime in `auth/` |
| New CLI command | `cli/src/commands/<name>.rs` + dispatch in `main.rs` |
| New JS-callable API | `rq-wasm/src/bindings.rs` + matching method in `vscode-extension/src/rqClient.ts` |
| New VSCode command | `vscode-extension/src/commands/<name>.ts` + register in `extension.ts` + `package.json` |
| New LSP feature | `vscode-extension/src/language/<feature>Provider.ts` |

---

## Commands

All Rust commands run from the **workspace root** (`/Users/javito/_code/rq`).

After **any** Rust change, run in this order:
```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Specific test runs:
```bash
cargo test --test request_run        # integration tests
cargo test <test_name>               # single test
cargo build --release
```

VSCode extension (`src/vscode-extension/`):
```bash
npm install && npm run compile
npm run watch   # during development
npm run test
```

---

## Rust Code Style

- Method ordering within `impl` blocks: constructors → public methods (grouped) → private methods (grouped). Private helpers always go at the bottom, after all public methods.
- **No comments (`//`, `///`)** — names must be self-explanatory
- Methods ≤ ~50 lines; extract helpers for complex logic
- No `.unwrap()` in production; use `?`
- Public APIs: `Result<T, Box<dyn std::error::Error>>` with descriptive messages
- Imports: `std` → external crates → internal modules; import types directly

---

## TypeScript Code Style

- **No comments, JSDoc, or docstrings**
- `camelCase` functions/variables; `PascalCase` classes/interfaces/types/enums
- `const` over `let`; never `var`; `async/await` over Promises

---

## Testing

- Test instance named `target`
- Each test covers exactly one behavior

### Test layout (`src/cli/tests/`)

```
tests/
├── common/mod.rs           # rq_cmd(), json_subset(), validate_json_response()
├── fixtures/               # Certs, auth templates, shared .rq files
├── request/run/
│   ├── input/              # .rq input files (auto-discovered)
│   └── expected/           # .json or .txt expected output (mirrors input structure)
├── request_run.rs          # Integration harness (libtest-mimic, harness=false)
├── request_list.rs         # request list command
├── request_show.rs
├── auth_show.rs / auth_list.rs / auth_integration_providers.rs
├── env_list.rs / ep_list.rs / ep_show.rs / ep_refs.rs
├── var_show.rs / var_list.rs / var_refs.rs
├── check.rs
└── help.rs
```

### Check tests — `check/run` pattern

Auto-discovered from `tests/check/input/`. Each `.rq` → one test. Expected output in `tests/check/expected/` at the same relative path. Files with no matching expected `.json` are treated as fixtures/companion imports and skipped.

**Filename suffixes:**

| Suffix | Meaning |
|---|---|
| `__env_NAME__` | Pass `-e NAME` to the check command |
| `__dir__` | Run check on the parent directory (file is an empty sentinel) |

**Expected output:**
- Always `.json` — subset-checked via `json_subset` (`{{*}}` wildcard, `{{regex:...}}`)
- Exit code inferred from expected: empty `errors` → exit 0, non-empty → exit 1

**To add a check test:**
1. `tests/check/input/{name}{__opts__}.rq`
2. `tests/check/expected/{name}{__opts__}.json`
3. `cargo test --test check_run`

**For directory-level check tests:**
1. Add empty sentinel `tests/check/input/{dir}/__dir__.rq`
2. Add `tests/check/expected/{dir}/__dir__.json`

**Two tests stay manual in `check_run.rs`** (can't be expressed as files):
- `check_nonexistent_source` — non-existent path
- `check_endpoint_shared_url_var_deduped` — inline temp file

---

### Integration tests — `request/run` pattern

Auto-discovered from `tests/request/run/input/`. Each `.rq` → one test. Expected output in `tests/request/run/expected/` at the same relative path.

**Filename suffixes:**

| Suffix | Meaning |
|---|---|
| `__code_N__` | Expect exit code N (default 0) |
| `__env_NAME__` | Use environment NAME |
| `__req_NAME__` | Run request NAME (dot notation for ep requests: `ep.req`) |
| `__dir__` | Use directory source |

**Expected output:**
- `.json` — JSON subset validation (`{{*}}` wildcard, `{{regex:...}}`)
- `.txt` — Exact stderr/stdout match (error cases)
- Both present → `.json` takes precedence

**To add an integration test:**
1. `tests/request/run/input/{feature}/{name}{__opts__}.rq`
2. `tests/request/run/expected/{feature}/{name}{__opts__}.json` (or `.txt`)
3. `cargo test --test request_run`

### Unit tests

Add `#[test]` to the existing `tests/{command}.rs`. Create a new file only for a brand-new command.

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

### Manual fixture tests (env vars / special dirs)

Add directly to `request_run.rs` using `Command::new(env!("CARGO_BIN_EXE_rq"))`.

### Test helpers (`common/mod.rs`)

| Helper | Use |
|---|---|
| `rq_cmd()` | `Command` → binary via `env!("CARGO_BIN_EXE_rq")` |
| `json_subset(expected, actual)` | Subset assertion with `{{*}}` / `{{regex:...}}` |
| `validate_json_response(stdout, path)` | Parse response envelope, validate body against `.json` |
| `validate_pure_json_response(stdout, path)` | Raw JSON output vs `.json` file |

### When CLI output changes

1. Output format changed → update matching `.json`/`.txt` expected files
2. New flag → add input `.rq` + expected file pair
3. New error → add `__code_N__` test with `.txt` expected
4. Always run: `cargo test --test request_run && cargo test`

---

## Working on a Task

1. Identify affected crate(s): `src/rq-lib/` (core logic), `src/cli/` (commands/CLI), `src/vscode-extension/`
2. Read the specific files before modifying
3. **Always add tests** for every change — no exceptions. For new syntax errors or validation, add input/expected files following the check pattern (see below). For TypeScript logic, add a test in the matching `test/language/*.test.ts` file.
4. Run `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test`

---

## What NOT to Do

- No comments in code
- No `.unwrap()` in production Rust
- No mixing of syntax/parsing logic with HTTP/execution logic
- Do not modify `deployment/` unless explicitly required
- Do not create new files without checking if logic fits in an existing module
