# Changelog

## [Unreleased]

### Features

- Added the `rq.results.tabMode` setting to choose where request results open: one shared tab (`single`, the default and the previous behavior), one tab per request (`perRequest`), or a new tab for every run (`perExecution`). **Run again** and **Copy** in a results tab now act on that tab's own request and body.

### Bug Fixes

- Fixed *Run Request* sending a different request than `rq request run` for the same `.rq` file. The extension now runs every request through the same preparation as the CLI and only performs the network call itself, so these divergences are gone:
  - A `let` that overrides an imported variable was ignored and the imported value was sent; the same happened in *Show Request* and in variable hovers.
  - A request imported by several files could be resolved with another file's variables. It is now resolved in the file it is listed under in the request explorer, as `rq request run -s <that file>` does, while the `.env` keeps coming from the opened folder.
  - Auth fields were resolved without the request's variables (`[required]` prompts, *Run Request with Variables*, endpoint scope), and an auth name declared in two folders could resolve to the other folder's provider.
  - A string body that looked like JSON, such as `io.read_file("item.json")`, was sent with `Content-Type: application/json`; it is now sent without one, as the CLI does. Use `io.read_json` for a JSON body.
  - A header repeated in a request was sent only once.
- Fixed a runtime `auth_token` losing to an `auth_token` defined with `let` in the file.
- Fixed a `timeout` too large to represent crashing `rq request run` and the extension. It is now reported as `Timeout value '…' is too large`, and a timeout longer than about 24.8 days no longer fires immediately in the extension.

## [0.8.1]

### Breaking Changes

- Values are now type checked, so files that used to fail only when the request was sent are flagged in the Problems panel: a headers map or JSON value used as a url, a string used as headers, a JSON variable used as a header value, or a template endpoint parameter bound to the wrong type. Arrays (`let a = ["x"];`) are rejected at the declaration — a list of header entries is written `$[...]`.
- An empty or whitespace-only `url` on `rq` or `ep` is now an error (`Parameter 'url' cannot be empty`). It stays valid where the URL is inherited: a `rq` inside an `ep`, or an `ep` extending a template.

### Bug Fixes

- Fixed the formatter changing request data: re-indenting a document also indented the continuation lines of multi-line string literals, so saving a `.rq` file changed the headers and bodies actually sent. Literals are now restored verbatim, line merging no longer crosses an unterminated string, and spacing rules no longer edit comment text (`// TODO:fix` stayed `// TODO: fix`).
- Fixed formatting a multiline call collapsing a multiline `$[...]` or `${...}` argument onto one line. Multiline arrays and JSON bodies now keep one entry per line, recursively; repeated spaces outside strings and comments collapse to one; a nested `},` no longer splits; and a block comment opened mid-line is left alone.
- Fixed `[method(...)]` offering no completions once its snippet had been exited. The HTTP verbs are now suggested whenever the cursor is inside the value, including in multi-line attributes.
- Fixed a headers map missing its closing `]` being reported as `Expected ')'` at the end of the file. It is now reported at the offending token as `Expected string key or ']'`, or as `Expected ']'` at the end of input, in `rq`, `ep` and `let`.
- Fixed snippets inserting sample values (`http://localhost:8080/...`, `https://auth.example.com/...`, `openid profile`, `./client-cert.p12`) that stayed in the file when tabbed past and could silently reach whatever listens on that port. They now insert empty placeholders, and the completion documentation shows an **Example:** with the samples filled in. `redirect_uri` keeps `http://localhost:8080/callback`, since its scheme selects the callback mode.
- Fixed a JSON variable used as a header value being serialised into the header and sent.
- Fixed a template endpoint with a wrong-typed parameter passing validation and failing at send time. A child endpoint inherits the bindings of the template it extends, so an inherited parameter is reported where it was declared.
- Fixed `oauth2_implicit` accepting empty required fields such as `authorization_url`; it now reports them like the other OAuth2 providers.
- Fixed the debug logging setting being lost when the WebAssembly worker restarted.

### Enhancements

- **Cancel a running request.** A stop button appears next to the running request in the request explorer, *RQ: Cancel Running Request* is available from the command palette, the progress notification can be cancelled, and the response panel's *Run Again* button turns into *Cancel* while the request runs. *Run Request* is disabled until the running one finishes or is cancelled.
- **Suppress lint findings.** `// rq-lint-ignore <rule>[: reason]` on its own line covers the next statement (or the next entry inside an `env`, `auth`, `$[...]` or `${...}`), at the end of a line it covers that line, and `// rq-lint-ignore-file <rule>` covers the whole file. Rules are named explicitly — there is no catch-all. Two new rules that cannot be suppressed keep them honest: `invalid_lint_suppression` (unknown or missing rule, misplaced file directive) and `unused_lint_suppression` (a suppression that silences nothing).
- **Quick fixes for lint findings.** *Quick Fix…* on a lint warning offers *Suppress `<rule>` on this line / for this statement / for this file*, merging into an existing directive when there is one and respecting indentation and CRLF. `hardcoded_secret` is never offered file-wide. An unused suppression offers *Remove unused suppression*.
- **`io.read_json(path)`** reads a JSON file next to the `.rq` file and returns it as a JSON value, so the body is sent with `Content-Type: application/json` unless you declare one. Invalid JSON is reported before anything is sent. Autocomplete and signature help include it.
- **Debug trace in the output channel.** With `rq.debugLogging` on, the RQ output channel shows a `curl -v`-style trace of each run: files parsed, environments, secret sources (by name only), where each variable comes from, the resolved request and the response, and OAuth2 token requests. Secret values are masked.
- The bundled MCP server instructions and its `generate_rq` prompt forbid adding `rq-lint-ignore` comments unless you ask for them, so an assistant cannot pass the linter by silencing what it was asked to fix. Its tool schemas are now JSON Schema 2020-12.
- Added a **Lint Rules** documentation page, linked from the VS Code extension guide, and *Suppressing lint findings* and *Value types* sections in the language definition.
- Upgraded the extension's npm dependencies, including `@modelcontextprotocol/sdk` 1.32.

