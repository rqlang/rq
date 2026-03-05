# RQ Language Support for VS Code

Language support for rq – a domain-specific language designed for managing and executing HTTP requests.

```
rq get("https://rqlang.com");
```

This extension adds syntax highlighting, IntelliSense and a Request Explorer for `.rq` files, and integrates with the rq CLI and auth flows, including interactive OAuth flows.

![rq animation](https://raw.githubusercontent.com/rqlang/rq/main/docs/media/rq.gif)

Want the full tour? Check out the [VS Code Extension docs](https://www.rqlang.com/docs/VSCODE_EXTENSION.html).

## Docs

- [Getting Started](https://www.rqlang.com/docs/GETTING_STARTED.html) — new here? Start here.
- [Language Definition](https://www.rqlang.com/docs/LANGUAGE_DEFINITION.html) — everything rq can do.
- [Installation Guide](https://www.rqlang.com/docs/INSTALLATION.html) — get up and running.

## Core features (overview)

- Syntax highlighting for rq keywords, HTTP methods, attributes and interpolations.
- Request Explorer sidebar to browse and run requests with environment selection.
- IntelliSense for functions, variables, and keywords.
- Integration with rq auth providers, including interactive OAuth flows.

## Requirements

> **Note:** This extension requires the **rq CLI executable** to function. On activation, the extension automatically detects whether the CLI is installed or needs to be updated, and downloads the appropriate binary from the [latest GitHub release](https://github.com/rqlang/rq/releases/latest) for your platform. See the [Installation Guide](https://www.rqlang.com/docs/INSTALLATION.html) for more details.