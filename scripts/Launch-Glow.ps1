$ErrorActionPreference = 'Stop'
$found = $false
$roots = @('HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients','HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients','HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients')
foreach ($root in $roots) {
    if (Test-Path -LiteralPath $root) {
        foreach ($key in Get-ChildItem -LiteralPath $root) {
            $entry = Get-ItemProperty -LiteralPath $key.PSPath
            if ($entry.name -like '*WebView2*' -and $entry.pv -and $entry.pv -ne '0.0.0.0') { $found = $true }
        }
    }
}
if (-not $found) {
    Write-Host '未检测到 WebView2 Runtime。若阅读组件无法打开，请从微软官网安装：'
    Write-Host 'https://developer.microsoft.com/microsoft-edge/webview2/'
    Write-Host '不会自动下载或修改系统。'
}
& (Join-Path $PSScriptRoot 'glow-reading.exe')
