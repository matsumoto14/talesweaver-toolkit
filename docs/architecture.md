# アーキテクチャ

方針: **構造ファースト**。ゲームのドメインモデルを中心に置き、5つの機能(ダメージ計算・キャラ管理・強化提案・ロードマップ・やりたいこと索引)はすべて同じモデルの消費者として実装する。

## 全体像

```
┌──────────────────────────────────────────────────────────┐
│ apps/desktop/src … フロントエンド(TS)。表示と入力のみ      │
│   api/commands.ts = コマンド呼び出しの唯一の境界(44 本)    │
└──────────────┬───────────────────────────┬───────────────┘
┌──────────────┴───────────┐  ┌────────────┴───────────────┐
│ デスクトップ版(Tauri)   │  │ ブラウザ版(WASM)         │
│ apps/desktop/src-tauri    │  │ crates/web + api/         │
│ + crates/storage(SQLite) │  │ browserStore(IndexedDB)  │
└──────────────┬───────────┘  └────────────┬───────────────┘
               ├───────────────────────────┘
┌──────────────┴───────────┐
│ crates/commands           │ 保存に触らないコマンドの中身
└──────────────┬───────────┘ (両方がここを呼ぶ)
┌──────────────┴───────────┐  ┌────────────────┐
│ crates/domain             │←─│ crates/gamedata │
│ 純粋なドメインモデル+計算   │  │ 静的データの型と  │
│ I/O なし・決定的           │  │ ローダ(wiki由来) │
└──────────────────────────┘  └───────┬────────┘
                                      ↑ 生成
                              ┌───────┴────────┐
                              │ tools/scraper ※ │
                              │ talewiki 取込み  │
                              └────────────────┘
```

デスクトップ版とブラウザ版は同じ画面・同じ計算で、違うのは**保存と外の口だけ**。
どの crate がどちらで動くかは次のとおり(決定の理由は docs/adr/012-web-build.md):

| | 中身 | デスクトップ | ブラウザ |
|---|---|---|---|
| `crates/domain` | ドメインモデルと計算(I/O なし) | ○ | ○ |
| `crates/gamedata` | wiki 由来の静的データ | ○ | ○ |
| `crates/commands` | 保存に触らないコマンドの中身 | ○ | ○ |
| `crates/web` | ブラウザ版の `invoke`(wasm-bindgen) | — | ○ |
| `crates/storage` | 登録キャラの保存(rusqlite) | ○ | — |
| `apps/desktop/src-tauri` | Tauri コマンド・OS 連携・バックアップ | ○ | — |
| `apps/desktop/src/api/browserStore.ts` | ブラウザ版の保存(IndexedDB) | — | ○ |

## クレート構成と責務

### crates/domain — ドメインモデルと計算(核)

- **モデル**: `Character`(素ステ・装備・スキル構成・覚醒/エタ・バフセット)、`Equipment`(部位別 12 スロット + 部位別シエナ登録 + 称号 + テシスコア。装備部位はアイテム参照/カスタム・基本能力値・エンチャント・強化 Lv・武器アビリティ・**ランダムオプション**を持つ。シエナのオーラは抽出・注入できるため装備登録と独立した部位別登録一覧 + 装着中IDで持ち、装着中だけを装備攻撃力等へ集計)、`SoulLinkStatus`(リンクステータス 1〜10 の Lv。キャラ単位・条件達成済み前提。1〜7を基本能力/戦闘計算へ接続し、8〜10は記録)、`CommonSkills`(共通スキル。装備攻撃力強化倍率・装備防御力倍率・割合追加ダメージ)、`Skill`、`Buff`、`Enemy`、`Content`(入場条件)
- **計算**: 能力値計算 → カテゴリ集計 → 与ダメージ式 → 段数・追加ダメージ の4段パイプライン(docs/damage-formula.md §13)
- **装備の所有形**: 各部位は登録リストと選択中IDを持つ。保存検証は登録全件、計算は選択中だけ。属性強化はキャラの選択属性を対象の実装備へ +9 自動反映する
- **機能サービス**: ダメージ計算・強化提案・ロードマップ判定・索引。すべて `Character` を入力に取る純関数群
- 制約:
  - **I/O を一切持たない**(SQLite・ファイル・ネットワーク禁止)。引数で受けて結果を返すだけ。決定的で全面的にユニットテスト可能
  - **全中間値をトレースとして返す**(旧リポの教訓: 最終値だけの一致は偶然一致を見逃す)
  - 丸めは専用関数(`floor_int`, `trunc2` 等)として型で固定し、裸の `as`/`floor()` を式に書かない
  - バフは「カテゴリ + 数値 + 重複枠」のデータであり、個別バフをコードで分岐しない

### crates/gamedata — 静的データ(wiki 由来)

- スキル・敵・バフカタログ・係数・覚醒倍率などの型定義とローダ。アプリに同梱し、ユーザーは編集しない
- 各データに出典(wiki ページ・取得日)とゲームバージョンをメタデータとして持たせる
- 旧リポの JSON(docs/legacy-twtoolkit.md ⭐)を初期シードにするが、スキーマはドメイン語彙で定義し直し、wiki の現行値で検証してから採用する

