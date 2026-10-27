# Local Development

This document lists the prerequisites and how to install them to develop RQ locally.

## Prerequisites

- VS Code
- Node.js (LTS recommended) + npm
- GitHub CLI (`gh`) (required by the `install-rq-dev` scripts and to download dev artifacts)
- Rust (`cargo`) (stable toolchain)
- `wasm-pack` (≥ 0.14.0)
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
- Add the WASM target (needed by `src/rq-wasm`): `rustup target add wasm32-unknown-unknown`

Verify:

- `rustc --version`
- `cargo --version`
- `rustup target list --installed` includes `wasm32-unknown-unknown`

### wasm-pack

- Install via cargo: `cargo install wasm-pack`

Verify:

- `wasm-pack --version`

### Docker

- Install Docker (Docker Desktop or engine): https://www.docker.com/products/docker-desktop/

Verify:

- `docker --version`
- `docker compose version`

## Quick repo check

Run the Rust commands from the repo root; the workspace covers `src/cli` and `src/rq-lib`.

- CLI and library (Rust):
  - Start the echo server (required for `cargo test`):
    - `docker run -d --rm --name rq-echo -p 8080:80 ealen/echo-server`
    - Wait until it responds:
      - `curl -s http://localhost:8080 > /dev/null`
  - `cargo test`
  - Stop the echo server when done:
    - `docker stop rq-echo`

- WASM build (`src/rq-wasm` is not a workspace member):
  - `cd src/rq-wasm`
  - `cargo test`
  - `cargo check --target wasm32-unknown-unknown`

- VS Code extension (Node/TS):
  - `cd src/vscode-extension`
  - `npm ci`
  - `npm run build-wasm`
  - `npm run compile`

- MCP server (`@rqlang/mcp`, bundled from the extension's `src/mcp/`):
  - `cd src/rq-mcp`
  - `npm ci`
  - `npm run build-wasm`
  - `npm run build`
  - `npm test`

## Installing dev builds

Pushes to `main` run `release_cd.yaml`, which builds the CLI for every platform and the extension VSIX as workflow artifacts. One build runs at a time, and of the pushes that arrive meanwhile only the latest is built, so not every commit gets its own artifacts. You can install the artifacts of the latest successful run to try `main` without building it yourself.

### CLI (`rq-dev`)

The dev CLI is installed as `rq-dev` (`rq-dev.exe` on Windows), so it runs alongside a released `rq` without replacing it.

Linux and macOS:

```bash
./deployment/install-rq-dev.sh
```

| Option | Meaning |
|---|---|
| `-n, --name NAME` | Name of the installed binary. Defaults to `rq-dev`. |
| `-d, --install-dir DIR` | Directory to install into. Defaults to `/usr/local/bin` as root and `~/.local/bin` otherwise. |

Windows:

```powershell
.\deployment\install-rq-dev.ps1
```

| Parameter | Meaning |
|---|---|
| `-Name NAME` | Name of the installed executable, without extension. Defaults to `rq-dev`. |
| `-InstallDir DIR` | Directory to install into. Defaults to `%LOCALAPPDATA%\rq-dev`, which is added to the user PATH. |

Both scripts run the downloaded binary before installing it and stop if it does not start. To uninstall, delete the binary (and, on Windows, remove its directory from the user PATH).

### VS Code extension

Download the VSIX artifact (`rq-language-extension-<version>`) of the latest successful run, either from the run's page under [Actions → release_cd.yaml](https://github.com/rqlang/rq/actions/workflows/release_cd.yaml) or with `gh`:

```bash
run_id=$(gh run list --repo rqlang/rq --workflow release_cd.yaml --status success --limit 1 --json databaseId --jq '.[0].databaseId')
gh run download "$run_id" --repo rqlang/rq --pattern 'rq-language-extension-*'
```

Then install the `.vsix` from VS Code, as described in [Install from a VSIX](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace#_install-from-a-vsix), or with `code --install-extension <file>.vsix`.

### Claude Code plugin

Load the plugin from the checkout for one session, without installing it:

```bash
claude --plugin-dir ./src/claude-plugin
```

The checkout pins `@rqlang/mcp@0.0.0`, which release builds replace with the release version, so point `src/claude-plugin/.mcp.json` at a local build (`node ${CLAUDE_PLUGIN_ROOT}/../rq-mcp/dist/server.js`; plugin servers start in the session's working directory, not in the plugin folder) instead of `npx`, and do not commit that change.

## Release channels

Released versions reach users through the GitHub Release, the VS Code Marketplace, Homebrew, Scoop, WinGet and the install script; [INSTALLATION.md](INSTALLATION.md) is the user-facing guide and [.github/workflows/README.md](../.github/workflows/README.md) describes the release process.

| Channel | Source in this repo | Published to |
|---|---|---|
| Install script (Linux and macOS) | `deployment/install-rq.sh` | Served from `main` through `raw.githubusercontent.com` |
| Homebrew | `deployment/homebrew/render-formula.sh` | `Formula/rqlang.rb` in [rqlang/homebrew-tap](https://github.com/rqlang/homebrew-tap) |
| Scoop | `deployment/scoop/render-manifest.sh` | `bucket/rqlang.json` in [rqlang/scoop-bucket](https://github.com/rqlang/scoop-bucket) |
| WinGet | `deployment/winget/render-manifests.sh` | Pull request to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) as `rqlang.rq` |

The renderers take a release tag and its `SHA256SUMS` and print the formula or manifests, so you can see what a release would publish:

```bash
gh release download 0.8.0 --repo rqlang/rq --pattern SHA256SUMS
deployment/homebrew/render-formula.sh 0.8.0 SHA256SUMS
deployment/scoop/render-manifest.sh 0.8.0 SHA256SUMS
mkdir -p winget-out && deployment/winget/render-manifests.sh 0.8.0 SHA256SUMS winget-out
```

The Scoop renderer needs `jq`. Because the install script is served straight from `main`, a change to it reaches users as soon as it is merged.

### Testing the deployment scripts

`deployment_bvt.yaml` runs these tests on every pull request that touches `deployment/`; run them locally after changing a script:

| Script | Test |
|---|---|
| `install-rq.sh` | `deployment/test/install-rq/run.sh` |
| `install-rq-dev.sh` | `deployment/test/install-rq-dev/run.sh` |
| `install-rq-dev.ps1` | `deployment/test/install-rq-dev/run.ps1` (Windows, `pwsh`) |
| `uninstall-rq.ps1` | `deployment/test/uninstall-rq/run.ps1` (Windows, `pwsh`) |
| Homebrew renderer | `deployment/homebrew/test/run.sh` |
| Scoop renderer | `deployment/scoop/test/run.sh` |
| WinGet renderer | `deployment/winget/test/run.sh` |

The shell tests replace `curl`, `gh` and `uname` with fakes, so they need no network access and do not install anything outside a temporary directory. To check a formula or manifest against a real release on real Homebrew, Scoop and WinGet, run `publish_release.yaml` with `dry_run` checked.

## Notes

- CLI unit/integration tests expect an HTTP echo server at `http://localhost:8080`.
