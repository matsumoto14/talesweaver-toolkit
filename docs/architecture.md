# アーキテクチャ

方針: **構造ファースト**。ゲームのドメインモデルを中心に置き、5つの機能(ダメージ計算・キャラ管理・強化提案・ロードマップ・やりたいこと索引)はすべて同じモデルの消費者として実装する。

## 全体像

```
┌─────────────────────────────────────────────┐
│ apps/desktop (Tauri シェル)                   │
│   src/ … フロントエンド(TS)。表示と入力のみ     │
│   commands … domain への薄いアダプタ           │
└──────────────┬──────────────────────────────┘
               │
┌──────────────┴───────────┐  ┌────────────────┐
│ crates/domain             │←─│ crates/gamedata │
│ 純粋なドメインモデル+計算   │  │ 静的データの型と  │
│ I/O なし・決定的           │  │ ローダ(wiki由来) │
└──────────────┬───────────┘  └───────┬────────┘
               │                      ↑ 生成
┌──────────────┴───────────┐  ┌───────┴────────┐
│ crates/storage            │  │ tools/scraper ※ │
│ SQLite(ユーザーデータのみ) │  │ talewiki 取込み  │
└──────────────────────────┘  └────────────────┘
```

## クレート構成と責務

### crates/domain — ドメインモデルと計算(核)

- **モデル**: `Character`(素ステ・装備・スキル構成・覚醒/エタ・バフセット)、`Equipment`、`Skill`、`Buff`、`Enemy`、`Content`(入場条件)
- **計算**: 能力値計算 → カテゴリ集計 → 与ダメージ式 → 段数・追加ダメージ の4段パイプライン(docs/damage-formula.md §8)
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
- domain の型との変換はここで行う。domain は SQLite を知らない

### apps/desktop — Tauri シェル

- コマンドは「storage/gamedata から読む → domain を呼ぶ → 結果を返す」だけの薄いアダプタ。ロジックを書かない
  - 未実装要素の中立値(装備・属性など)もコマンドに書かない。`DamageInput::new` のように domain 側へ集約し、実装時にそこを引数へ昇格させる
- UI は表示と入力のみ。計算・判定は必ず Rust 側

フロントエンドの階層(`apps/desktop/src/`):

```
main.ts, App.svelte  エントリと画面枠(ナビ・エラー帯)
api/types.ts         Tauri コマンドの入出力型。Rust の serde 構造体の写し(手動同期)
api/commands.ts      invoke ラッパー
ui/                  画面によらない汎用部品(Select, StatInput, Splitter, persistedState)
pages/<機能>/        機能ごとの画面と、その画面専用の部品
format.ts            数値整形
labels.ts            ステータスの表示名・並び順
toast.svelte.ts      エラー帯の共有状態
```

機能を足すときは `pages/<機能>/` を作る。2 つ以上の機能で使う部品だけを `ui/` に上げる。
SvelteKit は使っていないため `src/lib/` は置かない(`$lib` エイリアスがなく、階層が 1 段深くなるだけ)。

数値入力は `ui/StatInput.svelte`(ラベル|数値欄|range スライダー|MAX ボタン)1 種類に統一する(docs/goals/2026-08-21-character-screen-v2.md)。旧 `Stepper`/`NumberField` の 2 部品は廃止済み(「入力方式は 1 種類」という CLAUDE.md の UX 方針を部品数で強制する)。

画面レイアウトは可変にする(2026-08-21)。`ui/persistedState.svelte.ts`(`persisted(key, initial)`: localStorage に永続化する `$state` ラッパー。呼び出し元コンポーネントの `<script>` 初期化中に呼ぶ)と `ui/Splitter.svelte`(グリッドの列境界に置く、ドラッグ・ダブルクリック・矢印キーで列幅を変える縦区切り線)の組み合わせで、サイドバー(`App.svelte`)の折りたたみとダメージ計算・キャラ管理の列幅リサイズを実装する。各画面のグリッドは列幅を `grid-template-columns` の動的文字列で組み立て、区切り線トラック(6px)を明示的な grid カラムとして持つ(旧来の `gap:1px; background:var(--border)` によるトラックレス区切りから変更)。詳細は docs/decisions.md 参照。

例(`pages/character/`、docs/ux-guidelines.md「作成と詳細設定を分離する」の適用、2026-08-21 に「一覧|キャラデータ|設定」の 3 カラムへ再構成): `CharacterPage.svelte`(一覧+登録の入口。左カラム、マスター・ディテール)/ `CharacterRegisterForm.svelte`(名前+キャラ種のみの最小登録)/ `CharacterWorkspace.svelte`(選択キャラの編集 draft を 1 つの `$state` にまとめて持つ外枠。`{#key character.id}` で作り直され、`preview_effective_stats` を debounce 呼び出しして即時プレビューを保持する)/ `CharacterData.svelte`(中央カラム。名前・キャラ種・覚醒と、能力値表「ステ|素|補正|最終」・補正の内訳)/ `CharacterSettings.svelte`(右カラム。恒常補正/常用バフ/キャラスキル/調整のアコーディオン。専門用語(層名)はここに出さず、内訳表示にのみ出す)/ `draft.ts`(3 部品で共有する編集中ドラフトの型と組み立て関数)の構成にする。

### tools/scraper — talewiki 取り込み(※未実装)

- EUC-JP PukiWiki の取得(`cmd=source`)→ パース → gamedata の生成。手法は docs/damage-formula.md 取得メモと旧リポの scrapeSkills.js を参照
- 生成物との差分検出(wiki 更新の検知)をここで行う

## 依存の向き

```
apps/desktop → storage, gamedata, domain
storage      → domain(型のため)
gamedata     → domain(型のため)
domain       → (何にも依存しない)
tools/scraper→ gamedata(スキーマ共有)
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
