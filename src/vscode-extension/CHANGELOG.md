# Changelog

## [0.7.0]

### Enhancements

- **Idiom linting in the editor.** rq's style and idiom rules now run alongside the parser and appear in the Problems panel as warnings, tagged with the rule that produced them (`rq lint` source) and, where the fix is mechanical, a suggested rewrite. Rules are workspace-aware, so an endpoint duplicating a query string or auth provider declared in another `.rq` file is flagged even when that file is not open. Unsaved edits are linted as you type. Disable with the new `rq.lint.enabled` setting.
- **AI assistance via a bundled MCP server.** The extension now ships a Model Context Protocol server and registers it automatically, so Copilot Chat and other MCP-aware chats in VS Code can validate `.rq` source, run the idiom linter, and enumerate existing requests. It also publishes the language definition and idioms guide as resources and a `generate_rq` prompt that drives the full generate → validate → lint loop. The server runs on the editor's own Node.js and reuses the extension's WebAssembly build of rq — no native binary and no platform-specific download.
- New lint rules: `manual_auth_header` (a hand-written `Authorization: Bearer …` header that should be an `auth` provider), `hardcoded_secret` (a credential literal that belongs in `.env`), `duplicated_request_qs` and `query_param_as_header` (a query string repeated across sibling requests, or passed in the header position where rq silently sends it as a header), and `duplicated_ep_config` (a `qs` or `[auth(...)]` duplicated across endpoints extending the same template, or re-declared on a child that already inherits it).

### Performance

- Workspace scanning now skips `node_modules`, `.git`, and `target` directories. Previously every validation read every file under the workspace root, which was slow in repositories that keep dependencies alongside `.rq` files.

## [0.6.0]

### Bug Fixes

- Fixed duplicated `/` when joining an endpoint base URL with a request path — repeated slashes are now collapsed while the `://` scheme separator, query string, and fragment are left untouched.
- Fixed an unnecessary trailing `/` being added to resolved URLs when the base URL or a variable already ended with one.
- Fixed endpoint query strings being appended after the fragment instead of before it (`?a=1#frag` instead of `#frag?a=1`).
- Fixed request paths starting with `?` or `#` not being appended directly to the endpoint base URL.

### Enhancements

- Added a copy button next to the request URL in the response panel.
- Added a copy button on every request and response header row, copying the entry as `Name: value`.
- Added a copy-all button on the Request Headers and Response Headers section titles.
- Copy buttons now show a checkmark confirmation after copying, and the body copy button was restyled to match.
- Added an Advanced Example guide showing how to grow a single request into a full API suite, linked from the extension README.
- Rewrote the Getting Started guide.

## [0.5.0]

### Breaking Changes

- Headers are now defined using `$[ ... ]` instead of `[ ... ]`. Existing `.rq` files using the old syntax will need to be updated.

  Before:

  ```
  rq post_user(url: "...", headers: ["Content-Type": "application/json"]);
  ```

  After:

  ```
  rq post_user(url: "...", headers: $["Content-Type": "application/json"]);
  ```

### Bug Fixes

- Fixed variable autocomplete not suggesting matches correctly.
- Fixed autocomplete not working after text had already been typed at the cursor.
- Fixed autocomplete not suggesting properties for request `headers`.
- Fixed autocomplete not working for templated endpoints (`ep<base>(...)`).
- Fixed autocomplete failing on multiline `rq` and `ep` blocks.
- Fixed keyword autocompletion not appending a trailing space.
- Fixed functions (e.g. `time.now`, `io.read_file`) not being syntax-highlighted.
- Fixed empty JSON body (`${}`) failing when passed inline as an `rq` parameter instead of via a variable.
- Fixed request timeout configuration not being applied.
- Fixed incorrect version string sent in the `User-Agent` header.
- Fixed client certificate authentication failing on Windows (now uses PFX format).
- Fixed Run Again button not surfacing errors when the replayed request failed.
- Fixed the OAuth "getting token" status message not hiding after the command finished.
- Fixed typo in the "clear OAuth token" command.
- Fixed several issues in the format document command across `let`, `rq`, `ep`, and `auth` declarations.
- Fixed missing loading indicator in the configuration panel.
- Resolved warnings reported by CodeQL static analysis.

### Enhancements

- Implemented the named parameter attribute for `rq` and `ep` definitions.
- Added hover tooltips and autocomplete for built-in functions such as `time.now` and `io.read_file`.
- Auth failures now stop request execution and surface a clear error instead of continuing silently.
- Improved how logs and errors are reported in the Output window.
- Added debug logging to aid troubleshooting from the extension host.
- Allowed `"` and `` ` `` characters inside request body content.
- Added documentation for configuring OAuth with specific platforms.
- Upgraded `wasm-pack` to the latest version to remove build warnings.
- Updated the extension publish workflow to keep pace with the Node.js 24 runner migration on GitHub Actions.

## [0.4.0]

### Enhancements

- The extension no longer requires the `rq` CLI to be installed. All core functionality — request execution, syntax analysis, variable resolution, and auth handling — now runs entirely through a bundled WebAssembly module. The CLI remains supported as an optional tool but is no longer a prerequisite for the extension to work.

## [0.3.1]

### Bug Fixes

- Fixed syntax errors not being reported correctly in the VS Code Problems window.
- Fixed block comments (`/* */`) not being formatted properly.
- Fixed auth name not accepting direct variable references — only interpolation worked before.
- Fixed not all auth types being shown in autocomplete suggestions.
- Fixed named parameters being formatted incorrectly.
- Fixed empty array variables behaving differently than inline empty arrays in `rq` objects.
- Fixed missing error description when interpolation fails inside a JSON body.
- Fixed loading indicator running indefinitely in some cases.
- Fixed excessive error noise in the Output window from `var list` requests.

### Enhancements

- Implemented a language server that analyzes `.rq` files in real time.
- Added go-to-definition support for requests, variables, auth providers, and environments.
- Added rename symbol support across the file.
- Added find-all-references support.
- Added autocomplete for defined objects (variables, requests, environments, auth providers).
- Added autocomplete for `import` statements, discovering other `.rq` files in the workspace.
- Added autocomplete for object parameters (`rq`, `env`, `ep` fields).
- Added autocomplete for auth properties based on the selected `auth_type`.
- Added autocomplete for request attributes.
- Added hover tooltips for `ep`, `rq`, `env`, and `auth` statements.
- Added format document command for `.rq` files.
- Added `env` and `auth` entries to the RQ Explorer tree view.
- Added a copy button for response body in the extension result panel.
- Reviewed and updated code snippets.
- Updated CLI documentation for clarity and completeness.
- Updated UAT tests.
- Upgraded Rust and npm dependencies.
- Removed push trigger from BVT builds to reduce CI noise.
- Reviewed and resolved build warnings.

## [0.2.0]

### Bug Fixes

- Fixed incorrect variable substitution in the `url` property of endpoint definitions.
- Corrected the development version number calculation logic.
- Fixed issues in language definition sample files used for syntax highlighting and grammar.

### Enhancements

- Integrated CodeQL static analysis workflow for automated security scanning on every push.
- Auth name can now be left blank in request attributes to send anonymous requests per environment.
- Added validation to detect and report duplicate definitions of `rq`, `env`, `auth`, or endpoint identifiers.
- Published project documentation via GitHub Pages with custom DNS mapping.
- Updated and improved the VS Code extension README for clarity and completeness.
- Reviewed and addressed findings surfaced by the CodeQL security analysis.
- Endpoints can now reference requests by name using the `ep.rq` dot notation syntax.
- Added loading animations to explorer actions for immediate visual feedback when running requests.
