# 進捗状況

1 goal 1 行。詳細は各 goal 文書と `git log`。

- 2026-08-21 [最小 E2E](claude/goals/2026-08-21-minimal-e2e.md): キャラ登録 → スキル・敵選択 → ダメージ計算 + トレースが動く。装備・属性は未実装(中立値)
- 2026-08-21 [キャラステータス補正源](claude/goals/2026-08-21-character-stat-sources.md): ペット S スキル・ルーン・クラウン・神鳥の聖物・常用バフ・調整値を登録でき、ダメージ計算が自動反映してステトレースに寄与内訳を出す。装備値側(称号・装備アビリティ等)は未実装
- 2026-08-21 画面レイアウトの可変化: サイドバー折りたたみ・列リサイズ(docs/claude/decisions.md「画面レイアウトの可変化」)
- 2026-08-21 [キャラ画面の UX ガイドライン適用](claude/goals/2026-08-21-ux-guidelines-character-screen.md): 登録(名前+キャラ種のみ)と詳細設定を分離、ダメージ計算に一時調整の経路を追加
- 2026-08-21 [キャラ画面 v2](claude/goals/2026-08-21-character-screen-v2.md): 「一覧|キャラデータ|設定」の 3 カラム、数値入力を `StatInput` に統一、調整値は加算/固定の 2 種、保存前の即時プレビュー。gamedata にプレイアブル 19 キャラ・キャラスキルバフ 9 件。実機検証は docs/screenshots/20〜33
- 2026-08-22 PR レビュー指摘の修正: storage の自動マイグレーション、値域検証の domain 一本化、`get_stat_limits`、調整エディタ共通化(docs/claude/decisions.md「PR レビュー指摘の修正」)
