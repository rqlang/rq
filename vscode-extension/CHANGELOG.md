# Changelog

All notable changes to the rq VS Code extension will be documented in this file.

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
