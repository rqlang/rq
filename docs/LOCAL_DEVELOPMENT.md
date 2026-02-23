# Local Development

This document lists the prerequisites and how to install them to develop RQ locally.

## Prerequisites

- VS Code
- Node.js (LTS recommended) + npm
- GitHub CLI (`gh`) (required by `install-rq-dev*` scripts)
- Rust (`cargo`) (stable toolchain)
- Docker (Docker Desktop or equivalent engine)

## Install prerequisites

### VS Code

- Download and install: https://code.visualstudio.com/
- Enable the `code` command on your PATH:
  - VS Code → Command Palette → “Shell Command: Install 'code' command in PATH”.

Verify:

- `code --version`

### Node.js

- Install Node.js LTS: https://nodejs.org/

Verify:

- `node --version`
- `npm --version`

### GitHub CLI (gh)

- Install: https://cli.github.com/
- Authenticate (required to download dev artifacts):
  - `gh auth login`

Verify:

- `gh --version`
- `gh auth status`

### Rust / Cargo

- Install via rustup: https://rustup.rs/

Verify:

- `rustc --version`
- `cargo --version`

### Docker

- Install Docker (Docker Desktop or engine): https://www.docker.com/products/docker-desktop/

Verify:

- `docker --version`
- `docker compose version`

## Quick repo check

From the repo root:

- CLI (Rust):
  - Start the echo server (required for `cargo test`):
    - `docker run -d --rm --name rq-echo -p 8080:80 ealen/echo-server`
    - Wait until it responds:
      - `curl -s http://localhost:8080 > /dev/null`
  - `cd cli`
  - `cargo test`
  - Stop the echo server when done:
    - `docker stop rq-echo`

- VS Code extension (Node/TS):
  - `cd vscode-extension`
  - `npm ci`
  - `npm run compile`

## Notes

- `install-rq-dev.sh` / `install-rq-dev.ps1` require `gh` installed and authenticated.
- If you choose “install on PATH” on macOS/Linux, it may require elevated permissions (sudo).
- CLI unit/integration tests expect an HTTP echo server at `http://localhost:8080`.
