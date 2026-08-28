# 一般公開の準備

## 目的

リポジトリと配布物を外部に出せる状態にし、ユーザーがアプリから匿名で問い合わせを送れて、
それが GitHub Issue になる経路を作る。あわせて公開後の更新の配り方を決める。

## 決定(ユーザー判断)

- 同梱アイコン(talewiki 添付由来 943 枚)は**出典・非公式表記を付けて同梱を続ける**。
  権利者からの申し出に備え、ダウンロード方式へ切り替えられる余地だけ残す
- 問い合わせは**中継サーバー経由(匿名投稿可)**。ブラウザで GitHub を開く方式は採らない
- ホスティングは **Cloudflare Workers**、GitHub 側の認証は **GitHub App**
- Windows インストーラは **Azure Trusted Signing で署名する**
- 着手順は **段階4(アプリ整備) → 段階3(配布) → 段階5(問い合わせ)**

## 段階4: アプリ側の整備

### 4a ドキュメントと権利表記

- `LICENSE` — コードのライセンス
- `NOTICE.md` — 同梱画像・データの出典と権利の帰属、非公式である旨、削除要請の窓口
- `CHANGELOG.md` — **ユーザー向け**の言葉で書く。`docs/status.md` は開発者向けとして維持
- `README.md` を現状(19 キャラ 303 スキル・装備・属性・DPS まで実装済み)へ全面更新。
  冒頭に非公式表記

### 4b DB のバックアップと復元

後方互換を持たない方針(AGENTS.md)なので、**新版で開いた DB は旧版で開けない**。
ロールバック手段がバックアップしかないため、配布開始前に入れる。

- `crates/storage` に置く(SQLite の面倒を見るのは storage の責務。desktop は呼ぶだけ)
- 開く前に `tw-context.sqlite.bak.<app_version>` へコピー。同名があれば上書きしない
  (= その版で既に取ってある)。**直近 3 世代**だけ残す
- マイグレーションが失敗したら、壊れた DB を `*.broken.<timestamp>` へ退避 → 最新の bak から
  復元 → 再試行。それも失敗したら空 DB で起動する(**起動不能にしない**)
- 復元・退避が起きたことを起動時通知としてフロントへ返し、エラー帯に出す

### 4c CSP と情報パネル

- `tauri.conf.json` の `csp: null` を解除。アプリは外部へ一切通信していないので `'self'` で締める
  (段階5 で中継サーバーの origin だけ `connect-src` に足す)
- 上部バー右端に「情報」を追加。**明示クローズ式オーバーレイ**(装備登録と同じ形。
  背景クリックでは閉じず、`閉じる ×` か Escape)で、版 / 非公式表記 / 出典 / ライセンス /
  データの扱い / 更新確認 / 問い合わせ(段階5)を置く
- 「データは端末内のみ。外部送信は問い合わせ送信時だけ」を明記

## 段階3: 配布

- `.github/workflows/release.yml` に `tauri-apps/tauri-action`。`v*` タグ push で Windows を
  ビルドし、Release に `.msi` / NSIS `.exe` + `latest.json` を添付
- `bundle.targets` を `["nsis", "msi"]` に絞る(現在 `"all"`)
- Azure Trusted Signing を CI の署名ステップに組む
- `tauri-plugin-updater`。updater 用の署名鍵ペアを作り、公開鍵を `tauri.conf.json` に置く
- バージョンが `tauri.conf.json` / `package.json` / `Cargo.toml` の 3 箇所にあるので
  `tools/release/bump.py` で一括更新

## 段階5: 問い合わせ → GitHub Issue

```
アプリ ──POST /inquiry──▶ Cloudflare Worker ──GitHub API──▶ Issue (label: from-app)
  送信前に本文を全文表示     PoW / レート制限 / サニタイズ
                        ◀── issue URL を返してアプリに表示
```

- `services/inquiry-worker/` を新設(`docs/architecture.md` に追記)
- 認証は GitHub App(Issues: write のみ。installation token は 1 時間で失効)。秘密鍵は Worker secret
- スパム対策は初版から: **PoW**(Worker 発行 nonce、外部依存ゼロ)+ **IP ハッシュのレート制限**
  (例 5 件/日)+ **本文長上限** + **コードブロックに封じて `@` メンションと task list を無効化**。
  荒らされたら Turnstile を足す(WebView で動かすため CSP 緩和が要るので最初は入れない)
- 自動添付は アプリ版 / OS / WebView2 版 / 選択中のキャラ・スキル・コンテンツ / 直近エラー。
  **送信前に全文を画面で見せて確認させる**
- UI に明記する: 投稿は**公開 issue になる**(本名・メールを書かせない)/ 返信は issue 上で行うので
  送信後に出る URL を控えてもらう

## アップデート戦略

- **バージョン**: `0.MINOR.PATCH` で公開開始。`1.0.0` = 5 機能(ダメージ計算・キャラ管理・
  強化提案・ロードマップ・索引)がすべて実装され、実測突き合わせで検証済み
- **頻度**: goal ごとにタグを打たず、**2〜4 週で 1 本**に束ねる
- **配信**: 起動時に更新を確認し「新しい版があります」を帯で出すだけ。強制更新はしない
- **gamedata**: アプリ同梱のまま維持し、wiki 更新の反映もアプリ更新で配る
  (データだけ別配信に分離しない)。`tools/scraper` が動いたら差分検出 → 自動 issue 起票へ
- **サポート範囲**: 最新版のみ。問い合わせに版を自動添付するので判別できる

## 受け入れ条件

- [ ] `LICENSE` / `NOTICE.md` / `CHANGELOG.md` があり、README が現状と一致する
- [ ] DB が壊れていても起動でき、バックアップから復元したことが画面に出る
- [ ] `csp` が `null` でなく、アプリが正常に動く
- [ ] 情報パネルから 版・非公式表記・出典・データの扱い が読める
- [ ] タグ push で署名済みインストーラが Release に出る
- [ ] アプリから送った問い合わせが GitHub Issue になり、issue URL がアプリに表示される
- [ ] レート制限を超えた送信が拒否され、理由が画面に出る
