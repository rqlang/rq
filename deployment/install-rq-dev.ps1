<#!
.SYNOPSIS
Installs the rq CLI from the latest successful dev build on GitHub Actions.

.DESCRIPTION
Downloads the rq-windows-x86_64.exe artifact of the latest successful
release_cd.yaml run and installs it as rq-dev.exe into
"$Env:LOCALAPPDATA\rq-dev", next to the released rq without replacing it.
The install directory is added to the user PATH if not already present.

.PARAMETER Name
Name of the installed executable, without extension. Defaults to rq-dev.

.PARAMETER InstallDir
Directory to install into. Defaults to "$Env:LOCALAPPDATA\rq-dev".

.NOTES
Requires gh CLI installed and authenticated (gh auth login), and PowerShell 5.1+ or PowerShell Core.
#>
[CmdletBinding()]
param(
    [Parameter()]
    [string]$Name = 'rq-dev',

    [Parameter()]
    [string]$InstallDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Repo = 'rqlang/rq'
$Workflow = 'release_cd.yaml'
$ArtifactName = 'rq-windows-x86_64.exe'

function Write-Info($msg) { Write-Host "[INFO ] $msg" -ForegroundColor Cyan }

function Get-LatestRunId {
    $endpoint = "repos/$Repo/actions/workflows/$Workflow/runs?status=success&per_page=1"
    $runId = gh api $endpoint --jq '.workflow_runs[0].id // empty'
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to query the runs of $Workflow in $Repo. Check that gh is authenticated (gh auth status)."
    }
    if (-not $runId) {
        throw "No successful runs of $Workflow found in $Repo."
    }
    return "$runId".Trim()
}

function Get-VersionOutput([string]$Executable) {
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & $Executable --version 2>&1 | Out-String
        $exitCode = $LASTEXITCODE
    } catch {
        $output = $_.Exception.Message
        $exitCode = 1
    } finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($exitCode -ne 0) {
        throw "The downloaded '$ArtifactName' does not run on this system, so nothing was installed. Output: $output"
    }
    return $output.Trim()
}

function Send-EnvironmentChange {
    if (-not ('RqDevInstaller.NativeMethods' -as [type])) {
        Add-Type -Namespace RqDevInstaller -Name NativeMethods -MemberDefinition @'
[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
'@
    }
    $result = [UIntPtr]::Zero
    [RqDevInstaller.NativeMethods]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result) | Out-Null
}

function Add-DirectoryToUserPath([string]$Directory) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
    try {
        $userPath = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $parts = @($userPath.Split(';') | Where-Object { $_ })
        $present = $parts | Where-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -ieq $Directory.TrimEnd('\') }
        if ($present) {
            return
        }
        $key.SetValue('Path', (($parts + $Directory) -join ';'), [Microsoft.Win32.RegistryValueKind]::ExpandString)
    } finally {
        $key.Dispose()
    }
    Send-EnvironmentChange
    $Env:Path = "$Env:Path;$Directory"
    Write-Info "Added '$Directory' to the user PATH. Open a new terminal to pick it up."
}

if (-not $Name -or $Name.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0) {
    throw "Invalid executable name '$Name'."
}

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw 'GitHub CLI (gh) not found in PATH. Install it from https://cli.github.com/ and run gh auth login.'
}

if (-not $InstallDir) {
    if (-not $Env:LOCALAPPDATA) {
        throw 'LOCALAPPDATA environment variable is not set; cannot determine the install directory.'
    }
    $InstallDir = Join-Path $Env:LOCALAPPDATA 'rq-dev'
}
$InstallDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallDir)
if (-not (Test-Path -LiteralPath $InstallDir)) {
    Write-Info "Creating install directory: $InstallDir"
    [IO.Directory]::CreateDirectory($InstallDir) | Out-Null
}
$installExe = Join-Path $InstallDir "$Name.exe"

$runId = Get-LatestRunId
Write-Info "Using run $runId of $Workflow."

$workDir = Join-Path ([IO.Path]::GetTempPath()) ("rq-dev-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $workDir | Out-Null
$stagedExe = Join-Path $InstallDir (".$Name." + [guid]::NewGuid().ToString('N') + '.exe')
try {
    Write-Info "Downloading artifact '$ArtifactName'."
    gh run download $runId --name $ArtifactName --dir $workDir -R $Repo
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to download artifact '$ArtifactName' from run $runId. It may have expired."
    }

    $downloadedExe = Join-Path $workDir 'rq.exe'
    if (-not (Test-Path -LiteralPath $downloadedExe)) {
        $contents = (Get-ChildItem -LiteralPath $workDir -Name) -join ', '
        throw "Artifact '$ArtifactName' has no rq.exe. Contents: $contents"
    }

    Copy-Item -LiteralPath $downloadedExe -Destination $stagedExe
    $versionOutput = Get-VersionOutput $stagedExe
    Write-Info "Downloaded binary runs: $versionOutput"

    Write-Info "Copying $ArtifactName to $installExe"
    Move-Item -LiteralPath $stagedExe -Destination $installExe -Force

    try {
        Unblock-File -LiteralPath $installExe -ErrorAction Stop
    } catch {
        Remove-Item -LiteralPath $installExe -Stream Zone.Identifier -ErrorAction SilentlyContinue
    }
}
finally {
    Remove-Item -LiteralPath $stagedExe -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $workDir -Recurse -Force -ErrorAction SilentlyContinue
}

Add-DirectoryToUserPath $InstallDir

Write-Host "Success: Installed $versionOutput to $installExe" -ForegroundColor Green
