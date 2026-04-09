# Changelog

## [0.3.0]

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
