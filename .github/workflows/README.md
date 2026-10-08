# Workflows

GitHub Actions workflows for building, testing and releasing `rq`.

## Overview

| Workflow | Trigger | Purpose |
|---|---|---|
| `cli_bvt.yaml` | PR to `main` touching Rust code | fmt, clippy, build and test the Cargo workspace |
| `wasm_bvt.yaml` | PR to `main` touching `rq-wasm` or `rq-lib` | fmt, clippy, test and `wasm32` check of `rq-wasm` |
| `extension_bvt.yaml` | PR to `main` touching the extension | Lint, compile and test the VS Code extension |
| `codeql.yml` | Nightly | CodeQL analysis for Rust and JavaScript |
| `fuzz_nightly.yaml` | Nightly and manual | Fuzz the tokenizer and parser; opens an issue on a crash |
| `release_cd.yaml` | Push to `main` | Dev builds of the CLI and the extension as workflow artifacts |
| `release_prod.yaml` | GitHub Release published | Release builds uploaded to the GitHub Release |
| `publish_extension.yaml` | Manual | Publishes a release VSIX to the VS Code Marketplace |
| `publish_homebrew.yaml` | Called by `release_prod.yaml`, or manual | Renders, verifies and pushes the Homebrew formula to the tap |
| `build_cli.yaml` | Called by the release workflows | Builds and signs the CLI for every platform |
| `build_extension.yaml` | Called by the release workflows | Packages the VS Code extension VSIX |

## CLI platforms

`build_cli.yaml` builds one binary per target and uploads it as a workflow artifact named after the asset:

| Asset | Target | Runner |
|---|---|---|
| `rq-linux-x86_64` | `x86_64-unknown-linux-gnu` | `ubuntu-22.04` |
| `rq-linux-aarch64` | `aarch64-unknown-linux-gnu` | `ubuntu-22.04-arm` |
| `rq-windows-x86_64.exe` | `x86_64-pc-windows-msvc` | `windows-latest` |
| `rq-macos-x86_64` | `x86_64-apple-darwin` | `macos-latest` |
| `rq-macos-aarch64` | `aarch64-apple-darwin` | `macos-latest` |

OpenSSL is vendored, so the binaries do not depend on the system `libssl`. The Linux binaries are built on Ubuntu 22.04 so that they run on any distribution with glibc 2.35 or newer. The Windows binary is signed with the certificate in `CODE_SIGNING_CERT`.

## Versioning

The repository keeps `version = "0.0.0"` in the root `Cargo.toml`; the workflows rewrite it before building.

- **Dev (`release_cd.yaml`)**: `{next minor}-dev.{commits since tag}`. With latest tag `0.7.0` and 5 commits since, the version is `0.8.0-dev.5`. Without any tag the base is `0.0.0`, so the version is `0.1.0-dev.{commits}`.
- **Release (`release_prod.yaml`)**: the release tag, without a leading `v` if present.

## Release process

1. Make sure `main` is green.
2. On GitHub, go to Releases → "Draft a new release", create the tag (e.g. `0.8.0`) and write the notes.
3. Publish the release. `release_prod.yaml` then:
   - builds the CLI for every platform and the VSIX,
   - once every build has succeeded, uploads all binaries, the VSIX and a `SHA256SUMS` file to the release,
   - records build provenance attestations for every binary and the VSIX listed in `SHA256SUMS`,
   - unless the release is a pre-release, publishes the Homebrew formula (see below).
4. Run `publish_extension.yaml` with the tag to publish the VSIX to the Marketplace.

Nothing is uploaded to the release unless every build succeeds. To retry a failed release, re-run the failed jobs of the workflow run.

`rq-linux-ubuntu-22.04-x86_64` is still uploaded as a copy of `rq-linux-x86_64` so that existing installers keep working.

## Homebrew

The CLI is distributed for macOS and Linux (x86_64 and arm64) through the tap [`rqlang/homebrew-tap`](https://github.com/rqlang/homebrew-tap). The formula is named `rqlang` because `rq` is already taken by an unrelated cask in `homebrew/cask`; the installed command is still `rq`.

```bash
brew install rqlang/tap/rqlang
```

`publish_homebrew.yaml` renders `Formula/rqlang.rb` from the release `SHA256SUMS` with `deployment/homebrew/render-formula.sh`, installs and tests it from a local tap on every platform, and only then pushes it to the tap with the `HOMEBREW_TAP_TOKEN` secret (a fine-grained token with `Contents: write` on `rqlang/homebrew-tap`). To retry or republish a release, run it manually with the tag. Releases published before `SHA256SUMS` and `rq-linux-aarch64` existed (0.7.0 and older) cannot be published this way.

To test the renderer locally:

```bash
deployment/homebrew/test/run.sh
```

## Verifying a download

```bash
sha256sum --check --ignore-missing SHA256SUMS
gh attestation verify rq-linux-x86_64 --repo rqlang/rq
```

## Dev builds

`release_cd.yaml` uploads the CLI binaries and the VSIX as workflow artifacts (not releases). One build runs at a time: a running build always finishes, and of the pushes that arrive meanwhile only the latest is built next. The `deployment/install-rq-dev*` scripts download these artifacts from the latest successful run.

## Local build

```bash
cargo build --release -p rq
```
