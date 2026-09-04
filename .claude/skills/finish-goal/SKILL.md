---
name: finish-goal
description: ひとまとまりの作業(goal)の実装が終わったときの締め作業。テスト確認、ADR への決定記録、進捗、実機確認、commit 準備までの定型チェックリスト。「goal を締めて」「完了処理して」「仕上げて」と頼まれたときに使う。
---

# goal の締め作業

順に確認し、抜けがあれば埋める。すべて Small 扱い(Subagent は起動しない)。

1. **テスト**: `cargo test --workspace` と `cd apps/desktop && npm run build && npx svelte-check` を実行し、件数と pass/fail を控える(0 errors / 0 warnings が基準)
2. **決定記録** `docs/adr/`: 実装中に決めた事項を該当テーマの ADR に「決定 / 却下した選択肢 / 経緯」の形式で追記する(既存の書式に合わせる)。新しいテーマなら連番で ADR を新規作成し、`docs/adr/README.md` の索引に 1 行足す。作業ログ・テスト件数・スクリーンショット参照は書かない
3. **実機確認**: UI を変えた goal なら `gui-smoke` で確認する(スクリーンショットは一時ディレクトリに出し、リポには残さない)。Subagent に出すなら `smoke-tester`
4. **構成文書**: クレート・モジュール・画面構成を変えたなら docs/architecture.md を更新する
5. **diff 確認**: `git status` / `git diff --stat` で意図しないファイル(スクラッチ、DB、ログ)が混ざっていないか見る
6. **commit はユーザーの指示があるときだけ。** 代わりに、コミットメッセージ案(1 行 + 箇条書き)を提示する
8. ユーザーに「この goal が終わったら `/clear` で次を始める」ことを一言添える
