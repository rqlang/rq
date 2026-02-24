<#!
.SYNOPSIS
Downloads a rq Windows release asset from GitHub and installs it.

.DESCRIPTION
Given an optional GitHub release tag name, this script:
- Queries the GitHub Releases API for repo.
- Locates the asset named `rq-windows-x86_64.exe` in the specified release.
- Downloads the asset into a temporary file and removes the Internet Zone block (MOTW).
- Copies `rq.exe` into "$Env:LOCALAPPDATA\rq" (creating the directory if needed).
- Removes the temporary download file.
- Adds that directory to the **user** PATH.

.PARAMETER ReleaseTag
Optional GitHub release tag name (e.g. v0.4.0). If omitted, the latest release is used.

.EXAMPLE
PS> ./install-rq.ps1 -ReleaseTag v0.4.0
Downloads rq-windows-x86_64.exe from the v0.4.0 release into the current directory, installs rq.exe into %LOCALAPPDATA%\\rq and updates PATH.

.NOTES
Requires PowerShell 5.1+ or PowerShell Core, and Internet access to api.github.com and GitHub releases.
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [Parameter()]
    [string]$ReleaseTag,

    [Parameter()]
    [string]$InstallDir
)

set-strictmode -version latest
$ErrorActionPreference = 'Stop'

function Write-Info($msg) { Write-Host "[INFO ] $msg" -ForegroundColor Cyan }
function Write-Warn($msg) { Write-Warning $msg }
function Write-Err($msg)  { Write-Host "[ERROR] $msg" -ForegroundColor Red }

# Use a temp file for the download to avoid leaving rq.exe in the current directory
$DownloadDirectory = [IO.Path]::GetTempPath()

$owner = 'rqlang'
$repo  = 'rq'
$assetName = 'rq-windows-x86_64.exe'

if ([string]::IsNullOrWhiteSpace($ReleaseTag)) {
    Write-Info "ReleaseTag not provided. Fetching latest release from GitHub ($owner/$repo)."
    $releaseUrl = "https://api.github.com/repos/$owner/$repo/releases/latest"
} else {
    Write-Info "Fetching release '$ReleaseTag' from GitHub ($owner/$repo)"
    $releaseUrl = "https://api.github.com/repos/$owner/$repo/releases/tags/$ReleaseTag"
}

$headers = @{ 'User-Agent' = 'rq-installer-script' }

$release = $null
try {
    $release = Invoke-RestMethod -Uri $releaseUrl -Headers $headers -Method Get
} catch {
    if ([string]::IsNullOrWhiteSpace($ReleaseTag) -and $_.Exception.Message -match '404') {
        Write-Warn "Latest release endpoint returned 404. Falling back to releases list (including pre-releases)."
        $fallbackUrl = "https://api.github.com/repos/$owner/$repo/releases?per_page=1"
        try {
            $releaseList = Invoke-RestMethod -Uri $fallbackUrl -Headers $headers -Method Get
        } catch {
            Write-Err ("Failed to retrieve releases list for {0}/{1}: {2}" -f $owner, $repo, $_.Exception.Message)
            throw
        }
        if ($releaseList -and $releaseList.Count -gt 0) {
            $release = $releaseList[0]
        } else {
            Write-Err ("No releases found for {0}/{1}." -f $owner, $repo)
            throw
        }
    } else {
        Write-Err "Failed to retrieve release '$ReleaseTag' from GitHub: $($_.Exception.Message)"
        throw
    }
}

if (-not $release) {
    throw "Release '$ReleaseTag' not found for $owner/$repo."
}

if (-not $ReleaseTag) {
    # Update ReleaseTag to the one actually used (latest)
    $ReleaseTag = $release.tag_name
}

Write-Info "Installing release '$ReleaseTag' from repository $owner/$repo."

$asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1
if (-not $asset) {
    $available = ($release.assets | Select-Object -ExpandProperty name -ErrorAction SilentlyContinue) -join ', '
    throw "Asset '$assetName' not found in release '$ReleaseTag'. Available assets: $available"
}

# Use GitHub API URL for asset download (requires Accept: application/octet-stream)
$assetApiUrl = $asset.url
if (-not $assetApiUrl) {
    throw "API url not present for asset '$assetName' in release '$ReleaseTag'."
}

Write-Info "Downloading asset '$assetName' via GitHub API asset endpoint"
$rqPath = Join-Path $DownloadDirectory ("rq-install-" + [guid]::NewGuid().ToString('N') + ".exe")

try {
    $downloadHeaders = @{
        'User-Agent' = 'rq-installer-script'
        'Accept'     = 'application/octet-stream'
    }
    # Download directly to rq.exe to avoid renaming a locked file
    Invoke-WebRequest -Uri $assetApiUrl -OutFile $rqPath -Headers $downloadHeaders
} catch {
    Write-Err "Failed to download asset '$assetName': $($_.Exception.Message)"
    throw
}

if (-not (Test-Path $rqPath)) {
    throw "Download appeared to succeed but file not found at $rqPath"
}

# Remove Internet zone block (MOTW)
Write-Info 'Removing Internet Zone block (unblocking file)'
try {
    Unblock-File -Path $rqPath -ErrorAction Stop
} catch {
    Write-Warn "Unblock-File failed: $($_.Exception.Message). Attempting to remove Zone.Identifier stream."
    try { Remove-Item -Path "$rqPath:Zone.Identifier" -ErrorAction SilentlyContinue } catch { }
}

# Install into a per-user directory under LOCALAPPDATA (no admin required)
# or into a custom directory if -InstallDir was provided.
if ($InstallDir) {
    $installRoot = $InstallDir
    Write-Info "Using custom install directory: $installRoot"
} else {
    if (-not $Env:LOCALAPPDATA) {
        throw 'LOCALAPPDATA environment variable is not set; cannot determine user install directory.'
    }
    $installRoot = Join-Path $Env:LOCALAPPDATA 'rq'
}
if (-not (Test-Path $installRoot)) {
    Write-Info "Creating install directory: $installRoot"
    New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
}

$installExe = Join-Path $installRoot 'rq.exe'
Write-Info "Copying rq.exe to $installExe"
Copy-Item -Path $rqPath -Destination $installExe -Force

if ($installExe -and (Test-Path $installExe)) {
    Write-Info "Installed rq.exe to $installExe"
    Write-Host "Success!" -ForegroundColor Green
} else {
    throw "Failed to install rq.exe to $installExe"
}

# Clean up temporary download file
if (Test-Path $rqPath) {
    Remove-Item -Path $rqPath -Force -ErrorAction SilentlyContinue
}
