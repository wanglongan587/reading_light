param(
    [string]$InstallDirectory = 'E:\GlowReading',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if (-not $SkipBuild) {
        & (Join-Path $PSScriptRoot 'package-windows.ps1')
        if ($LASTEXITCODE -ne 0) { throw 'Packaging failed' }
    }
    $resolved = [IO.Path]::GetFullPath($InstallDirectory)
    if ($resolved -eq [IO.Path]::GetPathRoot($resolved)) { throw 'Install directory cannot be a drive root' }
    if (Get-Process glow-reading -ErrorAction SilentlyContinue) {
        throw 'Glow Reading is running. Close it before installing the update.'
    }
    New-Item -ItemType Directory -Force -Path $resolved | Out-Null
    Copy-Item -Path (Join-Path $projectRoot 'dist/windows-x64/*') -Destination $resolved -Recurse -Force
    $legacyWebViewCache = [IO.Path]::GetFullPath((Join-Path $resolved 'glow-reading.exe.WebView2'))
    $installPrefix = $resolved.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $legacyWebViewCache.StartsWith($installPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Legacy WebView cache path escaped the install directory'
    }
    if (Test-Path -LiteralPath $legacyWebViewCache) {
        Remove-Item -LiteralPath $legacyWebViewCache -Recurse -Force
    }
    $exe = Join-Path $resolved 'glow-reading.exe'
    if (-not (Test-Path -LiteralPath $exe)) { throw 'Installed executable is missing' }
    $localEnv = Join-Path $projectRoot '.env.local'
    if (Test-Path -LiteralPath $localEnv) {
        $configJson = @{ env_file = $localEnv } | ConvertTo-Json
        [IO.File]::WriteAllText((Join-Path $resolved 'openai-config.json'), $configJson, (New-Object Text.UTF8Encoding $false))
    }
    $shortcutPath = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Glow Reading.lnk'
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($shortcutPath)
    $shortcut.TargetPath = $exe
    $shortcut.WorkingDirectory = $resolved
    $shortcut.IconLocation = (Join-Path $resolved 'assets\glow.ico')
    $shortcut.Description = 'Glow Reading'
    $shortcut.Save()
    Write-Output "INSTALLED $resolved"
    Write-Output "SHORTCUT $shortcutPath"
} finally {
    Pop-Location
}
