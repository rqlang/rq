# rq

rq is a domain-specific language designed for managing and executing HTTP requests. It was originally inspired by the [HTTP file format](https://www.jetbrains.com/help/idea/exploring-http-syntax.html) created by JetBrains for their IDEs.

The project provides two main tools to work with `.rq` files:

- **Command-Line Interface (CLI)**: A powerful command-line tool that allows you to execute HTTP requests directly from your terminal, perfect for scripting, automation, and CI/CD pipelines.
- **VS Code Extension**: A rich, interactive experience within Visual Studio Code, featuring syntax highlighting, request execution, and a visual interface for managing your API requests.

## Status & Disclaimer

rq — both the language and the associated tools (CLI and VS Code extension) — is currently in **preview**. This means:

- You may encounter bugs, incomplete features, or rough edges in everyday use.
- The language syntax and semantics are **not yet stable**, and future versions may introduce **breaking changes**.

In particular, there is **no guarantee of long-term backward compatibility** for `.rq` files written against early versions of the language. If we discover that a particular construct, keyword, or design choice blocks important future capabilities, we reserve the right to:

- Redesign or remove that feature.
- Introduce new syntax that is not compatible with older `.rq` files.
- Adjust runtime behavior in ways that may require you to update existing files.

While this is not the primary goal, the priority at this stage is to keep the language flexible enough to evolve. If you rely on rq in critical environments, you should:

- Pin specific versions of the CLI and VS Code extension.
- Treat `.rq` files as **preview assets** that may need updates over time.
- Review release notes before upgrading to new versions.

Feedback, bug reports, and suggestions are very welcome and will directly influence how the language and tools evolve.

## Why rq?

rq is not just a text template format; it is a **programming language focused on the HTTP domain**. This has several advantages compared to ad‑hoc `.http` files or plain text snippets:

- **Structured, robust definitions**: Requests are expressed with clear syntax and semantics (requests, variables, endpoints, environments, etc.), which reduces ambiguity and makes it easier to validate and evolve your API interactions over time.
- **Safer than free‑form templates**: Because rq has a well-defined grammar, tools can parse and analyze your files reliably, catching many mistakes (missing variables, invalid attributes, inconsistent structures) before the request is even sent.
- **Fast, safe execution engine**: The runtime that executes rq requests is implemented in **Rust**, which means it is designed to be fast, memory‑efficient, and safe. You can run large suites of requests (for example in CI) with low overhead.
- **File‑based workflow, Git‑friendly**: rq is based on plain files in your repository. There is no hidden state or proprietary project format: you can version, review, branch, and merge your request definitions using the same Git workflows you already use for code.
- **Integrated, open tooling**: The main UI for working with rq is delivered as an extension for **Visual Studio Code**, an open and widely adopted editor. You can use your existing VS Code setup (themes, keybindings, extensions) while benefiting from a dedicated rq explorer, request runner, and rich response viewer.

## Installation

See [INSTALLATION.md](docs/INSTALLATION.md) for installation instructions.

## Getting Started

See [GETTING_STARTED.md](docs/GETTING_STARTED.md) for a guided introduction to rq and its core concepts.

## Language Definition

See [LANGUAGE_DEFINITION.md](docs/LANGUAGE_DEFINITION.md) for a detailed description of the rq language syntax and semantics.

## VS Code Extension

See [VSCODE_EXTENSION.md](docs/VSCODE_EXTENSION.md) for details about the Visual Studio Code extension and its features.

## CLI

See [CLI.md](docs/CLI.md) for documentation of the rq command-line interface.