## [0.7.0]

### Bug Fixes

- Fixed the extension failing with a directory-not-found error in a workspace that contains no `.rq` files yet — listing requests, endpoints, environments, variables and auth configs now returns empty instead of erroring.
- Fixed autocomplete stopping inside an `ep` parameter list — no suggestions after the first argument of a templated endpoint (`ep users<base>("/users", …)`). Argument positions are now detected with string- and comment-aware scanning, so completions are offered where an argument can actually go and nowhere else.
- Fixed parameter autocomplete offering a parameter whose slot a positional argument had already claimed — accepting the suggestion produced a `Duplicate parameter` error. Completion now applies the same first-unclaimed-slot rule as the parser.
- Fixed go-to-definition, find-all-references, rename and hover matching symbols by name across every `.rq` file in the workspace, which could jump to an unrelated file the current one never imports. They now resolve through the file's import graph.
- Fixed autocomplete returning nothing — no variables, endpoints or auth configs — in a nested file whose `import` pointed outside its own directory.
- Fixed hover showing no tooltip for a variable declared in an imported file unless an environment was selected.
- Fixed `.env` secrets going missing while editing a file with unsaved changes.
- Fixed unresolved-variable errors inherited from an endpoint being reported on the request line instead of the endpoint that declares them. A variable used in an `ep` attribute is now reported on the attribute, and sibling requests that resolve cleanly are no longer blamed.
- Fixed `[auth("...")]` autocomplete suggesting auth provider types (`auth_type.*`) and top-level keywords alongside the auth blocks actually declared. It now lists only the providers reachable from the file and its imports, shows each one's auth type, and works unquoted and across lines.
- Fixed a parse error in an imported file being reported once per importing file instead of once, on the file that contains it.
- Fixed named and positional parameters not being mixable in `rq` and `ep` declarations (`ep users<base>(url: "/users", $["X-U": "1"], "v=1")` now parses). A parameter given twice — repeated by name, or positionally and by name — is reported as `Duplicate parameter`.
- Fixed the hover tooltip for a local variable rendering a duplicate semicolon.
- Fixed the `json_body_as_string` lint rule firing on a URL built out of `{{...}}` interpolations, and reporting it on the importing file instead of the file that declares the variable.
- Fixed bare numeric values in the documented `let` examples (`let user_id = 1;`), which the parser rejects — the language reference and the advanced walkthrough now quote them.

### Enhancements

- **Idiom linting in the editor.** rq's style and idiom rules now run alongside the parser and appear in the Problems panel as warnings, tagged with the rule that produced them (`rq lint` source) and, where the fix is mechanical, a suggested rewrite. Rules are workspace-aware, so an endpoint duplicating a query string or auth provider declared in another `.rq` file is flagged even when that file is not open. Unsaved edits are linted as you type. Disable with the new `rq.lint.enabled` setting; toggling it revalidates the open files right away.
- **AI assistance via a bundled MCP server.** The extension now ships a Model Context Protocol server and registers it automatically, so Copilot Chat and other MCP-aware chats in VS Code can validate `.rq` source, run the idiom linter, and enumerate existing requests. It also publishes the language definition and idioms guide as resources and a `generate_rq` prompt that drives the full generate → validate → lint loop. The server runs on the editor's own Node.js and reuses the extension's WebAssembly build of rq — no native binary and no platform-specific download.
- Seventeen lint rules ship in this release, covering file and endpoint structure (`top_level_rq_should_be_ep`, `multiple_endpoints_per_file`, `duplicated_ep_base`, `duplicated_ep_config`, `base_ep_extension`, `duplicated_noun_in_ep`), requests and bodies (`empty_url_string`, `missing_body_on_write`, `json_body_as_string`, `redundant_content_type_on_json_body`, `duplicated_request_qs`, `query_param_as_header`), credentials (`manual_auth_header`, `hardcoded_secret`), and variables and imports (`single_brace_interpolation`, `let_default_instead_of_required`, `absolute_import_path`). Each rule and what it asks for is listed in the MCP Server guide.
- An `[auth("name")]` referencing an auth block that is not declared in the file or its imports is now reported as an error. Previously the reference was ignored and the request was sent with no auth at all; it now fails with `Auth configuration 'name' not found`.
- Reworked the code snippets so every one of them inserts parseable, idiomatic rq. The CRUD endpoint snippet covers `put` as well, marks each path identifier `[required(...)]` and no longer declares a `let` standing in for a runtime input; the `auth` snippets reference variables for credentials instead of inline empty strings; `rq`, `ep` and `env` come with placeholder URLs.
- Added two documentation pages, linked from the extension README: **AI-Assisted Authoring**, a walkthrough that grows a request suite through eight prompts and ends with one generated from an OpenAPI spec, and **MCP Server**, which documents the tools, their parameters, the style rules, and the limits of what the checks guarantee.
- The marketplace listing now says the extension does AI-assisted authoring and idiom linting: added the AI and Linters categories, plus MCP, Copilot and AI keywords.

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
