param([string]$Binary = 'target/release/cmtga.exe')
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    if (-not (Test-Path -LiteralPath $Binary)) { throw 'Build cmtga.exe first' }
    $package = Join-Path $PWD 'dist/cmtga-windows-x64'
    New-Item -ItemType Directory -Force $package | Out-Null
    Copy-Item -LiteralPath $Binary -Destination (Join-Path $package 'cmtga.exe')
    foreach ($item in @('README.md','LICENSE','THIRD_PARTY_NOTICES.md','third-party','docs','examples','themes')) { Copy-Item -LiteralPath $item -Destination $package -Recurse -Force }
    & (Join-Path $package 'cmtga.exe') doctor
    if ($LASTEXITCODE -ne 0) { throw 'Packaged executable doctor failed' }
    Compress-Archive -Path "$package/*" -DestinationPath dist/cmtga-windows-x64.zip -Force
    $hash = Get-FileHash -LiteralPath dist/cmtga-windows-x64.zip -Algorithm SHA256
    "$($hash.Hash.ToLower())  cmtga-windows-x64.zip" | Set-Content -LiteralPath dist/SHA256SUMS.txt -Encoding utf8
} finally { Pop-Location }
