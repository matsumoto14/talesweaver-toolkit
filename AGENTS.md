# TW Context

TalesWeaver(MMORPG)プレイヤー向けデスクトップツール。Tauri(Rust)+ SQLite(rusqlite)+ TypeScript/Svelte。
機能と全体構成は docs/architecture.md。進捗は `git log` と CHANGELOG.md。
docs/ 直下は人向け(利用者・貢献者が読む)。決定記録は docs/adr/(テーマ別 ADR)。エージェント向けの運用はこのファイルと `.claude/skills/` だけ。

## アーキテクチャ(構造ファースト)

- ゲームのドメインモデル(キャラ・装備・スキル・バフ・敵・コンテンツ)を先に立て、ダメージ計算などの機能はその消費者として実装する。計算ファーストにしない。
- 語彙はドメイン用語(wiki のカテゴリ名・ゲーム内名称)。出典(Excel セル名・wiki アンカー)はコメントや docs に置き、型名・関数名に持ち込まない。
- docs/architecture.md を正とする。直書き・レイヤー飛ばしの近道をしない。

## UX

**UI の実装・変更時は必ず docs/ux-guidelines.md(何を出すか)と docs/design-system.html(どう見せるか)を読む。**

- ux-guidelines の要点: 登録キャラデータを軸にし、wiki から取れる値は静的データとして同梱してユーザーに入力させない。入力欄は「自動値を上書きする例外操作」で、初期値は常に埋まっている。
- design-system の要点(出典はデザインモック TW Toolkit Prototype v4。食い違いは v4 が正):
    - **いつでも意識する 5 つ(§00)= 目的**。①視線を動かさない ②要らないものを見せない ③押した場所は動かない ④変わったら動かす ⑤考えさせない。以下の規格はその手段で、**規格に合っているのに画面が良くないときは 5 つのどれかが崩れている**。見た目の崩れ(折り返し・余白・ずれ)は症状なので、幅や余白で症状だけ消さない
    - **UI を変えたら、出す前に §00 の 5 つで自己チェックする**(毎回)。特に **04 変わったら動かす** は落としやすい —
  新しく足した数値・要約・バッジに `use:bump` / `use:flash` が付いているか、実機で `tools/design-audit/live/motion.js` と
  `attention.js` を走らせて確かめる。**新しく作った面が「平たい箱」になっていないか**も見る(面はインセット + ハイライト、
  値は数値書体、状態はバッジ。色で面全体を塗るのは §02 の帯の枠を食う)
- **押した場所は動かない**。クリックした要素の上に何も差し込まない。ドリルダウンは置き換えず右にペインを増やす。重なるもの(候補・ポップオーバー)はレイアウトを押さない。数値欄は `min-width` + `tabular-nums` で桁が増えても幅が変わらない
    - **入力は 5 形態の上から順に試す**(自動 → 段階選択 → チップ → ステッパー → 自由入力)。上で表現できないときだけ下に降りる。「適用」ボタンを挟まず、押した瞬間に結果が動く。上限は値の隣に常設する
    - **動きは「何が変わったか」を認知させるためだけに使う**。変わった要素だけ動かす。同時に変わるものは全部動かす。増減の色は必ず元に戻す。0.5s を超えない
    - 白 = 編集できる面 / インセット = 読み取り専用、水色 = 保存される / ラベンダー = 保存されない、未収録は破線 + `?` で `0` や空白にしない。角丸・アイコンサイズ・状態色は既存の段に収める

## 情報ソース