### crates/storage — ユーザーデータ(SQLite)

- 登録キャラ・バフセット・設定のみ。静的データは入れない(この分離が Web/モバイル展開時の差し替え範囲を最小化する)
- `rusqlite` を使うのでブラウザでは動かない。ブラウザ版の保存は TS 側(`api/browserStore.ts`、
  IndexedDB)にあり、デスクトップとデータは行き来しない(下の「ブラウザ版」を参照)
- `buff_sets` は名前と `BuffSelection` を持ち、`characters.default_buff_set_id` は「いつものセット」だけを参照する。計算時は `StatSources` とバフ選択を別引数で domain へ渡す
- `character_icons` は登録キャラごとの任意画像を128×128 PNGのBLOBで持つ。`characters` 削除時にCASCADEし、ゲーム内キャラの静的アイコンやdomainモデルには混ぜない
- domain の型との変換はここで行う。domain は SQLite を知らない

### crates/commands — 保存に触らないコマンドの中身

- ダメージ計算・能力値プレビュー・カタログ列挙・検証など、**保存に触らないコマンドの中身**。
  Tauri にも SQLite にも依存しないので wasm32 でそのままビルドできる
- デスクトップ版(`src-tauri`)とブラウザ版(`crates/web`)の両方がここを呼ぶ。計算が
  2 つに分かれると、片方だけ直った状態に気づけないため、実体は 1 つに保つ
- 逆に**保存は共通化しない**。保存が要るコマンドは各プラットフォームの入口が受け持つ

### crates/web — ブラウザ版の入口(WASM)

- 公開するのは `invoke(command, args)` の 1 本だけで、形は Tauri の `invoke` と同じ。
  画面は import 先が差し替わるだけで済み、コマンドごとのバインディングを持たない
- 44 コマンドのうち **26 はここが `commands` を呼んでそのまま返し、18 は TS 側**
  (`api/invoke.wasm.ts` → `browserStore.ts`)が IndexedDB で処理する。保存の前検証だけは
  `domain` を持つこちら側に問う(文言をデスクトップ版と揃えるため)
- 引数は Tauri と同じ camelCase で来るので、コマンドごとの引数 struct で受ける。名前の
  食い違いは実行時にしか出ないため、`args_check.rs` が画面の呼び出しと突き合わせる
- ビルドは `npm run build:web`(wasm-pack + `vite.web.config.ts`)。差し替えは**実行時分岐では
  なく vite の alias** なので、デスクトップ版のバンドルに WASM は入らない

### apps/desktop — Tauri シェル

- コマンドは「storage/gamedata から読む → domain を呼ぶ → 結果を返す」だけの薄いアダプタ。ロジックを書かない
  - 未実装要素の中立値(属性など)もコマンドに書かない。`DamageMaterial` のように domain 側へ集約し、実装時にそこを引数へ昇格させる
- UI は表示と入力のみ。計算・判定は必ず Rust 側

フロントエンドの階層(`apps/desktop/src/`):

```
main.ts, App.svelte    エントリと画面枠(上部タブ・エラー帯・キャラレール)。v4 デザイン準拠。
                       main.ts は値域上限(get_stat_limits)を取り切ってから App を動的 import する
                       (labels.ts などがモジュール評価時に上限を読むため。フォールバック値は持たない)
CharacterRail.svelte   左のキャラレール(全タブ共通の「どのキャラの話か」+ クリア数 + 登録導線。表示順は端末内設定として保持)
state.svelte.ts        共有状態(タブ・カタログ・登録キャラ・カスタム画像data URL・選択・コンテンツ判定・試し変更 sim)
api/types.ts           コマンドの入出力型。Rust の serde 構造体の写し(手動同期)
api/commands.ts        invoke ラッパー。画面からコマンドを呼ぶ唯一の入口(44 本)
api/invoke.ts          呼び出しの実体。デスクトップは Tauri、ブラウザは invoke.wasm.ts に
                       vite の alias で差し替わる(画面はどちらか知らない)
api/browserStore.ts    ブラウザ版の保存(IndexedDB)/ api/transfer.ts データの書き出し・読み込み
web/                   ブラウザ版での Tauri プラグイン相当(外部リンク・HTTP・更新・プロセス)
ui/                    画面によらない汎用部品(Select, StatInput, AdjustmentEditor, Splitter, persistedState)
pages/<機能>/          機能ごとの画面と、その画面専用の部品
buffs.ts               バフ選択の共通ロジック(純関数)
candidates.ts          強化候補の列挙(効果の計算は Rust 側 preview_damage)
draft.ts               キャラ編集ドラフトの型と組み立て
format.ts, labels.ts   数値整形・表示名
limits.svelte.ts       domain の値域上限(`get_stat_limits`)の共有状態
toast.svelte.ts        エラー帯の共有状態
```

機能を足すときは `pages/<機能>/` を作る。2 つ以上の機能で使う部品だけを `ui/` に上げる。
SvelteKit は使っていないため `src/lib/` は置かない(`$lib` エイリアスがなく、階層が 1 段深くなるだけ)。

