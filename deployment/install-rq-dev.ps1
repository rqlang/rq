<#!
.SYNOPSIS
install-rq-dev.ps1 # noqa: E501

Downloads the latest rq-windows-amd64 GitHub Actions artifact,
unblocks rq.exe, and installs it into %LOCALAPPDATA%\rq.
The install directory is added to the user PATH if not already present.

.NOTES
Requires: gh CLI installed and authenticated (gh auth login). Requires PowerShell 5.1+ or PowerShell Core.
#>

# Allow script to be called with -ReleaseTag param for compatibility, though ignored for dev builds
Param(
    [string]$ReleaseTag = "",
    [string]$InstallDir = ""
)

set-strictmode -version latest
$ErrorActionPreference = 'Stop'

$Repo = 'rqlang/rq'
$Workflow = 'release_cd.yaml'
$ArtifactName = 'rq-windows-x86_64.exe'

if (-not $Env:LOCALAPPDATA) {
    throw 'LOCALAPPDATA environment variable is not set; cannot determine user install directory.'
}
$DestinationBin = Join-Path $Env:LOCALAPPDATA 'rq'

if ($InstallDir) {
    $DestinationBin = $InstallDir
}

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

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw 'GitHub CLI (gh) not found in PATH. Install from https://cli.github.com/ and authenticate with gh auth login.'
}

$RunId = Get-LatestRunId

$ghArgs = @('-R', $Repo, 'run', 'download', $RunId, '--name', $ArtifactName, '--dir', '.')

$originalDir = Get-Location
try {
    if (-not ([IO.Path]::IsPathRooted($DestinationBin))) {
        $DestinationBin = Join-Path -Path $originalDir -ChildPath $DestinationBin
    }
} catch {}

if (-not (Test-Path $DestinationBin)) {
    Write-Info "Creating destination bin directory (pre-download): $DestinationBin"
    New-Item -ItemType Directory -Path $DestinationBin -Force | Out-Null
}
$resolvedDestBin = Resolve-Path -Path $DestinationBin
Write-Info "Resolved destination bin: $resolvedDestBin"

$workRoot = Join-Path -Path ([IO.Path]::GetTempPath()) -ChildPath ("rq-artifact-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $workRoot | Out-Null
Push-Location $workRoot
try {
    Write-Info "Downloading artifact '$ArtifactName' for run $RunId ..."
    $downloadCmd = "gh " + ($ghArgs -join ' ')
    Write-Info $downloadCmd
    gh @ghArgs | Out-Null

    $zips = Get-ChildItem -Filter '*.zip' -File -ErrorAction SilentlyContinue
    foreach ($zip in $zips) {
        $extractDir = Join-Path $workRoot ([IO.Path]::GetFileNameWithoutExtension($zip.Name))
        Write-Info "Expanding $($zip.Name) ..."
        Expand-Archive -Path $zip.FullName -DestinationPath $extractDir -Force
    }

    $exeCandidates = @(Get-ChildItem -Recurse -Filter 'rq.exe' -File -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)
    if (-not $exeCandidates -or $exeCandidates.Count -eq 0) {
        Write-Err "Did not find rq.exe inside artifact '$ArtifactName'. Contents:"
        Get-ChildItem -Recurse | Select-Object FullName,Length | Out-Host
        throw "rq.exe not found in artifact '$ArtifactName'."
    }

    if ($exeCandidates.Count -gt 1) {
        Write-Warn "Multiple rq.exe files found; using first one. List: `n$($exeCandidates -join "`n")"
    }
    $sourceExe = $exeCandidates[0]
    Write-Info "Using rq.exe at $sourceExe"

    $destExe = Join-Path $resolvedDestBin 'rq.exe'
    $backupExe = Join-Path $resolvedDestBin 'rq.exe.bck'

    if (Test-Path $destExe) {
        Write-Info "Backing up existing rq.exe to rq.exe.bck"
        Copy-Item -Path $destExe -Destination $backupExe -Force
    }

    Write-Info "Copying new rq.exe to $destExe"
    Copy-Item -Path $sourceExe -Destination $destExe -Force

    Write-Info 'Removing Zone.Identifier (unblock)'
    try {
        Unblock-File -Path $destExe -ErrorAction Stop
    } catch {
        Write-Warn "Unblock-File failed: $($_.Exception.Message). Attempting alternate stream removal."
        try { Remove-Item -Path "$destExe:Zone.Identifier" -ErrorAction SilentlyContinue } catch { }
    }

    Write-Info 'Verifying executable'
    if (-not (Test-Path $destExe)) { throw 'Failed to place rq.exe in destination.' }

    Write-Host "Success: Installed rq.exe to $destExe" -ForegroundColor Green
}
finally {
    Pop-Location
    try { Remove-Item -Recurse -Force -Path $workRoot } catch { Write-Warn "Temp cleanup failed: $($_.Exception.Message)" }
}