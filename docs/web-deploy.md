# ブラウザ版の配信(app.tw-context.dev)

ブラウザ版の中身と設計は [ADR-012](adr/012-web-build.md)。ここに書くのは**人が一度だけやる設定**と、
その理由。ビルドと配信は `.github/workflows/web.yml` が自動でやる。

| 何を | どこに |
|---|---|
| ブラウザ版(アプリ本体) | Cloudflare Pages `tw-context-app` → `app.tw-context.dev` |
| 紹介ページ(`site/`) | Cloudflare Pages(別プロジェクト)→ `tw-context.dev` |
| インストーラ・お知らせ | Cloudflare R2 `tw-context` → `dl.tw-context.dev` |

## 1. Pages プロジェクトを作る

**Git 接続にしない。** ブラウザ版のビルドは Rust の `wasm32-unknown-unknown` と wasm-pack を要る
(`npm run build:wasm`)。Pages のビルド環境にそれを用意させると、こちらで固定できない土台の上で
毎回コンパイルすることになる。GitHub Actions で組み上げて、Pages には静的ファイルだけ送る。

紹介ページとも**別プロジェクト**にする。ページの文言を直したつもりでアプリを落とさないため。

1. Cloudflare ダッシュボード → **Workers & Pages** → **Create** → **Pages** → **Upload assets**
2. プロジェクト名に **`tw-context-app`** を入れる(`web.yml` の `--project-name` と同じ文字列。
   ここがずれると CI は「プロジェクトが無い」で落ちる)
3. 空のフォルダ、または適当なプレースホルダの `index.html` を 1 つ上げて作成を終える。
   中身は最初の CI 実行で丸ごと置き換わるので何でもよい
4. `main` に push するか、Actions から **Web** を手動実行すると、以降は自動で更新される

GitHub Secrets は既存の `CLOUDFLARE_API_TOKEN` / `CLOUDFLARE_ACCOUNT_ID` をそのまま使う。
トークンの権限に **`Cloudflare Pages: Edit`** が要る(R2 用の権限しか付けていない場合は足す)。

## 2. カスタムドメインを割り当てる

プロジェクト → **Custom domains** → **Set up a domain** → **`app.tw-context.dev`**。

`tw-context.dev` は紹介ページが使っているので、アプリはサブドメインに置く。
`www` のようなページ側の系統と混ざらず、「紹介を読む → アプリを開く」の関係が URL で読める。

## 3. R2 に CORS を設定する(これが無いとお知らせが古いまま出る)

ブラウザ版はお知らせ(`https://dl.tw-context.dev/news/news.json`)を**素の `fetch`** で取る
(`apps/desktop/src/web/http.ts`)。別オリジンなので、R2 側が CORS を返さないとブラウザが
読み取りを止める。**エラーは出ない** — 呼び出し側(`src/news.ts`)が同梱ぶんに落ちるので、
アプリを出し直すまで**古いお知らせが黙って出続ける**。気づけない壊れ方なので必ず設定する。

**デスクトップ版は影響を受けない。** あちらは Rust(`@tauri-apps/plugin-http`)経由で取りに行くので
ブラウザの同一オリジンポリシーの外にいる。CORS はブラウザ版のためだけの設定。

R2 → バケット **`tw-context`** → **Settings** → **CORS policy** → **Add CORS policy**:

```json
[
  {
    "AllowedOrigins": ["https://app.tw-context.dev"],
    "AllowedMethods": ["GET", "HEAD"],
    "AllowedHeaders": ["*"],
    "ExposeHeaders": ["Content-Length", "Content-Type"],
    "MaxAgeSeconds": 3600
  }
]
```

- **オリジン**は Pages のプレビュー URL(`*.pages.dev`)を足したくなるが、増やすほど
  「どこから読まれてもよいファイル」が広がる。手元での確認は `npm run preview:web` で足りるので、
  本番のオリジンだけにしておく
- **メソッド**は `GET` と `HEAD` だけ。ブラウザ版が R2 に書くことはない
- 設定したら、ブラウザ版を開いて「お知らせ」タブが同梱ぶんではなく配信ぶんを出しているかを見る
  (開発者ツールの Network で `news.json` が 200 で返り、CORS の警告が出ないこと)

## データはブラウザ版とデスクトップ版で行き来しない

**意図してそうしている**(ADR-012)。ブラウザ版の保存先はそのブラウザの IndexedDB、
デスクトップ版は `%APPDATA%\dev.twcontext.app\tw-context.sqlite` で、預け先が別物。
同期するにはデータをサーバーに置くことになり、「匿名で使える」性質(ADR-009)を捨てる。

移すときは、**情報パネルの「書き出し」/「読み込み」**(JSON)を使う。
ブラウザの保存領域はサイトデータの削除で黙って消えるので、ブラウザ版で本気で作り込むなら
書き出しておくか、デスクトップ版に移すのが安全。この注意はブラウザ版の画面と紹介ページの
導線にも書いてある。
