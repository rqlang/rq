Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Installer = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\install-rq-dev.ps1'))
$Pwsh = (Get-Process -Id $PID).Path
$Csc = Join-Path $Env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$Scratch = Join-Path ([IO.Path]::GetTempPath()) ("rq-dev-test-" + [guid]::NewGuid().ToString('N'))
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

function Reset-Fixture {
    if (Test-Path $Scratch) { Remove-Item -Path $Scratch -Recurse -Force }
    foreach ($dir in 'bin', 'artifacts', 'install', 'exes') {
        New-Item -ItemType Directory -Path (Join-Path $Scratch $dir) | Out-Null
    }
    Copy-Item -Path (Join-Path $PSScriptRoot 'fake-gh.ps1') -Destination (Join-Path $Scratch 'bin\gh.ps1')
    Set-Content -Path (Join-Path $Scratch 'artifacts\run_id') -Value '42'
    Set-RawUserPath $OriginalUserPath
}

function New-FakeExe([string]$Path, [string]$Output, [int]$ExitCode) {
    $source = Join-Path $Scratch 'exes\fake.cs'
    Set-Content -Path $source -Value "class P { static int Main() { System.Console.WriteLine(`"$Output`"); return $ExitCode; } }"
    & $Csc /nologo /out:$Path $source | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "csc failed to build $Path" }
}

function Add-Artifact([string]$Output, [int]$ExitCode = 0) {
    $dir = Join-Path $Scratch 'artifacts\rq-windows-x86_64.exe'
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    New-FakeExe (Join-Path $dir 'rq.exe') $Output $ExitCode
}

function Use-InstallerEnvironment([scriptblock]$Action, [hashtable]$Environment = @{}, [string]$PathOverride) {
    $saved = @{}
    $vars = @{ FAKE_ARTIFACTS = (Join-Path $Scratch 'artifacts') } + $Environment
    $vars['Path'] = if ($PathOverride) { $PathOverride } else { "$(Join-Path $Scratch 'bin');$Env:Path" }
    foreach ($key in $vars.Keys) {
        $saved[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
        [Environment]::SetEnvironmentVariable($key, $vars[$key], 'Process')
    }
    try {
        & $Action
    } finally {
        foreach ($key in $saved.Keys) {
            [Environment]::SetEnvironmentVariable($key, $saved[$key], 'Process')
        }
    }
}

function Invoke-Installer([string[]]$InstallerArgs, [hashtable]$Environment = @{}, [string]$PathOverride) {
    Use-InstallerEnvironment {
        $previousPreference = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        $script:Output = & $Pwsh -NoProfile -ExecutionPolicy Bypass -File $Installer @InstallerArgs 2>&1 | Out-String
        $script:ExitCode = $LASTEXITCODE
        $ErrorActionPreference = $previousPreference
    } $Environment $PathOverride
    return $script:ExitCode -eq 0
}

function Get-InstalledVersion([string]$Name) {
    $exe = Join-Path $Scratch "install\$Name.exe"
    if (-not (Test-Path -LiteralPath $exe)) { return '' }
    return (& $exe | Out-String).Trim()
}

function Get-InstallEntries {
    return @(Get-ChildItem -Path (Join-Path $Scratch 'install') -Force -Name)
}

$InstallDirArgs = @('-InstallDir', (Join-Path $Scratch 'install'))

function Test-InstallsAsRqDevByDefault {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    if ((Invoke-Installer $InstallDirArgs) -and (Get-InstalledVersion 'rq-dev') -eq 'rq 0.9.0-dev.3' -and ((Get-InstallEntries) -join ',') -eq 'rq-dev.exe') {
        Pass 'installs as rq-dev.exe by default'
    } else { Fail 'installs as rq-dev.exe by default' }
}

function Test-InstallsUnderCustomName {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    if ((Invoke-Installer ($InstallDirArgs + @('-Name', 'rq'))) -and (Get-InstalledVersion 'rq') -eq 'rq 0.9.0-dev.3' -and -not (Test-Path (Join-Path $Scratch 'install\rq-dev.exe'))) {
        Pass 'installs under the name given by -Name'
    } else { Fail 'installs under the name given by -Name' }
}

function Test-RejectsNameWithSeparator {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    if (-not (Invoke-Installer ($InstallDirArgs + @('-Name', '..\rq'))) -and $script:Output -match "Invalid executable name" -and @(Get-InstallEntries).Count -eq 0) {
        Pass 'rejects a name that contains a path separator'
    } else { Fail 'rejects a name that contains a path separator' }
}

function Test-InstallsUnderNameWithBrackets {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    if ((Invoke-Installer ($InstallDirArgs + @('-Name', 'rq[dev]'))) -and (Get-InstalledVersion 'rq[dev]') -eq 'rq 0.9.0-dev.3' -and ((Get-InstallEntries) -join ',') -eq 'rq[dev].exe') {
        Pass 'installs under a name that contains brackets'
    } else { Fail 'installs under a name that contains brackets' }
}

function Test-RejectsNameWithWildcard {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    if (-not (Invoke-Installer ($InstallDirArgs + @('-Name', 'rq*'))) -and $script:Output -match "Invalid executable name" -and @(Get-InstallEntries).Count -eq 0) {
        Pass 'rejects a name that is not a valid file name'
    } else { Fail 'rejects a name that is not a valid file name' }
}

function Test-ResolvesRelativeInstallDirAgainstLocation {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    $location = Join-Path $Scratch 'location'
    New-Item -ItemType Directory -Path $location | Out-Null
    $previousDirectory = [Environment]::CurrentDirectory
    [Environment]::CurrentDirectory = $Scratch
    Push-Location $location
    try {
        Use-InstallerEnvironment { & $Installer -InstallDir 'tools' *> $null }
    } catch {
    } finally {
        Pop-Location
        [Environment]::CurrentDirectory = $previousDirectory
    }
    if ((Test-Path (Join-Path $location 'tools\rq-dev.exe')) -and -not (Test-Path (Join-Path $Scratch 'tools'))) {
        Pass 'resolves a relative install directory against the current location'
    } else { Fail 'resolves a relative install directory against the current location' }
}

function Test-KeepsVariablesInUserPath {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    Set-RawUserPath @{ Value = '%SystemRoot%\rq-test'; Kind = [Microsoft.Win32.RegistryValueKind]::ExpandString }
    $ok = Invoke-Installer $InstallDirArgs
    $raw = Get-RawUserPath
    if ($ok -and $raw.Value -eq "%SystemRoot%\rq-test;$(Join-Path $Scratch 'install')" -and $raw.Kind -eq [Microsoft.Win32.RegistryValueKind]::ExpandString) {
        Pass 'keeps environment variable references in the user PATH'
    } else { Fail 'keeps environment variable references in the user PATH' }
}

function Test-AddsInstallDirToUserPath {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    $installDir = Join-Path $Scratch 'install'
    $ok = Invoke-Installer $InstallDirArgs
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($ok -and ($userPath.Split(';') -contains $installDir)) {
        Pass 'adds the install directory to the user PATH'
    } else { Fail 'adds the install directory to the user PATH' }
}

function Test-DoesNotDuplicateUserPathEntry {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    $installDir = Join-Path $Scratch 'install'
    $null = Invoke-Installer $InstallDirArgs
    $ok = Invoke-Installer $InstallDirArgs
    $entries = @([Environment]::GetEnvironmentVariable('Path', 'User').Split(';') | Where-Object { $_ -eq $installDir })
    if ($ok -and $entries.Count -eq 1) {
        Pass 'does not add the install directory to the user PATH twice'
    } else { Fail 'does not add the install directory to the user PATH twice' }
}

function Test-FailsWhenGhMissing {
    Reset-Fixture
    $systemPath = Join-Path $Env:WINDIR 'System32'
    if (-not (Invoke-Installer $InstallDirArgs @{} $systemPath) -and $script:Output -match 'GitHub CLI \(gh\) not found in PATH' -and @(Get-InstallEntries).Count -eq 0) {
        Pass 'fails when gh is missing'
    } else { Fail 'fails when gh is missing' }
}

function Test-ShowsGhApiError {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    if (-not (Invoke-Installer $InstallDirArgs @{ FAKE_GH_API_ERROR = 'HTTP 401: Bad credentials' }) -and $script:Output -match 'HTTP 401: Bad credentials' -and $script:Output -match 'Failed to query the runs of release_cd.yaml' -and @(Get-InstallEntries).Count -eq 0) {
        Pass 'shows the gh error when the runs query fails'
    } else { Fail 'shows the gh error when the runs query fails' }
}

function Test-FailsWhenNoSuccessfulRun {
    Reset-Fixture
    Remove-Item -Path (Join-Path $Scratch 'artifacts\run_id')
    if (-not (Invoke-Installer $InstallDirArgs) -and $script:Output -match 'No successful runs of release_cd.yaml' -and @(Get-InstallEntries).Count -eq 0) {
        Pass 'fails when there is no successful run'
    } else { Fail 'fails when there is no successful run' }
}

function Test-DownloadsFromLatestRun {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    $log = Join-Path $Scratch 'gh.log'
    if ((Invoke-Installer $InstallDirArgs @{ FAKE_GH_LOG = $log }) -and ((Get-Content -Path $log) -match '^run download 42 --name rq-windows-x86_64.exe ')) {
        Pass 'downloads the artifact from the latest successful run'
    } else { Fail 'downloads the artifact from the latest successful run' }
}

function Test-FailsWhenArtifactMissing {
    Reset-Fixture
    if (-not (Invoke-Installer $InstallDirArgs) -and $script:Output -match "Failed to download artifact 'rq-windows-x86_64.exe' from run 42" -and @(Get-InstallEntries).Count -eq 0) {
        Pass 'fails when the artifact is missing'
    } else { Fail 'fails when the artifact is missing' }
}

function Test-FailsWhenBinaryDoesNotRun {
    Reset-Fixture
    Add-Artifact 'error while loading' 127
    if (-not (Invoke-Installer $InstallDirArgs) -and $script:Output -match 'does not run on this system' -and @(Get-InstallEntries).Count -eq 0) {
        Pass 'fails without leaving files when the downloaded binary does not run'
    } else { Fail 'fails without leaving files when the downloaded binary does not run' }
}

function Test-ReplacesExistingBinary {
    Reset-Fixture
    Add-Artifact 'rq 0.9.0-dev.3'
    New-FakeExe (Join-Path $Scratch 'install\rq-dev.exe') 'rq 0.9.0-dev.1' 0
    if ((Invoke-Installer $InstallDirArgs) -and (Get-InstalledVersion 'rq-dev') -eq 'rq 0.9.0-dev.3' -and ((Get-InstallEntries) -join ',') -eq 'rq-dev.exe') {
        Pass 'replaces an existing rq-dev.exe without leaving staged files'
    } else { Fail 'replaces an existing rq-dev.exe without leaving staged files' }
}

try {
    Test-InstallsAsRqDevByDefault
    Test-InstallsUnderCustomName
    Test-RejectsNameWithSeparator
    Test-InstallsUnderNameWithBrackets
    Test-RejectsNameWithWildcard
    Test-ResolvesRelativeInstallDirAgainstLocation
    Test-KeepsVariablesInUserPath
    Test-AddsInstallDirToUserPath
    Test-DoesNotDuplicateUserPathEntry
    Test-FailsWhenGhMissing
    Test-ShowsGhApiError
    Test-FailsWhenNoSuccessfulRun
    Test-DownloadsFromLatestRun
    Test-FailsWhenArtifactMissing
    Test-FailsWhenBinaryDoesNotRun
    Test-ReplacesExistingBinary
} finally {
    Set-RawUserPath $OriginalUserPath
    Remove-Item -Path $Scratch -Recurse -Force -ErrorAction SilentlyContinue
}

if ($script:Failed) { exit 1 }
