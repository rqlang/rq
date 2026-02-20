<#!
.SYNOPSIS
install-rq-dev-extension.ps1 # noqa: E501

Downloads the rq-language-extension VSIX artifact from GitHub Actions and installs it into VS Code.

.DESCRIPTION
Given a GitHub Actions workflow run ID (or auto-detecting the latest), this script uses the GitHub CLI (gh) to download the artifact matching rq-language-extension-<version>.vsix pattern. It then installs the VSIX into VS Code using the 'code --install-extension' command.

.PARAMETER RunId
The numeric GitHub Actions workflow run ID to retrieve artifacts from. If omitted, the latest run ID will be auto-detected.

.PARAMETER Force
Switch. If provided, VS Code will reinstall the extension even if already installed (passes --force to code).

.EXAMPLE
PS> ./update-rq-extension.ps1
Downloads the VSIX from the latest 'Release CD' workflow run and installs it.

.EXAMPLE
PS> ./update-rq-extension.ps1 -RunId 123456789
Explicit run ID for a specific build.

.EXAMPLE
PS> ./update-rq-extension.ps1 -Force
Download latest and force reinstall even if already present.

.NOTES
Requires: 
- gh CLI installed and authenticated (gh auth login)
- VS Code CLI (code) available in PATH
- PowerShell 5.1+ or PowerShell Core
#>
[CmdletBinding(SupportsShouldProcess=$true)]
param(
    [Nullable[long]]$RunId,

    [switch]$Force
)

set-strictmode -version latest
$ErrorActionPreference = 'Stop'

$Repo = 'rqlang/rq'
$Workflow = 'release_cd.yaml'
$ArtifactPattern = 'rq-language-extension-*'

function Write-Info($msg){ Write-Host "[INFO ] $msg" -ForegroundColor Cyan }
function Write-Warn($msg){ Write-Warning $msg }
function Write-Err ($msg){ Write-Host "[ERROR] $msg" -ForegroundColor Red }

function Get-LatestRunId {
    Write-Info "Auto-detecting latest successful workflow run (repo=$Repo workflow=$Workflow)"
    
    $endpoint = "repos/$Repo/actions/workflows/$Workflow/runs?status=success&per_page=1"
    
    $latestId = $null
    try {
        $latestId = gh api $endpoint --jq '.workflow_runs[0].id' 2>$null
    } catch {
        Write-Warn "Primary gh api query failed: $($_.Exception.Message)" 
    }
    if (-not $latestId) {
        $jsonRaw = gh api $endpoint 2>$null
        if ($jsonRaw) {
            try {
                $obj = $jsonRaw | ConvertFrom-Json -Depth 6
                $latestId = $obj.workflow_runs[0].id
            } catch { Write-Warn "Fallback JSON parse failed: $($_.Exception.Message)" }
        }
    }
    if (-not $latestId) { throw "No successful workflow runs found for repo '$Repo' (workflow: $Workflow)" }
    Write-Info "Using latest run id: $latestId"
    return [long]$latestId
}

# Verify gh CLI
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw 'GitHub CLI (gh) not found in PATH. Install from https://cli.github.com/ and authenticate with gh auth login.'
}

# Verify code CLI
if (-not (Get-Command code -ErrorAction SilentlyContinue)) {
    throw 'VS Code CLI (code) not found in PATH. Ensure VS Code is installed and added to PATH.'
}

# Determine RunId if not supplied
if (-not $RunId) {
    $RunId = Get-LatestRunId
}

# List available artifacts to find matching pattern
Write-Info "Fetching artifacts list for run $RunId ..."
$artifactsJson = gh api "repos/$Repo/actions/runs/$RunId/artifacts" 2>$null
if (-not $artifactsJson) {
    throw "Failed to fetch artifacts for run $RunId in repo $Repo"
}

$artifactsObj = $artifactsJson | ConvertFrom-Json -Depth 6
$matchingArtifacts = @($artifactsObj.artifacts | Where-Object { $_.name -like $ArtifactPattern })

if (-not $matchingArtifacts -or $matchingArtifacts.Count -eq 0) {
    Write-Err "No artifacts matching pattern '$ArtifactPattern' found in run $RunId. Available artifacts:"
    $artifactsObj.artifacts | ForEach-Object { Write-Host "  - $($_.name) ($($_.size_in_bytes) bytes)" }
    throw "No matching artifact found."
}

if ($matchingArtifacts.Count -gt 1) {
    Write-Warn "Multiple artifacts match pattern '$ArtifactPattern'. Using first one:"
    $matchingArtifacts | ForEach-Object { Write-Host "  - $($_.name)" }
}

$artifactName = $matchingArtifacts[0].name
Write-Info "Selected artifact: $artifactName"

# Download artifact
$workRoot = Join-Path -Path ([IO.Path]::GetTempPath()) -ChildPath ("rq-extension-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $workRoot | Out-Null
Push-Location $workRoot
try {
    Write-Info "Downloading artifact '$artifactName' for run $RunId ..."
    $ghArgs = @('run', 'download', $RunId, '--name', $artifactName, '--dir', '.', '-R', $Repo)
    $downloadCmd = "gh " + ($ghArgs -join ' ')
    Write-Info $downloadCmd
    gh @ghArgs | Out-Null

    # After download, artifact may be a zip - expand if present
    $zips = Get-ChildItem -Filter '*.zip' -File -ErrorAction SilentlyContinue
    foreach ($zip in $zips) {
        $extractDir = Join-Path $workRoot ([IO.Path]::GetFileNameWithoutExtension($zip.Name))
        Write-Info "Expanding $($zip.Name) ..."
        Expand-Archive -Path $zip.FullName -DestinationPath $extractDir -Force
    }

    # Find .vsix file(s)
    $vsixCandidates = @(Get-ChildItem -Recurse -Filter '*.vsix' -File -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)
    if (-not $vsixCandidates -or $vsixCandidates.Count -eq 0) {
        Write-Err "Did not find .vsix file inside artifact '$artifactName'. Contents:"
        Get-ChildItem -Recurse | Select-Object FullName,Length | Out-Host
        throw ".vsix not found in artifact '$artifactName'."
    }

    if ($vsixCandidates.Count -gt 1) {
        Write-Warn "Multiple .vsix files found; using first one. List: `n$($vsixCandidates -join "`n")"
    }
    $vsixPath = $vsixCandidates[0]
    Write-Info "Using VSIX at $vsixPath"

    # Install VSIX
    $codeArgs = @('--install-extension', $vsixPath)
    if ($Force) {
        $codeArgs += '--force'
        Write-Info "Installing extension (force mode) ..."
    } else {
        Write-Info "Installing extension ..."
    }
    
    $installCmd = "code " + ($codeArgs -join ' ')
    Write-Info $installCmd
    
    & code @codeArgs
    if ($LASTEXITCODE -ne 0) {
        throw "VS Code extension installation failed with exit code $LASTEXITCODE"
    }

    Write-Host "Success: Installed extension from $vsixPath" -ForegroundColor Green
}
finally {
    Pop-Location
    # Cleanup temp dir
    try { Remove-Item -Recurse -Force -Path $workRoot } catch { Write-Warn "Temp cleanup failed: $($_.Exception.Message)" }
}