- ゲーム仕様の一次ソースは [Tale Wiki](https://talewiki.com/)(EUC-JP の PukiWiki)。取得方法は docs/damage-formula.md 末尾。
- 旧リポ `C:\github\private\twtoolkit`(非公開)に Excel 計算器由来の静的データ(スキル・敵・バフ JSON)がある。数値は古い可能性があるため wiki を正とする。

## ビルド・テスト

前提: Rust stable(MSVC)、VS 2022 Build Tools、Node 22。cargo は `%USERPROFILE%\.cargo\bin`(PATH 未登録なら追加)。

- 依存取得: `cd apps/desktop && npm install`
- テスト: `cargo test --workspace`(リポジトリルート)
- フロント: `cd apps/desktop && npm run build && npx svelte-check`
- 開発起動: `cd apps/desktop && npm run tauri dev`
- GUI の実機確認・撮影は `gui-smoke` skill の手順で行う(Subagent に出すなら `smoke-tester`)
- DB: `%APPDATA%\dev.twcontext.app\tw-context.sqlite`

## 原則

- 後方互換性を保たない。互換レイヤー・フォールバックを足さず、古いパスを削除する。
- 現在の要件を満たす最もシンプルな実装を選ぶ。投機的な抽象化・設定・間接化をしない。
- end-to-end で動く最小から始め、動くものの上に積む。
- 複雑さを減らすなら実績あるライブラリを使う。既にある依存を先に使い、型とドキュメントを確認せずに「機能がない」と判断しない。
- アーキテクチャは長期前提で決める。後で置き換える前提のつなぎを受け入れない。

## 実行ワークフロー

メインセッション(Fable 5.1)が司令塔・最終判断者。Subagent は必要なときだけ使う。背景は docs/adr/010-agent-workflow.md。

**IMPORTANT: Subagent として実行されている場合はこの節を無視し、与えられたタスクを自分で直接遂行する。`Agent` を起動しない。**

着手前に変更を分類し、その段階の手順だけを踏む:

1. **Small**(数ファイル内の修正・微調整・docs・`/code-review` 指摘対応): Fable 自身が実装 → 関連テスト → diff 確認。探索は Explore。`researcher` / `implementer` / `reviewer` は起動しない。
2. **Normal**(単一 domain/module 内の機能追加・修正): Fable が受け入れ条件を整理し、規模が大きいときのみ `implementer` に委譲。Fable 自身が diff とテスト結果を評価する。独立 reviewer は必須にしない。
3. **Complex**(アーキテクチャ変更・複数 module 横断・DB migration・セキュリティ・並行処理・破壊的操作・データ整合性・大規模リファクタリング・原因不明の障害): `researcher` → Fable が方針決定 → `implementer` → `reviewer` → Fable 最終判定。

- 同一変更に `reviewer` と `/code-review` を重ねない。追加レビューは観点が異なる場合(security / migration / architecture)のみ。
- Subagent への依頼は目的・受け入れ条件・対象ファイル・制約のみ。返答は結論・根拠・変更ファイル・テスト要約・残存リスクのみ(ログ・diff 全文は返さない)。

### Subagent と Skills

Agent 定義は `~/.claude/agents/`(ユーザー単位)。すべて `disallowedTools: Agent`(再委譲禁止)。

| Agent | model / effort | 用途 |
|---|---|---|
| researcher | Sonnet / high | Complex 変更の実装前調査。ファイル変更不可 |
| implementer | Sonnet / medium | 承認済みスコープの実装と関連テスト |
| reviewer | Sonnet / high | Complex 変更の独立レビュー。ファイル変更不可 |
| smoke-tester | Sonnet / medium | Tauri 実機の GUI 操作・撮影(WebView2 CDP + Playwright) |
| Explore(組み込み) | — | ファイル探索・シンボル検索 |

Skills は `.claude/skills/`(talewiki-fetch / gui-smoke / finish-goal / design-review / release / db-migration)。各 SKILL.md の description が使いどころ。

依頼の作法:

- implementer: 司令塔が wiki 等で裏取り済みの値は「確認済み(出典・値)」と依頼文に書く。`[仮]` かどうかを implementer に判断させない
- reviewer: 司令塔がテスト・build を確認済みなら「再実行不要」と明記し、読解に専念させる
- smoke-tester: 入力値は domain テストと同じ値を依頼文に書く。値を変えると期待結果が一致せず再実行になる

### Context 管理(ユーザー操作の推奨)

- ひとまとまりの作業(goal)を完了して commit したら `/clear`。1 セッションに複数 goal を載せない
- モデルは `fable`(200k)。`fable[1m]` は使わない(200k 超が割増になり `autoCompactWindow` も効かない)
- `/code-review` は専用セッションで起動する(内部で 17 本前後の Agent が起動する)
