---
name: gui-smoke
description: Tauri デスクトップアプリを実機起動し、WebView2 のリモートデバッグ(CDP)+ Playwright で画面操作・スモークテスト・スクリーンショット撮影を行う。UI 変更の実機確認、docs/screenshots の更新、画面の回帰確認を頼まれたときに使う。
---

# GUI スモークテスト(WebView2 CDP + Playwright)

専用ドライバは書かない。WebView2 に `--remote-debugging-port=9222` を付けて `npm run tauri dev` を起動し、Playwright の `chromium.connectOverCDP` で操作する。

## 手順

1. **起動**(バックグラウンド。初回の Rust ビルドは数分):
   ```
   powershell -File .claude/skills/gui-smoke/scripts/start-app.ps1
   ```
   9222 が開くまで待って戻る。既に起動済みならそのまま使う(二重起動しない)。
2. **スクリプト作成**: `scripts/smoke-template.js` をスクラッチパッドに **1 ファイルだけ**コピーし、確認項目を書く。ヘルパー(`nav` / `openCharacter` / `openGroup` / `statInput` / `selectByLabel` / `checkbox` / `saveCharacter` / `calculate` / `openTrace` / `textAfter` など)は実機で動作確認済みなので、**DOM 探索(`innerHTML` / `bodyText.slice` の出力)から始めない**。ヘルパーで届かない要素があるときだけ、その要素の周辺に限定して `page.content()` を確認する。スクリプトは `Edit` で差分修正し、毎回 heredoc で書き直さない。
3. **実行**: Playwright は本リポに入っていないので旧リポの node_modules を使う:
   ```
   NODE_PATH=/c/github/private/twtoolkit/node_modules node <script>.js
   ```
4. **撮影**: スクリーンショットは `docs/screenshots/<NN>-<内容>.png`(fullPage、ビューポート 1280×840 が標準)。既存番号と重複させない。
5. **終了**: 起動したプロセスを止める(`Stop-Process -Name talesweaver-toolkit` または tauri dev のウィンドウを閉じる)。

## 注意

- パスは **Bash ツールなら `/c/github/...`(`cd C:\...` は使えない)**、PowerShell が必要な操作(`Stop-Process` 等)は `PowerShell` ツールで実行する
- 入力値は司令塔の依頼文の値をそのまま使う。依頼に無い値を自分で決めない(domain テストと同じ値で実機確認する運用)

- **開発 DB `%APPDATA%\com.talesweaver.toolkit\talesweaver-toolkit.sqlite` を削除しない。** 検証用キャラは名前に「検証」を付けて作り、終わったら画面から削除する
- セレクタは DOM 構造(`.group-head` / `label.field` / `nav button` など)に依存する。UI 変更後は先に `page.content()` を短く確認してから書く
- `tauri dev` は Rust 変更で自動再起動する。再起動後は CDP に再接続が必要
- 報告はログ全文ではなく「確認項目 → OK/NG、NG の再現手順、スクリーンショットのパス」のみ
