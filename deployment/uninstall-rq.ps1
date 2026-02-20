<#!
.SYNOPSIS
Uninstalls the rq CLI from Windows.

.DESCRIPTION
Removes the rq.exe binary from the install directory and removes that
directory from the user PATH environment variable.

By default the install directory is "$Env:LOCALAPPDATA\rq". Use -InstallDir
to override if rq was installed to a custom location.

.PARAMETER InstallDir
Directory where rq.exe is installed. Defaults to "$Env:LOCALAPPDATA\rq".

.EXAMPLE
PS> ./uninstall-rq.ps1
Removes rq.exe from %LOCALAPPDATA%\rq and cleans the user PATH.

.EXAMPLE
PS> ./uninstall-rq.ps1 -InstallDir "C:\tools\rq"
Removes rq.exe from C:\tools\rq and cleans the user PATH.
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [Parameter()]
    [string]$InstallDir
)

set-strictmode -version latest
$ErrorActionPreference = 'Stop'

function Write-Info($msg) { Write-Host "[INFO ] $msg" -ForegroundColor Cyan }
function Write-Warn($msg) { Write-Warning $msg }
function Write-Err($msg)  { Write-Host "[ERROR] $msg" -ForegroundColor Red }

function Remove-DirectoryFromPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Directory
    )

    $Directory = [IO.Path]::GetFullPath($Directory)

    $userKey = 'HKCU:\Environment'
    try {
        $userPath = ''
        if (Test-Path $userKey) {
            $userPath = (Get-ItemProperty -Path $userKey -Name Path -ErrorAction SilentlyContinue).Path
        }
        if (-not $userPath) {
            Write-Info "User PATH is empty, nothing to remove."
            return
        }

        $parts = $userPath.Split(';') | Where-Object { $_ }
        $filtered = $parts | Where-Object { $_.TrimEnd('\') -ne $Directory.TrimEnd('\') }

        if ($filtered.Count -eq $parts.Count) {
            Write-Info "'$Directory' was not found in user PATH."
            return
        }

        $newPath = ($filtered -join ';')
        Set-ItemProperty -Path $userKey -Name Path -Value $newPath
        Write-Info "Removed '$Directory' from user PATH. Open a new terminal to see the change."
    }
    catch {
        Write-Warn "Failed to update user PATH: $($_.Exception.Message)"
    }
}

if (-not $InstallDir) {
    if (-not $Env:LOCALAPPDATA) {
        Write-Err 'LOCALAPPDATA environment variable is not set; cannot determine install directory.'
        exit 1
    }
    $InstallDir = Join-Path $Env:LOCALAPPDATA 'rq'
}

$InstallDir = [IO.Path]::GetFullPath($InstallDir)
$BinaryPath = Join-Path $InstallDir 'rq.exe'

Write-Info "Uninstalling rq from '$InstallDir'"

if (Test-Path $BinaryPath) {
    if ($PSCmdlet.ShouldProcess($BinaryPath, 'Remove rq.exe')) {
        Remove-Item -Path $BinaryPath -Force
        Write-Info "Removed '$BinaryPath'"
    }
} else {
    Write-Warn "rq.exe not found at '$BinaryPath'. Skipping file removal."
}

$remainingFiles = @()
if (Test-Path $InstallDir) {
    $remainingFiles = @(Get-ChildItem -Path $InstallDir -Force)
}

if ((Test-Path $InstallDir) -and $remainingFiles.Count -eq 0) {
    if ($PSCmdlet.ShouldProcess($InstallDir, 'Remove empty install directory')) {
        Remove-Item -Path $InstallDir -Force
        Write-Info "Removed empty directory '$InstallDir'"
    }
}

Remove-DirectoryFromPath -Directory $InstallDir

Write-Host "Success: rq has been uninstalled." -ForegroundColor Green
