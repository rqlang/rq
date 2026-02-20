# RQ Language Support for VS Code

Language support for rq – a domain-specific language designed for managing and executing HTTP requests.

This extension adds syntax highlighting, IntelliSense and a Request Explorer for `.rq` files, and integrates with the rq CLI and auth flows, including interactive OAuth flows.

![rq animation](https://github.com/rqlang/rq/blob/main/docs/media/rq.gif)

> ## ⚠️ CLI Installation
>
> This extension requires the **rq CLI** as its backend. On first launch it will check whether the CLI is installed and up to date. If it is missing or outdated, the extension will offer to install or update it automatically.
>
> The goal is to make rq available in any terminal so you can also use it from the command line.
>
> On **Ubuntu Desktop** and **Windows** the installation is fully automatic — no prompts, no password needed. On Ubuntu the binary is installed to `~/.local/bin`, which is already in your PATH. On Windows it is installed to `%LOCALAPPDATA%\rq` and added to your user PATH automatically.
>
> On **WSL**, **macOS**, and **other Linux** distributions the extension will ask you to choose between installing system-wide to `/usr/local/bin` (requires `sudo`) or locally to `~/.local/bin` (no password needed — you may need to add it to your PATH manually).
>
> Your choice is saved in the VS Code setting `rq.cli.installOnPath` and can be changed later.

## Language and extension documentation

For full documentation of the language and the VS Code extension, see the main project docs:

- Getting started: [Getting Started](https://github.com/rqlang/rq/blob/main/docs/GETTING_STARTED.md)
- rq language: [Language Definition](https://github.com/rqlang/rq/blob/main/docs/LANGUAGE_DEFINITION.md)
- VS Code extension: [VS Code Extension](https://github.com/rqlang/rq/blob/main/docs/VSCODE_EXTENSION.md)

## Core features (overview)

- Syntax highlighting for rq keywords, HTTP methods, attributes and interpolations.
- Request Explorer sidebar to browse and run requests with environment selection.
- IntelliSense for functions, variables, and keywords.
- Integration with rq auth providers, including interactive OAuth flows.

For a detailed tour of all features, refer to [VS Code Extension](https://github.com/rqlang/rq/blob/main/docs/VSCODE_EXTENSION.md).