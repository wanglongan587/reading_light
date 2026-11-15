param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if (-not $SkipBuild) {
        Push-Location web
        try {
            npm.cmd ci --no-fund
            if ($LASTEXITCODE -ne 0) { throw 'npm ci failed' }
            npm.cmd run build
            if ($LASTEXITCODE -ne 0) { throw 'Reader build failed' }
        } finally { Pop-Location }
        cargo build --release --locked -p glow-reading
        if ($LASTEXITCODE -ne 0) { throw 'Rust build failed' }
    }
    $distRoot = [IO.Path]::GetFullPath((Join-Path $projectRoot 'dist'))
    $stage = [IO.Path]::GetFullPath((Join-Path $distRoot 'windows-x64'))
    if (-not $stage.StartsWith($distRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Package stage must stay inside dist/'
    }
    if (Test-Path -LiteralPath $stage) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $stage | Out-Null
    Copy-Item -LiteralPath 'target/release/glow-reading.exe' -Destination $stage -Force
    Copy-Item -LiteralPath 'README.md','LICENSE','THIRD_PARTY_NOTICES.md' -Destination $stage -Force
    Copy-Item -LiteralPath 'scripts/Launch-Glow.ps1' -Destination $stage -Force
    New-Item -ItemType Directory -Force -Path (Join-Path $stage 'assets') | Out-Null
    Copy-Item -LiteralPath 'assets/SOURCES.md','assets/quotes.json','assets/glow.ico' -Destination (Join-Path $stage 'assets') -Force
    New-Item -ItemType Directory -Force -Path (Join-Path $stage 'docs') | Out-Null
    Copy-Item -LiteralPath 'docs/VALIDATION.md','docs/TRYOUT.md' -Destination (Join-Path $stage 'docs') -Force
    python scripts/collect-licenses.py
    if ($LASTEXITCODE -ne 0) { throw 'License collection failed' }
    $archive = Join-Path $projectRoot 'dist/glow-reading-windows-x64.zip'
    python scripts/zip-windows.py
    if ($LASTEXITCODE -ne 0) { throw 'ZIP packaging failed' }
    (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash | Set-Content -LiteralPath "$archive.sha256"
    Write-Output $archive
} finally { Pop-Location }
