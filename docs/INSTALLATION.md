---
layout: default
title: Installation
nav_order: 3
---

# Installation

rq is a domain-specific language designed for managing and executing HTTP requests.

This document describes how to install the rq VS Code extension.

## VS Code extension

The rq VS Code extension is distributed through the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=rq-lang.rq-language)

## VS Code Extension (Manual Install from VSIX)

Each release publishes a `.vsix` file as a release asset on GitHub. You can download and install it manually if you prefer not to use the Marketplace.

1. Go to the [Releases page](https://github.com/rqlang/rq/releases) and open the release you want.
2. Under **Assets**, download the `.vsix` file (e.g. `rq-language-extension-0.2.0.vsix`).
3. Open VS Code and go to the **Extensions** view (`Ctrl+Shift+X` / `Cmd+Shift+X`).
4. Click the **`...`** icon (top-right of the Extensions panel, next to the refresh button).
5. Select **Install from VSIX...** and choose the downloaded file.

## CLI on Linux (Install Script)

The install script downloads the CLI binary for your architecture (`x86_64` or `aarch64`) from the latest GitHub release, checks it against the release's `SHA256SUMS`, renames it to `rq` and copies it to a directory on your `PATH`:

```bash
curl -fsSL https://raw.githubusercontent.com/rqlang/rq/main/deployment/install-rq.sh | bash
```

Releases published before `SHA256SUMS` existed (0.7.0 and older) cannot be verified, so the script refuses to install them; download those manually as described below.

The Linux binaries need glibc 2.34 or newer and OpenSSL 3 (`libssl.so.3`), which Ubuntu 22.04, Debian 12, RHEL 9 and later ship by default. The script runs the downloaded binary before installing it, so on an older system it stops with an error and installs nothing.

Where it installs depends on who runs it:

- **As a regular user**: `~/.local/bin/rq`. No `sudo` needed, and only your user gets `rq`.
- **As root**: `/usr/local/bin/rq`, available to every user on the machine:

  ```bash
  curl -fsSL https://raw.githubusercontent.com/rqlang/rq/main/deployment/install-rq.sh | sudo bash
  ```

Most distributions already put `~/.local/bin` on the `PATH`. If yours does not, the script says so and prints the line to add to your shell profile (`~/.bashrc`, `~/.zshrc`, …):

```bash
export PATH="$HOME/.local/bin:$PATH"
```

To install a specific release or choose the target directory yourself, pass options after `bash -s --`:

```bash
curl -fsSL https://raw.githubusercontent.com/rqlang/rq/main/deployment/install-rq.sh | bash -s -- --release-tag 0.8.0 --install-dir /opt/rq/bin
```

| Option | Meaning |
|---|---|
| `-r, --release-tag TAG` | Release to install (for example `0.8.0`). Defaults to the latest release. |
| `-d, --install-dir DIR` | Directory to copy `rq` into. Overrides the default location. |

Running the script again installs over the existing binary, so the same command upgrades `rq`. Check the result with:

```bash
rq --version
```

To uninstall, delete the binary: `rm ~/.local/bin/rq` (or `sudo rm /usr/local/bin/rq`).

## CLI (Manual Download)

If you want to use the rq CLI independently, you can download the prebuilt **portable** binary directly from the GitHub Releases page. They need no installation steps; on Linux they need glibc 2.34 or newer and OpenSSL 3 (`libssl.so.3`).

1. Open the Releases page in your browser:
	- https://github.com/rqlang/rq/releases
2. In the **Assets** section of the release, download the file that matches your platform:
	- **Windows**: `rq-windows-x86_64.exe`
	- **Linux (Ubuntu 22.04 x86_64)**: `rq-linux-ubuntu-22.04-x86_64`
	- **Linux (other x86_64)**: `rq-linux-x86_64`
	- **Linux (aarch64)**: `rq-linux-aarch64`
	- **macOS Intel (x86_64)**: `rq-macos-x86_64`
	- **macOS Apple Silicon (M1/M2)**: `rq-macos-aarch64`
3. Optionally rename the downloaded file to a shorter name (for example `rq`).
4. Make the file executable (Linux / macOS):

  ```bash
  chmod +x ./rq-linux-x86_64
  ```

5. Either run it directly from the download location:

  ```bash
  ./rq-linux-x86_64 --version
  ```

	Or move it to a directory on your `PATH` (for example `/usr/local/bin` or `~/.local/bin`) and run it from anywhere:

  ```bash
  mv ./rq-linux-x86_64 /usr/local/bin/rq
  rq --version
  ```

On Windows, you can run the portable executable directly after downloading (for example from your Downloads folder):

```powershell
cd $env:USERPROFILE\Downloads
./rq-windows-x86_64.exe --version
```

For convenience, you can also move `rq-windows-x86_64.exe` to a folder that is already on your `PATH` (or update the `PATH` environment variable) so that you can run `rq` from any directory.

