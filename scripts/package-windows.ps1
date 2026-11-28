param(
    [switch]$SkipBuild,
    [switch]$BuildInstaller
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot

function Invoke-CodeSign {
    param([string]$FilePath)
    if (-not $env:GLOW_SIGN_PFX) { return }
    $signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if (-not $signtool) {
        $sdkBin = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
            Sort-Object FullName -Descending | Select-Object -First 1
        if ($sdkBin) { $signtool = $sdkBin.FullName }
    } else {
        $signtool = $signtool.Source
    }
    if (-not $signtool) {
        throw 'GLOW_SIGN_PFX is set but signtool.exe was not found on PATH or in Windows SDK.'
    }
    $tsUrl = if ($env:GLOW_TIMESTAMP_URL) { $env:GLOW_TIMESTAMP_URL } else { 'http://timestamp.digicert.com' }
    $args = @('sign', '/fd', 'SHA256', '/tr', $tsUrl, '/td', 'SHA256', '/f', $env:GLOW_SIGN_PFX)
    if ($env:GLOW_SIGN_PASSWORD) {
        $args += @('/p', $env:GLOW_SIGN_PASSWORD)
    }
    $args += $FilePath
    & $signtool @args
    if ($LASTEXITCODE -ne 0) { throw "Authenticode signing failed for $FilePath" }
}

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
    $cargoToml = Get-Content -LiteralPath 'Cargo.toml' -Raw
    $versionMatch = [regex]::Match($cargoToml, 'version\s*=\s*"([^"]+)"')
    $appVersion = if ($versionMatch.Success) { $versionMatch.Groups[1].Value } else { '0.1.5' }

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
    Invoke-CodeSign (Join-Path $stage 'glow-reading.exe')

    Copy-Item -LiteralPath 'README.md','LICENSE','THIRD_PARTY_NOTICES.md' -Destination $stage -Force
    Copy-Item -LiteralPath 'scripts/Launch-Glow.ps1' -Destination $stage -Force
    New-Item -ItemType Directory -Force -Path (Join-Path $stage 'assets') | Out-Null
    Copy-Item -LiteralPath 'assets/SOURCES.md','assets/quotes.json','assets/glow.ico' -Destination (Join-Path $stage 'assets') -Force
    New-Item -ItemType Directory -Force -Path (Join-Path $stage 'docs') | Out-Null
    Copy-Item -LiteralPath 'docs/VALIDATION.md','docs/TRYOUT.md','docs/ROADMAP.md' -Destination (Join-Path $stage 'docs') -Force
    python scripts/collect-licenses.py
    if ($LASTEXITCODE -ne 0) { throw 'License collection failed' }
    $archive = Join-Path $projectRoot 'dist/glow-reading-windows-x64.zip'
    python scripts/zip-windows.py
    if ($LASTEXITCODE -ne 0) { throw 'ZIP packaging failed' }
    (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash | Set-Content -LiteralPath "$archive.sha256"
    Write-Output $archive

    if ($BuildInstaller) {
        $makensis = Get-Command makensis.exe -ErrorAction SilentlyContinue
        if ($makensis) {
            $installerOut = Join-Path $distRoot "GlowReading-$appVersion-windows-x64-setup.exe"
            & $makensis.Source "/DAPP_VERSION=$appVersion" "/DSTAGE_DIR=$stage" "/DOUT_FILE=$installerOut" 'scripts/installer-windows.nsi'
            if ($LASTEXITCODE -ne 0) { throw 'NSIS installer build failed' }
            Invoke-CodeSign $installerOut
            (Get-FileHash -LiteralPath $installerOut -Algorithm SHA256).Hash | Set-Content -LiteralPath "$installerOut.sha256"
            Write-Output $installerOut
        } else {
            Write-Warning 'makensis.exe not found; skipped NSIS installer generation.'
        }
    }

    python scripts/generate-update-manifest.py $distRoot
    if ($LASTEXITCODE -ne 0) { throw 'Update manifest generation failed' }
} finally { Pop-Location }
