param([string]$Binary = 'target/release/cmtga.exe')
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    New-Item -ItemType Directory -Force test-output | Out-Null
    & $Binary doctor
    if ($LASTEXITCODE -ne 0) { throw 'doctor failed' }
    & $Binary validate --request examples/challenge-rules.json
    if ($LASTEXITCODE -ne 0) { throw 'validation failed' }
    foreach ($mode in @('software','auto')) {
        & $Binary text --text 'NATIVE TEST' --duration 1 --preset impact --output "test-output/smoke-$mode.mp4" --overwrite --acceleration $mode
        if ($LASTEXITCODE -ne 0) { throw "Native $mode render failed" }
    }
    & $Binary render --request examples/challenge-rules.json --output test-output/challenge-rules.mp4 --overwrite
    if ($LASTEXITCODE -ne 0) { throw 'Demo render failed' }
    foreach ($time in @(0.15,1,3,6,9,12.5,15)) {
        & $Binary frame --request examples/challenge-rules.json --time $time --output "test-output/frame-$time.png" --overwrite
        if ($LASTEXITCODE -ne 0) { throw 'Frame verification failed' }
    }
} finally { Pop-Location }
