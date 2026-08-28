# 問い合わせ中継(Cloudflare Workers)

アプリの「情報 → 問い合わせ」から送られた内容を GitHub Issue にする。

```
アプリ ──GET  /challenge──▶ Worker            nonce と難易度を返す
アプリ                      (PoW を解く)
アプリ ──POST /inquiry ───▶ Worker ──API──▶ GitHub Issue (label: from-app, unverified)
                                   ◀── issue の URL を返す
```

## 匿名で受けるための備え

| | 内容 |
|---|---|
| proof-of-work | Worker が署名した nonce に対して sha256 の先頭 20 ビットが 0 になる解を要求。外部サービスに依存しない |
| レート制限 | IP のハッシュ単位で 1 日 5 件。**IP そのものは保存しない** |
| サニタイズ | 長さ上限、制御文字の除去、`@名前` / `#123` をリンクさせない、``` を潰してコードブロックから抜け出せなくする |

荒らされたら Cloudflare Turnstile を足す。WebView で動かすため CSP の緩和が要るので最初は入れない。

## GitHub App を作る

1. Settings → Developer settings → GitHub Apps → New
2. Permissions: **Repository permissions → Issues: Read and write** だけ。ほかは No access
3. Webhook は無効でよい
4. **App ID** を控える
5. 秘密鍵を生成してダウンロード(`.pem`)
6. このリポジトリにだけ Install し、Install 後の URL 末尾の数値(**installation id**)を控える

GitHub が配る鍵は PKCS#1 なので、WebCrypto が読める PKCS#8 に変換する:

```sh
openssl pkcs8 -topk8 -inform PEM -outform PEM -nocrypt \
  -in your-app.private-key.pem -out app-key-pkcs8.pem
```

## デプロイ

```sh
cd services/inquiry-worker
npm install

# KV を作って、出た id を wrangler.toml に書く
npx wrangler kv namespace create INQUIRY

# 秘密はこの 2 つだけ。App ID と installation id は wrangler.toml の [vars] にある
npx wrangler secret put GITHUB_APP_PRIVATE_KEY   # app-key-pkcs8.pem の全文を貼る
npx wrangler secret put NONCE_SECRET             # 任意の乱数(openssl rand -hex 32)

npx wrangler deploy
```

## ドメイン

エンドポイントは **`https://inquiry.tw-context.dev`**。アプリ側の 2 か所に既に入っている:

- `apps/desktop/src/inquiry.ts` の `INQUIRY_ENDPOINT`
- `apps/desktop/src-tauri/tauri.conf.json` の `csp` の `connect-src`

`workers.dev` ではなく独自ドメインにしてあるのは、**この URL が配布したアプリの CSP に
焼き込まれる**ため。あとから変えると、既に入っている版からは二度と送信できなくなる。
自分のドメインなら、Cloudflare をやめても向き先を移して古い版を生かせる。

設定は Cloudflare ダッシュボードの Worker → Settings → Domains & Routes →
Add → **Custom Domain** で `inquiry.tw-context.dev`。証明書とルートは自動で付く
(`wrangler.toml` に routes は書かない)。apex は配布ページ用に空けておく。

## リポジトリ側の準備

`from-app` と `unverified` のラベルを作っておく(無いと issue 作成が失敗する)。

## 動作確認

```sh
curl https://inquiry.tw-context.dev/challenge
# PoW を解かずに投げると 400 になる
curl -X POST https://inquiry.tw-context.dev/inquiry -H 'content-type: application/json' \
  -d '{"nonce":"x.y.z","solution":"0","title":"t","body":"b"}'
```

## 費用

無料枠(1 日 10 万リクエスト、KV 1 日 10 万読み・1000 書き)で十分に収まる。
レート制限を超える書き込みは KV に届く前に弾かれる。
