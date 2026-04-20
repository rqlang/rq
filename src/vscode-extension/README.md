# RQ Language Support for VS Code

Language support for rq – a domain-specific language designed for managing and executing HTTP requests.

```
rq get("https://rqlang.com");
```

This extension adds syntax highlighting, a built-in language server, IntelliSense, and a Request Explorer for `.rq` files, with full support for auth flows including interactive OAuth.

![rq animation](https://raw.githubusercontent.com/rqlang/rq/main/docs/media/rq.gif)

Want the full tour? Check out the [VS Code Extension docs](https://www.rqlang.com/docs/VSCODE_EXTENSION.html).

## Docs

- [Getting Started](https://www.rqlang.com/docs/GETTING_STARTED.html) — new here? Start here.
- [Language Definition](https://www.rqlang.com/docs/LANGUAGE_DEFINITION.html) — everything rq can do.
- [Installation Guide](https://www.rqlang.com/docs/INSTALLATION.html) — get up and running.

## Core features (overview)

- Syntax highlighting for rq keywords, HTTP methods, attributes and interpolations.
- **Language server** — real-time diagnostics as you type: parse errors, semantic errors, and missing variables are surfaced in the Problems panel without running a request.
- **IntelliSense** — autocomplete for variables, request names, environments, auth providers, object parameters, auth properties, attributes, and imports.
- **Hover tooltips** — inline summaries for `rq`, `ep`, `env`, and `auth` statements.
- **Navigation** — go to definition, find all references, and rename symbol across the workspace.
- **Format document** — format the current `.rq` file via the **Format Document** command (`Shift+Alt+F` on Windows/Linux, `Shift+Option+F` on macOS).
- **Request Explorer** sidebar to browse and run requests, with environment selection and run/run-with-variables actions.
- **Configuration view** — a dedicated sidebar view listing the environments and auth providers defined across your workspace.
- **Response panel** — view response status, headers, and formatted body; copy the body with one click.
- **Integration with rq auth providers, including interactive OAuth flows (Authorization Code with PKCE and Implicit).**
