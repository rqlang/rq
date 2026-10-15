if ($Env:FAKE_GH_LOG) {
    Add-Content -Path $Env:FAKE_GH_LOG -Value ($args -join ' ')
}

if ($args[0] -eq 'api') {
    if ($Env:FAKE_GH_API_ERROR) {
        [Console]::Error.WriteLine($Env:FAKE_GH_API_ERROR)
        exit 1
    }
    $runIdFile = Join-Path $Env:FAKE_ARTIFACTS 'run_id'
    if (Test-Path $runIdFile) {
        (Get-Content -Path $runIdFile -Raw).Trim()
    }
    exit 0
}

if ($args[0] -eq 'run' -and $args[1] -eq 'download') {
    $name = $args[[array]::IndexOf($args, '--name') + 1]
    $dir = $args[[array]::IndexOf($args, '--dir') + 1]
    $source = Join-Path $Env:FAKE_ARTIFACTS $name
    if (-not (Test-Path $source)) {
        [Console]::Error.WriteLine('no artifact matches any of the names or patterns provided')
        exit 1
    }
    Copy-Item -Path (Join-Path $source '*') -Destination $dir -Recurse
    exit 0
}

[Console]::Error.WriteLine("fake gh: unexpected command $($args -join ' ')")
exit 2
