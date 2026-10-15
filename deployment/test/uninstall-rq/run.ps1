Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Uninstaller = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\uninstall-rq.ps1'))
$Pwsh = (Get-Process -Id $PID).Path
$Scratch = Join-Path ([IO.Path]::GetTempPath()) ("rq-uninstall-test-" + [guid]::NewGuid().ToString('N'))
$script:Failed = $false

function Pass($name) { Write-Host "ok   $name" }
function Fail($name) { Write-Host "FAIL $name"; $script:Failed = $true }

function Get-RawUserPath {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment')
    try {
        if ($key.GetValueNames() -notcontains 'Path') { return $null }
        return @{
            Value = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            Kind = $key.GetValueKind('Path')
        }
    } finally {
        $key.Dispose()
    }
}

function Set-RawUserPath($RawPath) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
    try {
        if ($null -eq $RawPath) {
            $key.DeleteValue('Path', $false)
        } else {
            $key.SetValue('Path', $RawPath.Value, $RawPath.Kind)
        }
    } finally {
        $key.Dispose()
    }
}

$OriginalUserPath = Get-RawUserPath
$InstallDir = Join-Path $Scratch 'install'

function Reset-Fixture {
    if (Test-Path $Scratch) { Remove-Item -Path $Scratch -Recurse -Force }
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
    Set-Content -Path (Join-Path $InstallDir 'rq.exe') -Value 'fake'
    Set-RawUserPath $OriginalUserPath
}

function Invoke-Uninstaller {
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $script:Output = & $Pwsh -NoProfile -ExecutionPolicy Bypass -File $Uninstaller -InstallDir $InstallDir 2>&1 | Out-String
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $previousPreference
    return $exitCode -eq 0
}

function Test-RemovesBinaryAndEmptyDirectory {
    Reset-Fixture
    if ((Invoke-Uninstaller) -and -not (Test-Path $InstallDir)) {
        Pass 'removes rq.exe and the empty install directory'
    } else { Fail 'removes rq.exe and the empty install directory' }
}

function Test-RemovesInstallDirAndKeepsVariablesInUserPath {
    Reset-Fixture
    Set-RawUserPath @{ Value = "%SystemRoot%\rq-test;$InstallDir;%USERPROFILE%\bin"; Kind = [Microsoft.Win32.RegistryValueKind]::ExpandString }
    $ok = Invoke-Uninstaller
    $raw = Get-RawUserPath
    if ($ok -and $raw.Value -eq '%SystemRoot%\rq-test;%USERPROFILE%\bin' -and $raw.Kind -eq [Microsoft.Win32.RegistryValueKind]::ExpandString) {
        Pass 'removes the install directory from the user PATH and keeps variable references'
    } else { Fail 'removes the install directory from the user PATH and keeps variable references' }
}

function Test-RemovesInstallDirWrittenWithVariable {
    Reset-Fixture
    $Env:RQ_UNINSTALL_TEST_ROOT = $Scratch
    Set-RawUserPath @{ Value = '%SystemRoot%\rq-test;%RQ_UNINSTALL_TEST_ROOT%\install'; Kind = [Microsoft.Win32.RegistryValueKind]::ExpandString }
    try {
        $ok = Invoke-Uninstaller
    } finally {
        Remove-Item Env:RQ_UNINSTALL_TEST_ROOT
    }
    $raw = Get-RawUserPath
    if ($ok -and $raw.Value -eq '%SystemRoot%\rq-test') {
        Pass 'removes an install directory entry written with a variable reference'
    } else { Fail 'removes an install directory entry written with a variable reference' }
}

try {
    Test-RemovesBinaryAndEmptyDirectory
    Test-RemovesInstallDirAndKeepsVariablesInUserPath
    Test-RemovesInstallDirWrittenWithVariable
} finally {
    Set-RawUserPath $OriginalUserPath
    Remove-Item -Path $Scratch -Recurse -Force -ErrorAction SilentlyContinue
}

if ($script:Failed) { exit 1 }
