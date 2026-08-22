# Claude Code 運用ガイド

CLAUDE.md「実行ワークフロー」の詳細と、ユーザー側の運用推奨。背景は docs/claude/decisions.md「2026-08-22 Claude Code エージェント運用の整理」。

## Agent 定義

`~/.claude/agents/`(ユーザー単位)に置く。すべて `disallowedTools: Agent`(再委譲禁止)。

| Agent | model / effort | 用途 |
|---|---|---|
| researcher | Sonnet / high | Complex 変更の実装前調査。ファイル変更不可 |
| implementer | Sonnet / medium | 承認済みスコープの実装と関連テスト |
| reviewer | Sonnet / high | Complex 変更の独立レビュー。ファイル変更不可 |
| smoke-tester | Sonnet / medium | Tauri 実機の GUI 操作・撮影(WebView2 CDP + Playwright) |
| Explore(組み込み) | — | ファイル探索・シンボル検索(機械的な探索は Haiku) |

`~/.claude/settings.json` の `env.CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH="2"` でネストを 2 段に制限(既定 3)。`/code-review` の内部 fan-out は 2 段で動く。

## Skills(`.claude/skills/`、必要なときだけ読み込まれる)

| skill | 用途 |
|---|---|
| talewiki-fetch | Tale Wiki(EUC-JP)のページソース取得スクリプトと落とし穴(NEC 拡張文字) |
| gui-smoke | Tauri 実機の起動スクリプト・Playwright テンプレート・撮影規約。smoke-tester にプリロード |
| finish-goal | goal 完了時のチェックリスト(テスト・決定記録・status・スクリーンショット) |

## 3 段階の判断基準

| 段階 | 例 | 手順 |
|---|---|---|
| Small | 数ファイル内の修正、文言・UI 微調整、テスト追加、docs、`/code-review` 指摘対応 | Fable 自身が実装 → 関連テスト → diff 確認。必要なら Explore |
| Normal | 単一 domain/module 内の機能追加・修正 | Fable が受け入れ条件を整理(+Explore)→ 規模が大きいときのみ implementer → Fable が diff・テストを評価。reviewer は迷う箇所に限定 |
| Complex | アーキテクチャ変更、複数 module 横断、DB migration、認証・認可・セキュリティ、並行処理、破壊的操作、データ整合性、大規模リファクタリング、原因不明の障害 | researcher → Fable 方針決定 → implementer → reviewer → Fable 最終判定。重大指摘は implementer へ差し戻し |

## レビューの重複禁止

- `reviewer` と `/code-review` を同一変更に重ねない。`/code-review`(ユーザー起動)は内部で指摘を検証済みなので、Fable が直接評価し Small として対応する。
- 追加レビューは観点が異なる場合(security / migration / architecture)のみ。

## Subagent への依頼と返答

- 依頼: 目的・受け入れ条件・対象ファイル・制約のみ。CLAUDE.md の内容は繰り返さない(Subagent も CLAUDE.md を読む)。
- 返答: 結論・根拠・変更/該当ファイル・テスト結果の要約・残存リスクのみ。ログ・探索結果・diff 全文は返さない。

## Context 管理(ユーザー操作の推奨)

- goal(docs/claude/goals/)1 本を完了して commit したら `/clear`。1 セッションに複数 goal を載せない。
- context が 150k を超えたら区切りで `/compact`(自動 compact は上限付近でしか動かず、1M context ではほぼ発動しない)。
- 放置したセッションを再開するときは、続きが本当に必要か考え、必要なら最初に `/compact`。
- `/code-review` は専用セッションで起動する(内部で 17 本前後の Agent が起動する)。

## usage で確認する指標

- メインの Agent 起動回数と `~/.claude/projects/<repo>/<session>/agent-*.jsonl` の本数が 1:1 に近いか(再帰が消えているか)
- implementer の比率と 1 起動あたりの turn 数
- general-purpose が `/code-review` 内部以外でほぼゼロか
- セッション最大 context とセッション長
