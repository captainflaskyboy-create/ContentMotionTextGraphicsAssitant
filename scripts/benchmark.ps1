param([string]$Binary = 'target/release/cmtga.exe', [string]$Request = 'examples/challenge-rules.json', [string]$Acceleration = 'auto')
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    New-Item -ItemType Directory -Force test-output | Out-Null
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $result = & $Binary render --request $Request --output test-output/benchmark.mp4 --overwrite --acceleration $Acceleration | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or -not $result.ok) { throw 'Benchmark render failed' }
    $clock.Stop()
    $result | Add-Member -NotePropertyName process_wall_seconds -NotePropertyValue $clock.Elapsed.TotalSeconds
    $result | ConvertTo-Json -Depth 12 | Set-Content test-output/benchmark.json -Encoding utf8
    $result | Select-Object frames,file_bytes,render_seconds,total_seconds,process_wall_seconds,render_fps,peak_working_set_bytes
} finally { Pop-Location }
