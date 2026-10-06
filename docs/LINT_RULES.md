---
layout: default
title: Lint Rules
nav_order: 10
---

{% raw %}

# Lint Rules

Beyond parse and semantic errors, rq checks `.rq` files against a set of idiom and style rules. The same rules run in `rq check`, which reports them as warnings (see [Validating files](CLI.md#validating-files-rq-check)), in the editor — see [Idiom linting](VSCODE_EXTENSION.md#idiom-linting) — and in the `lint_rq` tool of the [MCP Server](MCP_SERVER.md).

Every finding names the rule that produced it. In VS Code the id is shown next to the `rq lint` source in the Problems panel, e.g. `rq lint(empty_url_string)`; `rq check` prints it as `warning[empty_url_string]`; the MCP server returns it in the `rule` field. That id is what an `rq-lint-ignore` comment takes.

## Rules

| Rule | What it wants |
|---|---|
| `top_level_rq_should_be_ep` | Two top-level requests share a base URL — group them in an `ep` |
| `duplicated_noun_in_ep` | Request inside `ep users` is named `get_users` — use the verb alone |
| `empty_url_string` | `rq list("")` — inside an `ep` write `rq list()` |
| `json_body_as_string` | `body: "{}"` sends a string — use `body: ${}` |
| `redundant_content_type_on_json_body` | rq derives `Content-Type` from a JSON body |
| `missing_body_on_write` | A POST/PUT/PATCH with no payload |
| `multiple_endpoints_per_file` | Split into one `ep` per file |
| `duplicated_ep_base` / `duplicated_ep_config` | Two endpoints repeat a base URL, headers or auth — extract a template |
| `base_ep_extension` | A template with only one consumer — inline it |
| `manual_auth_header` | Hand-built `Authorization` — use an `auth` provider |
| `hardcoded_secret` | A credential literal in a committed file — move it to `.env` |
| `single_brace_interpolation` | `{name}` is literal text — use `{{name}}` |
| `query_param_as_header` | A query parameter passed where headers go |
| `duplicated_request_qs` | The same query parameter written into sibling requests — put it on the `ep` as `qs` |
| `let_default_instead_of_required` | A `let` standing in for a runtime input — use `[required(...)]` |
| `absolute_import_path` | `import "/Users/..."` — use a relative path |
| `invalid_lint_suppression` | An `rq-lint-ignore` comment naming an unknown rule or no rule |
| `unused_lint_suppression` | An `rq-lint-ignore` comment that silences nothing |

The rules are picky in both directions. `duplicated_ep_base` asks you to extract a template once two endpoints share a base URL — and `base_ep_extension` rejects that same template while only one endpoint extends it. A template earns its keep from the second consumer onward, not before.

## Suppressing a finding

A finding you have decided to keep is silenced with an `rq-lint-ignore` comment naming its rule — see [Suppressing lint findings](LANGUAGE_DEFINITION.md#suppressing-lint-findings). `invalid_lint_suppression` and `unused_lint_suppression` check those comments themselves and cannot be suppressed.

{% endraw %}