- **ビジュアル**はデザインモック「TW Toolkit Prototype v4」(claude.ai/design)準拠のライトテーマ。トークンは `app.css`、フォント(M PLUS Rounded 1c / M PLUS 1 Code)は `@fontsource` で同梱
- **数値入力**は `ui/StatInput.svelte` の 1 種類のみ(従来どおり)。範囲上限は `limits.svelte.ts` から取る
- **`pages/home/`**: `HomePage.svelte` — 到達一覧(エリア → コンテンツ、目安バー・バッジ・入場条件ノート)、お気に入り(localStorage)、「次に変えるなら」(候補を `preview_damage` で再計算し、押すと計算タブの試し変更に入る)
- **`pages/calc/`**: `CalcPage.svelte` — 対象プレート(◀▶ + エリア別一覧)、スキル選択、1発(最大)+ 合計/クリティカル、もし〜だったら、なぜこの数字?(攻撃力の内訳 / 防御を抜く / 倍率で伸ばす。トレースの式から組み立て)/ `TracePanel.svelte` — 詳細トレース。右カラム「計算の材料」= 試し変更(sim)・装備・バフ・調整・コンボ・入場条件
- **`pages/buffs/`**: セット一覧 → 静的カタログからの選択 → 効果・排他枠要約。独自バフ定義は作らない
- **`pages/chars/`**: `CharsPage.svelte`(外枠)/ `RegisterPane.svelte`(名前 + 19 職アイコンのみの最小登録・コピー登録)/ `Workspace.svelte`(draft 管理・`preview_effective_stats` の即時プレビュー・保存・いまの実力シート)/ `SourcePane.svelte`(補正源ドリルダウンの編集ペイン)。登録後のStatusPaneだけで任意画像を選び、レール・ホーム・現在キャラへ共通反映する

### tools/scraper — talewiki 取り込み(※未実装)

- EUC-JP PukiWiki の取得(`cmd=source`)→ パース → gamedata の生成。手法は docs/damage-formula.md 取得メモと旧リポの scrapeSkills.js を参照
- 生成物との差分検出(wiki 更新の検知)をここで行う

### services/inquiry-worker — 問い合わせの中継(アプリ外)

- アプリ右上の「問い合わせ」から送られた内容を GitHub Issue にする Cloudflare Workers
- **アプリ本体からは独立している**。ここが落ちても計算・保存は動く(問い合わせが送れないだけ)
- アプリに秘密を持たせないための中継。GitHub App の秘密鍵は Worker のシークレットにあり、
  アプリ側は認証を持たない。代わりに proof-of-work + IP ハッシュのレート制限で匿名投稿を守る
- 唯一の外部通信先。エンドポイントは `apps/desktop/src/inquiry.ts` と
  `tauri.conf.json` の `connect-src` の **2 か所**に書くので、変えるときは両方直す

### site/ — 紹介ページ(アプリ外)

- `tw-context.dev` に置く静的 HTML 1 枚。ビルド不要。Cloudflare Pages の別プロジェクトとして配る
- 配色はアプリの `app.css` のトークンを写す。サイトとアプリで色がずれると別物に見える
- ダウンロードボタンは R2 の固定 URL(`dl.tw-context.dev/latest/…`)を指す。
  中身はリリースワークフローが同じビルド成果物から上書きするので、Releases とずれない

## 依存の向き

```
apps/desktop → storage, gamedata, domain
storage      → domain(型のため)
gamedata     → domain(型のため)
domain       → (何にも依存しない)
tools/scraper→ gamedata(スキーマ共有)
services/    → (どのクレートにも依存しない。HTTP でだけ繋がる)
```

domain が最内層。逆流(domain → storage 等)は禁止。

## 語彙の規約

- 型名・関数名・フィールド名は**ドメイン用語**(例: `AttackPower`, `SkillMultiplier`, `DamageCategory::FinalDamageRate`)
- wiki のカテゴリ記号(A〜Y)や Excel セル名(AF54)は出典情報。doc コメントに `/// wiki: カテゴリL(最終ダメージ)` の形で残し、識別子には使わない

## 機能がモデルを共有する構図(キャラデータが軸)

- **ダメージ計算** = `damage(character, skill, enemy, buff_set) -> DamageResult(トレース付き)`
- **強化提案** = 候補変更(装備・強化・ステ振り)を character に適用して damage を再評価し、差分をランキング
- **ロードマップ** = content の入場条件と character の現状を突き合わせて不足を列挙
- **索引** = 「やりたいこと」→ 必要な機能・コンテンツ・強化への逆引き(gamedata 上のグラフ)

強化提案とロードマップが「計算機能の再利用」で書けるのは、キャラモデルを軸にした構造の直接の利点。

## 検証戦略

1. **ユニット**: domain の各段(能力値・カテゴリ集計・式・追加ダメージ)を個別に
2. **トレース比較**: 旧リポ(Excel 由来)と同一入力での中間値比較。差分は「意図した差(wiki 準拠)」か「バグ」かを必ず分類
3. **実測突き合わせ**: ゲーム内の実測ダメージを記録して期待値と比較する機能をアプリ自体に持たせる(最終の正解は実測。旧リポ最大の欠落)
