# 監査・スモークの前後で開発 DB を退避 / 復元する。
#
# なぜ要るか: キャラタブは自動保存、バフセットも押した時点で保存される。
# 「押したら必ず元に戻すこと」という運用ルールは対策にならなかった —
# 実際に押し間違い・戻し忘れ・戻したつもりの誤検証が起き、ユーザーの
# バフセットが 13 → 15 → 14 件と変わって元に戻せなくなった
# (2026-08-30。docs/adr/006 の経緯を参照)。
#
# アプリは DB を開いたままなので、**退避は起動前・復元は終了後**に行う。
#
#   powershell -File .claude/skills/gui-smoke/scripts/db-guard.ps1 -Action save
#   ... アプリを起動して監査 ...
#   powershell -File .claude/skills/gui-smoke/scripts/db-guard.ps1 -Action restore
#
# -Action status で、退避済みかどうかと差分の有無だけを見る。

param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('save', 'restore', 'status')]
    [string]$Action
)

$ErrorActionPreference = 'Stop'

$Db = Join-Path $env:APPDATA 'dev.twcontext.app\tw-context.sqlite'
$Guard = Join-Path $env:APPDATA 'dev.twcontext.app\tw-context.sqlite.guard'

function Test-AppRunning {
    $p = Get-Process -Name 'talesweaver-toolkit' -ErrorAction SilentlyContinue
    return $null -ne $p
}

if (-not (Test-Path $Db)) {
    Write-Output "DB が無い: $Db"
    exit 1
}

switch ($Action) {
    'save' {
        if (Test-AppRunning) {
            # 起動中に取ると WAL の途中を掴む。落としてから取る
            Write-Output 'NG アプリが起動中。終了してから save すること'
            exit 1
        }
        Copy-Item $Db $Guard -Force
        $h = (Get-FileHash $Guard -Algorithm SHA256).Hash
        Write-Output "退避した: $Guard"
        Write-Output "sha256: $h"
    }
    'restore' {
        if (-not (Test-Path $Guard)) {
            Write-Output "NG 退避が無い。save していないか、既に restore 済み"
            exit 1
        }
        if (Test-AppRunning) {
            Write-Output 'NG アプリが起動中。終了してから restore すること'
            exit 1
        }
        $before = (Get-FileHash $Db -Algorithm SHA256).Hash
        $after = (Get-FileHash $Guard -Algorithm SHA256).Hash
        if ($before -eq $after) {
            Write-Output 'OK 監査は DB を書き換えていない'
        }
        else {
            Write-Output '書き換わっていたので戻した(何が変わったかは監査ログから追うこと)'
        }
        Copy-Item $Guard $Db -Force
        Remove-Item $Guard -Force
        Write-Output '復元した'
    }
    'status' {
        if (-not (Test-Path $Guard)) {
            Write-Output '退避なし'
            exit 0
        }
        $before = (Get-FileHash $Db -Algorithm SHA256).Hash
        $after = (Get-FileHash $Guard -Algorithm SHA256).Hash
        if ($before -eq $after) { Write-Output '退避あり / 差分なし' }
        else { Write-Output '退避あり / **差分あり**(restore で戻る)' }
    }
}
