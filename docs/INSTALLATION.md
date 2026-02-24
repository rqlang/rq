# Installation

rq is a domain-specific language designed for managing and executing HTTP requests.

This document describes how to install the rq VS Code extension.

## VS Code extension

The rq VS Code extension is distributed through the VS Code Marketplace (extension link coming soon).

The extension uses the rq CLI as its backend. When the extension starts it will:

- Check whether the `rq` CLI is available on your system.
- Compare the installed CLI version with the extension version.
- If the CLI is missing or outdated, offer to install it from scratch or update it.

The installation and update of the CLI are handled automatically by the extension using platform-specific scripts:

- On Linux and macOS it runs the Bash installer script `deployment/install-rq.sh`.
- On Windows it runs the PowerShell installer script `deployment/install-rq.ps1`.

The install behaviour depends on the platform:

The binary is installed to a **local directory**:
- **Linux / macOS**: `~/.local/bin`
- **Windows**: `%LOCALAPPDATA%\rq`

The extension will use the binary directly from this location.

On **Ubuntu Desktop** (and many other modern Linux distributions), `~/.local/bin` is typically already in your `PATH`. This means you can run the `rq` command from your terminal immediately after installation (you may need to open a new terminal window).

On **macOS**, **Windows**, and other systems where `~/.local/bin` is not in the `PATH`, you will need to add the installation directory to your `PATH` manually if you wish to use the CLI outside of VS Code.

## CLI (Manual Download)

If you prefer not to rely on the extension to install the CLI, or if you want to use the CLI independently, you can download the prebuilt **portable** binary directly from the GitHub Releases page. The binaries are self-contained and do not require any additional runtime or installation steps.

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

