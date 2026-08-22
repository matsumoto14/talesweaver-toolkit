---
name: finish-goal
description: goal(docs/claude/goals/ の 1 件)の実装が終わったときの締め作業。テスト確認、決定記録、進捗、スクリーンショット、commit 準備までの定型チェックリスト。「goal を締めて」「完了処理して」「仕上げて」と頼まれたときに使う。
---

# goal の締め作業

順に確認し、抜けがあれば埋める。すべて Small 扱い(Subagent は起動しない)。

1. **テスト**: `cargo test --workspace` と `cd apps/desktop && npm run build && npx svelte-check` を実行し、件数と pass/fail を控える(0 errors / 0 warnings が基準)
2. **goal 文書** `docs/claude/goals/<date>-<slug>.md`: 受け入れ条件の各項目に達成/未達と確認方法を書く。スコープ外に回したものを「スコープ外」節に残す
3. **決定記録** `docs/claude/decisions.md`: 実装中に仮決めした事項を「**決定** / 理由 / 出典・確認方法」の形式で追記(既存の節の書式に合わせる)。末尾にテスト結果の出典行を書く
4. **進捗** `docs/status.md`: 1 行追記(日付・goal 文書へのリンク・何ができるようになったか)。CLAUDE.md には書かない
5. **実機確認**: UI を変えた goal なら `gui-smoke` で確認し、`docs/screenshots/` を更新する。Subagent に出すなら `smoke-tester`
6. **構成文書**: クレート・モジュール・画面構成を変えたなら docs/architecture.md を更新する。docs/ 直下の md を変えたら `python tools/docs-site/build.py` で docs/site/ を再生成する
7. **diff 確認**: `git status` / `git diff --stat` で意図しないファイル(スクラッチ、DB、ログ)が混ざっていないか見る
8. **commit はユーザーの指示があるときだけ。** 代わりに、コミットメッセージ案(1 行 + 箇条書き)を提示する
9. ユーザーに「この goal が終わったら `/clear` で次を始める」ことを一言添える
