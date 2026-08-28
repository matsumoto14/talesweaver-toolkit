# セキュリティ

## 報告

脆弱性を見つけた場合は、公開の Issue ではなく
[GitHub の Security Advisory](https://github.com/matsumoto14/talesweaver-toolkit/security/advisories/new)
から報告してください。個人が趣味で作っているツールなので即応はできませんが、確認して対応します。

## このツールが触るもの

- **読み書きするのは 1 か所だけ** — `%APPDATA%\com.talesweaver.toolkit\` 以下の SQLite ファイルと、
  そのバックアップ(直近 3 世代)
- **ゲームクライアントには接続しません。** ゲームのファイルもメモリも読み書きしません
- **自動的な外部通信はありません。** ネットワークに出るのは次の 2 つだけです
    - ユーザーが問い合わせを送ったとき(送る内容は送信前に全文表示されます)
    - 起動時の更新確認(GitHub Releases の `latest.json` を見るだけ)
- WebView の CSP は `'self'` 系に絞ってあります(`apps/desktop/src-tauri/tauri.conf.json`)

## サポートするバージョン

最新版のみです。古い版で見つかった問題は、最新版で直っていないかを先に確認してください。
