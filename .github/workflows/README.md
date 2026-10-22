# Workflows

GitHub Actions workflows for building, testing and releasing `rq`.

## Overview

| Workflow | Trigger | Purpose |
|---|---|---|
| `cli_bvt.yaml` | PR to `main` touching Rust code | fmt, clippy, build and test the Cargo workspace |
| `wasm_bvt.yaml` | PR to `main` touching `rq-wasm` or `rq-lib` | fmt, clippy, test and `wasm32` check of `rq-wasm` |
| `extension_bvt.yaml` | PR to `main` touching the extension | Lint, compile and test the VS Code extension |
| `mcp_bvt.yaml` | PR to `main` touching the MCP server, `rq-wasm` or `rq-lib` | Build `rq-wasm` and the `@rqlang/mcp` package, and test it end to end over stdio |
| `deployment_bvt.yaml` | PR to `main` touching `deployment/` | Tests the install script and the Homebrew, Scoop and WinGet renderers on Linux and macOS |
| `codeql.yml` | Nightly | CodeQL analysis for Rust and JavaScript |
| `fuzz_nightly.yaml` | Nightly and manual | Fuzz the tokenizer and parser; opens an issue on a crash |
| `release_cd.yaml` | Push to `main` | Dev builds of the CLI and the extension as workflow artifacts |
| `release_prod.yaml` | GitHub Release published | Release builds uploaded to the GitHub Release |
| `publish_release.yaml` | Manual | Publishes a release to the chosen channels, or only verifies them with `dry_run` |
| `publish_extension.yaml` | Called by `publish_release.yaml`, or manual | Publishes a release VSIX to the VS Code Marketplace |
| `publish_homebrew.yaml` | Called by `publish_release.yaml`, or manual | Renders, verifies and pushes the Homebrew formula to the tap |
| `publish_scoop.yaml` | Called by `publish_release.yaml`, or manual | Renders, verifies and pushes the Scoop manifest to the bucket |
| `publish_npm.yaml` | Called by `publish_release.yaml`, or manual | Smoke-tests the release `@rqlang/mcp` tarball and publishes it to npm |
| `publish_winget.yaml` | Called by `publish_release.yaml`, or manual | Renders, verifies and submits the WinGet manifests to `microsoft/winget-pkgs` |
| `build_cli.yaml` | Called by the release workflows | Builds and signs the CLI for every platform |
| `build_extension.yaml` | Called by the release workflows | Packages the VS Code extension VSIX |
| `build_mcp.yaml` | Called by the release workflows | Builds, tests and packs the `@rqlang/mcp` npm tarball |

## CLI platforms

`build_cli.yaml` builds one binary per target and uploads it as a workflow artifact named after the asset:

| Asset | Target | Runner |
|---|---|---|
| `rq-linux-x86_64` | `x86_64-unknown-linux-gnu` | `ubuntu-22.04` |
| `rq-linux-aarch64` | `aarch64-unknown-linux-gnu` | `ubuntu-22.04-arm` |
| `rq-windows-x86_64.exe` | `x86_64-pc-windows-msvc` | `windows-latest` |
| `rq-macos-x86_64` | `x86_64-apple-darwin` | `macos-latest` |
| `rq-macos-aarch64` | `aarch64-apple-darwin` | `macos-latest` |

OpenSSL is vendored, so the binaries do not depend on the system `libssl`; an OpenSSL security fix reaches users only through a new release with an updated `openssl-src`. The Linux binaries are built on Ubuntu 22.04 and need glibc 2.34 or newer. The Windows binary is signed with the certificate in `CODE_SIGNING_CERT`.

## Versioning

The repository keeps `version = "0.0.0"` in the root `Cargo.toml`; the workflows rewrite it before building.

- **Dev (`release_cd.yaml`)**: `{next minor}-dev.{commits since tag}`. With latest tag `0.7.0` and 5 commits since, the version is `0.8.0-dev.5`. Without any tag the base is `0.0.0`, so the version is `0.1.0-dev.{commits}`.
- **Release (`release_prod.yaml`)**: the release tag, without a leading `v` if present.

## Release process

1. Make sure `main` is green.
2. On GitHub, go to Releases → "Draft a new release", create the tag (e.g. `0.8.0`) and write the notes.
3. Publish the release. `release_prod.yaml` then:
   - builds the CLI for every platform, the VSIX and the `@rqlang/mcp` tarball,
   - once every build has succeeded, uploads all binaries, the VSIX, the tarball and a `SHA256SUMS` file to the release,
   - records build provenance attestations for every asset listed in `SHA256SUMS`.

   Nothing is published to any channel yet, so the release assets can be tested first.
4. Run `publish_release.yaml` with the tag and `dry_run` checked (the default). It renders and verifies the Homebrew formula, the Scoop manifest and the WinGet manifests, finds the VSIX and smoke-tests the npm tarball, without publishing anything.
5. Run `publish_release.yaml` again with `dry_run` unchecked to publish to the VS Code Marketplace, npm, the Homebrew tap and the Scoop bucket, and to open a pull request with the WinGet manifests (see below). Untick a channel to leave it out.

Nothing is uploaded to the release unless every build succeeds. To retry a failed release, re-run the failed jobs of the workflow run.

`rq-linux-ubuntu-22.04-x86_64` is still uploaded as a copy of `rq-linux-x86_64` so that existing installers keep working.

## Homebrew

