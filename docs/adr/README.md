# ADR(テーマ別決定記録)

goal 文書と時系列の決定ログ(decisions.md)を 2026-08-28 にテーマ単位へ再編したもの。
1 テーマ 1 ファイル。新しい決定は該当テーマに追記し、テーマが無ければ連番で新規作成してここに 1 行足す。
過去の経緯の全文は git 履歴(docs/claude/goals/、docs/claude/decisions.md)を参照。

- [001 ドメインモデルと構造ファースト](001-domain-model.md) — 素ステ/最終能力値の上限、補正源のレイヤー、カテゴリ enum の器
- [002 ダメージ計算のカテゴリ構成と出典](002-damage-formula-sources.md) — カテゴリ別の式・出典・`[仮]` の残件、補正源(称号/OP/マスタリー等)
- [003 敵・コンテンツカタログとデータ出典](003-enemy-content-catalog.md) — 実測表を正とする格付け、入場条件の swiki 化
- [004 装備モデル](004-equipment-model.md) — 12 部位・9 値・登録一覧+選択中 ID、エンチャント/アビリティ/AF
- [005 シエナのオーラとテシスコア](005-siena-thesis-core.md) — スロット積み上げ入力、独立登録、地域別 6 枠とセット効果
- [006 UI デザインシステム](006-ui-design-system.md) — v4 準拠、トークン 4 段、アイコン規格、適合ループ
- [007 入力 UX](007-input-ux.md) — 自動値優先、入力 5 形態、StatInput 統一
- [008 storage と migration 方針](008-storage.md) — rusqlite、user_version + 列実在確認、v1→v8、バックアップ
- [009 一般公開・配布](009-public-release.md) — NOTICE、TW Context、配布 CI + R2、問い合わせ中継、CSP
- [010 エージェント運用](010-agent-workflow.md) — Small/Normal/Complex 3 段階と再委譲禁止の背景
- [011 ソウルリンク](011-soul-link.md) — リンクステータス 1〜4 をキャラ単位で保持し、装備基本能力へ直加算
- [012 デスクトップとブラウザの両対応](012-web-build.md) — 境界は commands.ts 1 枚、実体は alias で差し替え。計算は共通 crate・保存は各プラットフォーム
