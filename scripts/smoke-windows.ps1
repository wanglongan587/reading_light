param([string]$Executable = 'target/debug/glow-reading.exe', [string]$DataDirectory = 'target/smoke-check')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $exe = (Resolve-Path -LiteralPath $Executable).Path
    $data = [IO.Path]::GetFullPath((Join-Path $projectRoot $DataDirectory))
    if (-not $data.StartsWith((Join-Path $projectRoot 'target'), [StringComparison]::OrdinalIgnoreCase)) { throw 'Smoke data must stay inside target/' }
    New-Item -ItemType Directory -Force -Path $data | Out-Null
    $oldData = $env:GLOW_DATA_DIR
    $env:GLOW_DATA_DIR = $data
    try {
        $stdout = Join-Path $data 'stdout.log'
        $stderr = Join-Path $data 'stderr.log'
        # WebView2 needs a presented native window to exercise animation frames.
        # Start-Process also waits reliably for a Windows GUI-subsystem binary.
        $process = Start-Process -FilePath $exe -ArgumentList '--smoke-test' -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
        if (-not $process.WaitForExit(90000)) {
            Stop-Process -Id $process.Id
            throw 'Native smoke test timed out'
        }
        $process.Refresh()
        $output = Get-Content -LiteralPath $stdout -Raw
        $errors = Get-Content -LiteralPath $stderr -Raw
        if (($null -ne $process.ExitCode -and $process.ExitCode -ne 0) -or $output -notmatch 'GLOW_SMOKE_OK' -or $errors -match 'GLOW_SMOKE_FAILED|panicked') {
            throw "Native smoke failed (exit $($process.ExitCode)): $output $errors"
        }
        Write-Output $output
    } finally { $env:GLOW_DATA_DIR = $oldData }
} finally { Pop-Location }