The CLI is distributed for macOS and Linux (x86_64 and arm64) through the tap [`rqlang/homebrew-tap`](https://github.com/rqlang/homebrew-tap). The formula is named `rqlang` because `rq` is already taken by an unrelated cask in `homebrew/cask`; the installed command is still `rq`.

```bash
brew install rqlang/tap/rqlang
```

`publish_homebrew.yaml` renders `Formula/rqlang.rb` from the release `SHA256SUMS` with `deployment/homebrew/render-formula.sh`, installs and tests it from a local tap on every platform, and only then pushes it to the tap with the `HOMEBREW_TAP_TOKEN` secret (a fine-grained token with `Contents: Read and write` on `rqlang/homebrew-tap`). Pre-releases are rejected, and the tap never moves back to an older version than the one it already has. To retry or republish a release, run it manually with the tag. Releases published before `SHA256SUMS` and `rq-linux-aarch64` existed (0.7.0 and older) cannot be published this way.

To test the renderer locally:

```bash
deployment/homebrew/test/run.sh
```

## Scoop

The CLI is distributed for Windows (x86_64) through the bucket [`rqlang/scoop-bucket`](https://github.com/rqlang/scoop-bucket). The manifest is named `rqlang`, matching the Homebrew formula; the installed command is still `rq`.

```powershell
scoop bucket add rqlang https://github.com/rqlang/scoop-bucket
scoop install rqlang/rqlang
```

`publish_scoop.yaml` renders `bucket/rqlang.json` from the release `SHA256SUMS` with `deployment/scoop/render-manifest.sh`, installs it on Windows with the Scoop installer pinned in `SCOOP_INSTALLER_COMMIT` and checks that `rq --version` reports exactly that version, and only then pushes it to the bucket with the `SCOOP_BUCKET_TOKEN` secret (a fine-grained token with `Contents: Read and write` on `rqlang/scoop-bucket`). The manifest has no `checkver`/`autoupdate`: this workflow is the only thing that updates it. Pre-releases are rejected, and the bucket never moves back to an older version than the one it already has. To retry or republish a release, run it manually with the tag. Releases published before `SHA256SUMS` existed (0.7.0 and older) cannot be published this way.

Installing through both Scoop and WinGet leaves two `rq.exe` on `PATH`; use one or the other.

To test the renderer locally:

```bash
deployment/scoop/test/run.sh
```

## WinGet

The CLI is distributed for Windows (x86_64) through the community repository [`microsoft/winget-pkgs`](https://github.com/microsoft/winget-pkgs) as the portable package `rqlang.rq`; the installed command is `rq`.

```powershell
winget install rqlang.rq
```

`publish_winget.yaml` renders the version, installer and `en-US` locale manifests from the release `SHA256SUMS` with `deployment/winget/render-manifests.sh`, validates them with `winget validate`, installs them on Windows with `winget install --manifest` and checks that `rq --version` reports exactly that version. Only then does it open a pull request against `microsoft/winget-pkgs` with `wingetcreate submit`, pinned in `WINGETCREATE_VERSION` and checked against `WINGETCREATE_SHA256`. The pull request comes from a fork of `winget-pkgs` owned by the account behind the `WINGET_PKGS_TOKEN` secret (a classic token with the `public_repo` scope; `wingetcreate` does not support fine-grained tokens). The package is available once the WinGet moderators merge it. Pre-releases are rejected, and nothing is submitted when the version is already in `winget-pkgs` or an open pull request already submits it. To retry or republish a release, run it manually with the tag. Releases published before `SHA256SUMS` existed (0.7.0 and older) cannot be published this way.

Installing through both WinGet and Scoop leaves two `rq.exe` on `PATH`; use one or the other. `deployment/uninstall-rq.ps1` removes an `rq` installed by the former PowerShell install script.

To test the renderer locally:

```bash
deployment/winget/test/run.sh
```

## npm

The MCP server is distributed as [`@rqlang/mcp`](https://www.npmjs.com/package/@rqlang/mcp), so any MCP client can run it with `npx -y @rqlang/mcp`. It is the same server the extension registers for Copilot, bundled from `src/vscode-extension/src/mcp/` with `rq-wasm`; see [`src/rq-mcp/README.md`](../../src/rq-mcp/README.md).

`publish_npm.yaml` downloads `rqlang-mcp-<version>.tgz` from the release, installs it in an empty project and checks that `validate_rq` answers over stdio, and only then publishes it with `npm publish --provenance` using the `NPM_TOKEN` secret (a granular access token with read and write access to the `@rqlang` scope). Pre-releases are rejected, and a version already on npm is skipped. To retry or republish a release, run it manually with the tag.

## Install script

`deployment/install-rq.sh` installs the CLI on Linux and macOS straight from the release downloads on github.com, without the GitHub API. It verifies the binary against the release `SHA256SUMS` when the release publishes one, runs it before installing, and swaps it into place with a rename so it can upgrade a running `rq`.

`deployment_bvt.yaml` runs its tests on every PR that touches `deployment/`. To run them locally (they use a fake `curl`, no network needed):

```bash
deployment/test/install-rq/run.sh
```

## Verifying a download

```bash
sha256sum --check --ignore-missing SHA256SUMS
gh attestation verify rq-linux-x86_64 --repo rqlang/rq
```

## Dev builds

`release_cd.yaml` uploads the CLI binaries and the VSIX as workflow artifacts (not releases). One build runs at a time: a running build always finishes, and of the pushes that arrive meanwhile only the latest is built next. The `deployment/install-rq-dev.sh` and `deployment/install-rq-dev.ps1` scripts install the CLI from the latest successful run as `rq-dev`, so it does not replace a released `rq`. The VSIX is downloaded from the run and installed from VS Code; see [LOCAL_DEVELOPMENT.md](../../docs/LOCAL_DEVELOPMENT.md#installing-dev-builds).

## Local build

```bash
cargo build --release -p rq
```
