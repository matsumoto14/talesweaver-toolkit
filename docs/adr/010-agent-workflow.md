# ADR-010: エージェント運用

- ステータス: 採用
- 期間: 2026-08-22
- 元文書: decisions.md「Claude Code エージェント運用の整理」(git 履歴参照)

## 背景

- 2026-08-21 時点では CLAUDE.md の `researcher`/`implementer`/`reviewer` に専用エージェント定義が無く、general-purpose エージェントをその役割で代用していた(定義ファイル未整備)。
- CLAUDE.md の実行ワークフローが変更規模を問わず一律だったため、Sonnet で動く `implementer` サブエージェントも CLAUDE.md を読み込み、内部で同じ 3 役(researcher → implementer → reviewer)を再帰的に起動していた。直近セッションの transcript(`~/.claude/projects/<repo>/`)集計では、メインからの Agent 起動 19 回に対し Subagent transcript 49 本、implementer 5 回起動に対し 18 本、最大 4 段ネストに達していた。researcher の fork も調査を 6 並列 × 2 段で複製していた。
- 「`/code-review` 指摘 10 件の修正」のような小粒な変更ですら、reviewer(検証)→ implementer(内部で researcher → implementer → reviewer → implementer)→ general-purpose(実機確認)という重いフルワークフローを経ていた。
- `/code-review` は内部で 1 オーケストレータ + 16 Agent(Fable モデル)を起動し指摘を検証済みにもかかわらず、その後 `reviewer` で同じ変更を再検証していた。
- 実機 GUI 確認を general-purpose で行うと親の Fable モデルで動作し、スモークテスト 5 回で Subagent 出力トークンの約 1/4 を占めていた。
- 13.5 時間・3 goal・最大 276k context の長時間セッションが発生した。Claude Code は自動で `/clear` `/compact` を行わない。

## 決定

- Subagent 内では CLAUDE.md の実行ワークフローを適用しない(再委譲禁止)。各エージェント定義に `disallowedTools: Agent` を設定し、CLAUDE.md に「この節はメインセッションにのみ適用」と明記した。
- 変更を Small / Normal / Complex の 3 段階に分類し、researcher → implementer → reviewer のフルワークフローは Complex のみに限定した。
- `reviewer` と `/code-review` を同一変更に重ねない(観点が異なる場合のみ追加レビューを許容)。
- 実機 GUI 確認は専任の `smoke-tester`(Sonnet / medium)に固定し、general-purpose には行わせない。
- `implementer` は effort medium、`researcher`/`reviewer` は high を維持する(実装は受け入れ条件・対象ファイルが依頼文で与えられるため high の余地が小さい一方、調査・独立レビューは Complex 限定なので品質を優先する)。
- Context 管理(goal ごとに `/clear`、150k 超で `/compact`、`/code-review` は専用セッション)を運用推奨とした。

## 経緯

- 2026-08-21 時点で `researcher`/`implementer`/`reviewer` は専用定義が無く general-purpose で代用していたが、同日中に `~/.claude/agents/`(ユーザー単位)へ定義を配置して解消した。
- 上記の再帰起動・重複レビューの実態を transcript 集計から把握し、2026-08-22 にこの整理をまとめて決定した。
- 2026-09-02 メインを Fable 5.1(1M context)へ更新した棚卸しで、Subagent の `model` をフル ID からエイリアス `sonnet` に、Context 管理を手動 `/compact` から `autoCompactWindow` に置き換えた。未ドキュメントの `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH` は外し、再帰ガードは `disallowedTools: Agent` に一本化。2026-08-28 の「実装は必ず implementer に委譲」はコンテキスト温存が目的だったので取り下げ、Small/Normal/Complex の 3 段階を標準に戻した。
