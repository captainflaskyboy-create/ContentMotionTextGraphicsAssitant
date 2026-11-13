param([string]$Target = '')
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    # Remove workstation identity from paths embedded in panic/dependency metadata.
    $previousFlags = $env:RUSTFLAGS
    $env:RUSTFLAGS = "$previousFlags --remap-path-prefix=$env:USERPROFILE=/user"
    $effectiveTarget = $Target
    if (-not $effectiveTarget) {
        $compilerDetails = & rustc -vV
        if ($LASTEXITCODE -ne 0) { throw 'Could not inspect Rust target' }
        $effectiveTarget = ($compilerDetails | Select-String '^host: ').Line.Substring(6)
    }
    # Windows system DLLs remain dynamic; embed the MSVC C runtime for portability.
    if ($effectiveTarget.EndsWith('-msvc')) { $env:RUSTFLAGS += ' -C target-feature=+crt-static' }
    if ($Target) { & cargo build --release --locked --target $Target } else { & cargo build --release --locked }
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
} finally { $env:RUSTFLAGS = $previousFlags; Pop-Location }
