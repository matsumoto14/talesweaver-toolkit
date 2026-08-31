# 紹介ページ(tw-context.dev)

ビルド不要の静的 HTML 1 枚。`index.html` を直接ブラウザで開けば確認できる。

## 置き方

Cloudflare の **Workers & Pages → Pages → Git に接続**で、このリポジトリを繋ぐ:

| 項目 | 値 |
|---|---|
| ルート ディレクトリ | `site` |
| ビルド コマンド | (空) |
| ビルド出力ディレクトリ | `.`(または `site`) |

カスタムドメインに **`tw-context.dev`** と **`www.tw-context.dev`** を割り当てる。

問い合わせ中継の Worker とは**別プロジェクト**にする。ページの更新でうっかり API を
落とさないため。

## ダウンロードボタン

ボタンの飛び先は R2 の固定 URL:

```
https://dl.tw-context.dev/latest/TW-Context-setup.exe
```

リリースのたびに `.github/workflows/release.yml` が上書きするので、**ページ側は
リリースのたびに触らなくてよい**。版と日付だけを GitHub API から補っていて、
取得に失敗してもボタンは押せる。

R2 バケットとトークンの用意は `.github/RELEASE.md` を参照。

## 「ブラウザで試す」の飛び先

ダウンロードボタンの下の導線は **`https://app.tw-context.dev/`**(ブラウザ版)を指す。
主役はインストール版のままなので、同じ大きさのボタンを並べず文字リンクにしてある。

ブラウザ版は**このページとは別の Pages プロジェクト**(`tw-context-app`)で、
`.github/workflows/web.yml` が push のたびに配る。Pages プロジェクトの作り方・
カスタムドメイン・R2 の CORS 設定は `docs/web-deploy.md` を参照。

## 色

配色はアプリの `apps/desktop/src/app.css` と同じトークンを写している
(規格は `docs/design-system.html`)。**サイトとアプリで色がずれると、
ダウンロードした人が別物に見える**ので、アプリ側のトークンを変えたらここも合わせる。

## 差し替えが要るもの

- `img/calc.png` は検証時の計算画面の暫定コピー。
  **ブランド表記が旧称 `TW TOOLKIT` のままで、検証用の雑なキャラ名が写っている**。
  公開前に、現在の UI で見栄えのする状態を撮り直して差し替える
- 署名を導入したら、「WindowsによってPCが保護されました」の節を消す
