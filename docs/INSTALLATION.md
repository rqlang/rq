# Installation

rq is a domain-specific language designed for managing and executing HTTP requests.

This document describes how to install both the rq VS Code extension and the rq CLI.

## VS Code extension

The rq VS Code extension is distributed through the VS Code Marketplace (extension link coming soon).

The extension uses the rq CLI as its backend. When the extension starts it will:

- Check whether the `rq` CLI is available on your system.
- Compare the installed CLI version with the extension version.
- If the CLI is missing or outdated, offer to install it from scratch or update it.

The installation and update of the CLI performed by the extension use the **same scripts** described in the CLI section below:

- On Linux and macOS it runs the Bash installer script `deployment/install-rq.sh`.
- On Windows it runs the PowerShell installer script `deployment/install-rq.ps1`.

The install behaviour depends on the platform:

On **Ubuntu Desktop** and **Windows** the installation is fully automatic — no prompts, no password needed. On Ubuntu the binary is installed to `~/.local/bin`, which is already in your PATH. On Windows it is installed to `%LOCALAPPDATA%\rq` and added to your user PATH automatically.

On **WSL**, **macOS**, and **other Linux** distributions the extension will ask you to choose between installing system-wide to `/usr/local/bin` (requires `sudo`) or locally to `~/.local/bin` (no password needed — you may need to add it to your PATH manually).

Your choice is saved in the VS Code setting `rq.cli.installOnPath` and can be changed later.

If you prefer, you can first install the CLI manually using the instructions in the next section, and then install and use the VS Code extension.

## CLI

### Linux & macOS (Bash)

On Linux and macOS, you can install the rq CLI by piping the installer script to `bash`. The script auto-detects your platform and downloads the correct binary:

| Platform | Asset | Compatibility |
|---|---|---|
| Ubuntu 22.04 x86_64 | `rq-linux-ubuntu-22.04-x86_64` | Any Linux x86_64 with glibc ≥ 2.35 (Ubuntu 22.04+, Debian 12+, Fedora 36+, etc.) |
| Ubuntu 24.04 / other Linux x86_64 | `rq-linux-x86_64` | Any Linux x86_64 with glibc ≥ 2.39 (Ubuntu 24.04+, Fedora 40+, etc.) |
| macOS Intel (x86_64) | `rq-macos-x86_64` | macOS x86_64 |
| macOS Apple Silicon (M1/M2) | `rq-macos-aarch64` | macOS arm64 |

Both Linux binaries are standard ELF binaries dynamically linked against glibc. They are **not** limited to Ubuntu — they should work on any Linux distribution that ships a compatible glibc version. The Ubuntu 22.04 build links against an older glibc and therefore has **broader compatibility**. Distributions using musl instead of glibc (e.g. Alpine Linux) are not supported.

> **Disclaimer:** The Linux binaries have only been tested on Ubuntu. While they should work on other glibc-based distributions, compatibility is not guaranteed.

**With sudo** (installs to `/usr/local/bin`, already in PATH):

```bash
curl -fsSL https://raw.githubusercontent.com/rqlang/rq/master/deployment/install-rq.sh | sudo bash
```

**Without sudo** (installs to `~/.local/bin`):

```bash
curl -fsSL https://raw.githubusercontent.com/rqlang/rq/master/deployment/install-rq.sh | bash
```

Since `~/.local/bin` may not be in your PATH, add the following line to your shell profile (`~/.bashrc`, `~/.zshrc`, or `~/.bash_profile`):

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Then reload your profile (`source ~/.bashrc`, `source ~/.zshrc`, etc.) or open a new terminal.

To install a specific release instead of the latest, add `--release-tag`:

```bash
curl -fsSL https://raw.githubusercontent.com/rqlang/rq/master/deployment/install-rq.sh | sudo bash -s -- --release-tag "v0.4.0"
```

What this does:

- Detects the OS (Linux or macOS) and architecture (x86_64 or arm64) to select the correct asset.
- On Linux, checks if the Ubuntu version is 22.04 to use the specific build.
- Calls the GitHub Releases API and finds the latest release (or the specific tag you passed with `--release-tag`).
- Downloads the binary via the GitHub API into the current directory as `rq`.
- If run with sudo, copies `rq` to `/usr/local/bin/rq` (no PATH changes needed).
- If run without sudo, copies `rq` to `~/.local/bin/rq`. You will need to add `~/.local/bin` to your PATH manually (see above).

After the script finishes, open a **new** terminal session (or run `source ~/.zshrc` / `source ~/.bashrc`) and verify:

```bash
rq --version
```

### Windows (PowerShell)

On Windows, you can install the rq CLI by running a single PowerShell command that downloads the installer script into the **current** directory, unblocks it, and runs it. By default it installs the **latest** release, but you can optionally specify a release tag with `-ReleaseTag`.

```powershell
curl -L "https://raw.githubusercontent.com/rqlang/rq/master/deployment/install-rq.ps1" -o install-rq.ps1; Unblock-File -Path .\install-rq.ps1; .\install-rq.ps1
```

To install a specific release instead of the latest, add the optional `-ReleaseTag` parameter (for example `v0.4.0`):

```powershell
curl -L "https://raw.githubusercontent.com/rqlang/rq/master/deployment/install-rq.ps1" -o install-rq.ps1; Unblock-File -Path .\install-rq.ps1; .\install-rq.ps1 -ReleaseTag "v0.4.0"
```

What this does (with or without `-ReleaseTag`):

- Calls the GitHub Releases API for the rq repository and finds the latest release (or the specific tag you passed with `-ReleaseTag`).
- Locates the asset named `rq-windows-x86_64.exe` in that release.
- Downloads it via the GitHub API into the current directory as `rq.exe`.
- Removes the Internet zone block from `rq.exe`.
- Copies `rq.exe` into `%LOCALAPPDATA%\rq`.
- Adds `%LOCALAPPDATA%\rq` to your **user** `PATH`.

After the script finishes, open a **new** PowerShell or `cmd.exe` session and you should be able to run:

```powershell
rq --version
```

### Direct download (portable binary)

If you prefer not to run an installer script, you can download the prebuilt **portable** binary directly from the GitHub Releases page. The binaries are self-contained and do not require any additional runtime or installation steps.

1. Open the Releases page in your browser:
	- https://github.com/rqlang/rq/releases
2. In the **Assets** section of the release, download the file that matches your platform:
	- **Windows**: `rq-windows-x86_64.exe`
	- **Linux (Ubuntu 22.04 x86_64)**: `rq-linux-ubuntu-22.04-x86_64`
	- **Linux (other x86_64)**: `rq-linux-x86_64`
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

