# Tauri アプリをリモートデバッグ付きでバックグラウンド起動し、CDP ポートが開くまで待つ。
# 使い方: powershell -File .claude/skills/gui-smoke/scripts/start-app.ps1 [-Port 9222] [-TimeoutSec 600]
param(
    [int]$Port = 9222,
    [int]$TimeoutSec = 600
)
$ErrorActionPreference = "Stop"
$repo = Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")
$desktop = Join-Path $repo "apps\desktop"

function PortOpen { try { (New-Object Net.Sockets.TcpClient("127.0.0.1", $Port)).Close(); $true } catch { $false } }

if (PortOpen) { Write-Output "CDP port $Port は既に開いている(起動済み)"; exit 0 }

$env:Path += ";$env:USERPROFILE\.cargo\bin"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
$log = Join-Path $env:TEMP "tauri-dev.log"
$p = Start-Process -FilePath "cmd.exe" -ArgumentList "/c npm run tauri dev > `"$log`" 2>&1" -WorkingDirectory $desktop -WindowStyle Hidden -PassThru
Write-Output "起動中 pid=$($p.Id) log=$log"

$deadline = (Get-Date).AddSeconds($TimeoutSec)
while ((Get-Date) -lt $deadline) {
    if (PortOpen) { Write-Output "CDP port $Port open"; exit 0 }
    if ($p.HasExited) { Write-Output "プロセスが終了した。ログ末尾:"; Get-Content $log -Tail 30; exit 1 }
    Start-Sleep -Seconds 3
}
Write-Output "timeout ($TimeoutSec s)。ログ末尾:"; Get-Content $log -Tail 30; exit 1
