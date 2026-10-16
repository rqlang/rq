---
layout: default
title: Installation
nav_order: 3
---

# Installation

rq ships as two pieces that can be installed independently:

- the **CLI** (`rq`), to run and check `.rq` files from a terminal or a CI pipeline;
- the **VS Code extension**, which bundles its own copy of the rq engine, so it does not need the CLI.

## Which method to use

| Platform | Recommended | Alternatives |
|---|---|---|
| macOS (Apple Silicon or Intel) | [Homebrew](#homebrew-macos-and-linux) | [Install script](#install-script-linux-and-macos), [manual download](#manual-download) |
| Linux (x86_64 or aarch64) | [Install script](#install-script-linux-and-macos) | [Homebrew](#homebrew-macos-and-linux), [manual download](#manual-download) |
| Windows (x86_64) | [WinGet](#winget-windows) | [Scoop](#scoop-windows), [manual download](#manual-download) |

Homebrew, WinGet and Scoop only carry stable releases. Pre-releases are published on the [Releases page](https://github.com/rqlang/rq/releases) only, so use the install script with `--release-tag` or a manual download to try one.

Whatever the method, check the result with:

```bash
rq --version
```

## CLI

### Homebrew (macOS and Linux)

The formula lives in the [rqlang/homebrew-tap](https://github.com/rqlang/homebrew-tap) tap and installs the binary for your platform and architecture:

```bash
brew install rqlang/tap/rqlang
```

The formula is called `rqlang`, but the command it installs is `rq`.

| Task | Command |
|---|---|
| Upgrade | `brew upgrade rqlang` |
| Uninstall | `brew uninstall rqlang` |

On Linux the same [system requirements](#linux-requirements) as the other methods apply.

### WinGet (Windows)

rq is published in the WinGet community repository as `rqlang.rq`:

```powershell
winget install --id rqlang.rq --exact
```

WinGet installs it as a portable package and puts `rq` on your `PATH`; open a new terminal before running it.

| Task | Command |
|---|---|
| Upgrade | `winget upgrade --id rqlang.rq --exact` |
| Uninstall | `winget uninstall --id rqlang.rq --exact` |

New versions reach WinGet through a pull request to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs), so they can appear there a few days after the GitHub release.

If you installed `rq` earlier with the former PowerShell install script, it lives in `%LOCALAPPDATA%\rq`. Delete that folder and remove it from your user `PATH` (or run `deployment\uninstall-rq.ps1` from a clone of this repository) before switching to WinGet or Scoop, so that only one `rq.exe` is on the `PATH`.

### Scoop (Windows)

The manifest lives in the [rqlang/scoop-bucket](https://github.com/rqlang/scoop-bucket) bucket. Add the bucket once, then install:

```powershell
scoop bucket add rqlang https://github.com/rqlang/scoop-bucket
scoop install rqlang/rqlang
```

The manifest is called `rqlang`, but the command it installs is `rq`.

| Task | Command |
|---|---|
| Upgrade | `scoop update rqlang` |
| Uninstall | `scoop uninstall rqlang` |

### Install script (Linux and macOS)

The install script downloads the CLI binary for your platform and architecture (`x86_64` or `aarch64`/Apple Silicon) from the latest GitHub release, checks it against the release's `SHA256SUMS`, renames it to `rq` and copies it to a directory on your `PATH`:

```bash
curl -fsSL https://raw.githubusercontent.com/rqlang/rq/main/deployment/install-rq.sh | bash
```

It needs `curl` and `sha256sum` (or `shasum`). Releases published before `SHA256SUMS` existed (0.7.0 and older) cannot be verified, so the script refuses to install them; download those [manually](#manual-download).

The script runs the downloaded binary before installing it, so on a system that does not meet the [requirements](#linux-requirements) it stops with an error and installs nothing.

Where it installs depends on who runs it:

- **As a regular user**: `~/.local/bin/rq`. No `sudo` needed, and only your user gets `rq`.
- **As root**: `/usr/local/bin/rq`, available to every user on the machine:

  ```bash
  curl -fsSL https://raw.githubusercontent.com/rqlang/rq/main/deployment/install-rq.sh | sudo bash
  ```

Most Linux distributions already put `~/.local/bin` on the `PATH`; macOS does not. If it is missing, the script says so and prints the line to add to your shell profile (`~/.bashrc`, `~/.zshrc`, …):

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

Running the script again installs over the existing binary, so the same command upgrades `rq`.

To uninstall, delete the binary: `rm ~/.local/bin/rq` (or `sudo rm /usr/local/bin/rq`).

### Manual download

Every release attaches a prebuilt, portable binary per platform. They need no installation step.

1. Open the [Releases page](https://github.com/rqlang/rq/releases) and pick a release.
2. Under **Assets**, download the file for your platform, together with `SHA256SUMS`:

   | Platform | Asset |
   |---|---|
   | Windows x86_64 | `rq-windows-x86_64.exe` |
   | macOS Apple Silicon | `rq-macos-aarch64` |
   | macOS Intel | `rq-macos-x86_64` |
   | Linux x86_64 | `rq-linux-x86_64` |
   | Linux aarch64 | `rq-linux-aarch64` |

   `rq-linux-ubuntu-22.04-x86_64` is the same file as `rq-linux-x86_64`, kept under its old name for existing download links.

3. Verify the download against `SHA256SUMS`:

   ```bash
   sha256sum --check --ignore-missing SHA256SUMS
   ```

   On macOS use `shasum -a 256 --check --ignore-missing SHA256SUMS`; on Windows compare the output of `Get-FileHash rq-windows-x86_64.exe` with the line for that file. With the [GitHub CLI](https://cli.github.com/) you can also check that the file was built by this repository's release workflow:

   ```bash
   gh attestation verify rq-linux-x86_64 --repo rqlang/rq
   ```

4. On Linux and macOS, make the file executable, rename it to `rq` and move it to a directory on your `PATH`:

   ```bash
   chmod +x rq-linux-x86_64
   mkdir -p ~/.local/bin
   mv rq-linux-x86_64 ~/.local/bin/rq
   rq --version
   ```

   If `rq` is not found, add `~/.local/bin` to your `PATH` as shown in the [install script](#install-script-linux-and-macos) section.

   The macOS binaries are not signed, so macOS blocks a file downloaded with a browser when it first runs. If `rq --version` is blocked, remove the quarantine flag and run it again:

   ```bash
   xattr -d com.apple.quarantine ~/.local/bin/rq
   ```

On Windows, rename `rq-windows-x86_64.exe` to `rq.exe` and move it to a folder on your `PATH` (or add its folder to the `PATH`). You can also run it directly from the download folder:

```powershell
cd $env:USERPROFILE\Downloads
.\rq-windows-x86_64.exe --version
```

Releases published before 0.8.0 do not include `SHA256SUMS` or the `rq-linux-aarch64` binary.

### Linux requirements

The Linux binaries, whichever way you install them, need glibc 2.34 or newer and OpenSSL 3 (`libssl.so.3`), which Ubuntu 22.04, Debian 12, RHEL 9 and later ship by default.

## VS Code extension

### VS Code Marketplace

The extension is published on the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=rq-lang.rq-language). Install it from the **Extensions** view by searching for `rq`, or from a terminal:

```bash
code --install-extension rq-lang.rq-language
```

### Manual install from VSIX

Each release also attaches the extension as a `.vsix` file, for machines without Marketplace access.

1. Open the [Releases page](https://github.com/rqlang/rq/releases) and pick a release.
2. Under **Assets**, download the `.vsix` file (for example `rq-language-0.8.0.vsix`).
3. Install it from a terminal:

   ```bash
   code --install-extension rq-language-0.8.0.vsix
   ```

   Or, in VS Code, open the **Extensions** view (`Ctrl+Shift+X` / `Cmd+Shift+X`), click the **`...`** menu at the top of the panel and choose **Install from VSIX...**.

   See [Install from a VSIX](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace#_install-from-a-vsix) in the VS Code documentation for details.
