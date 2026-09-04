# セキュリティ

## 報告

脆弱性を見つけた場合は、公開の Issue ではなく
[GitHub の Security Advisory](https://github.com/matsumoto14/talesweaver-toolkit/security/advisories/new)
から報告してください。個人が趣味で作っているツールなので即応はできませんが、確認して対応します。

## このツールが触るもの

- **読み書きするのは 1 か所だけ** — デスクトップ版は `%APPDATA%\dev.twcontext.app\` 以下の SQLite ファイルと
  そのバックアップ(直近 3 世代)。ブラウザ版はそのブラウザの IndexedDB
- **ゲームクライアントには接続しません。** ゲームのファイルもメモリも読み書きしません
- **ユーザーの操作なしに送信することはありません。** ネットワークに出るのは次の 3 つだけです
    - ユーザーが問い合わせ・実測を送ったとき — 送信先は `https://inquiry.tw-context.dev` の 1 か所だけで、
      送る内容は送信前に全文表示されます
    - 更新確認(`https://dl.tw-context.dev/latest/latest.json` を読むだけ。デスクトップ版のみ)
    - お知らせの取得(`https://dl.tw-context.dev/news/news.json` を読むだけ)
- WebView の CSP は `'self'` 系に絞ってあります(`apps/desktop/src-tauri/tauri.conf.json`)

## サポートするバージョン

最新版のみです。古い版で見つかった問題は、最新版で直っていないかを先に確認してください。
