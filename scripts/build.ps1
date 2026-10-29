param([string]$Target = '')
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    # Remove workstation identity from paths embedded in panic/dependency metadata.
    $previousFlags = $env:RUSTFLAGS
    $env:RUSTFLAGS = "$previousFlags --remap-path-prefix=$env:USERPROFILE=/user"
    if ($Target) { & cargo build --release --locked --target $Target } else { & cargo build --release --locked }
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
} finally { $env:RUSTFLAGS = $previousFlags; Pop-Location }
