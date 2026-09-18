#!/usr/bin/env bash
# 作業の終わりに 2 つだけ見る Stop hook。判定して知らせるだけで、止めはしない。
#
#   1. 保存データの形を変えていないか(SQLite / IndexedDB / 書き出し JSON の 3 か所)
#   2. 変更内容が書き残されているか(CHANGELOG の未リリース節、または news.json)
#
# なぜ要るか: 版を外に出したあとは、保存の形を変えるとユーザーの手元のデータが壊れる。
# 保存は 3 か所あり(docs/adr/008-storage.md / .claude/skills/db-migration)、片方だけ変えると
# ブラウザ版だけキャラタブが開けなくなる、が実際に起きた(2026-09-15)。
# お知らせ・変更履歴は、使う人が変化に気づく唯一の入口なので、書き忘れていないかを見る。
# 置き場所は release skill 手順 5-6 のとおり: ふだんは CHANGELOG の「## [未リリース]」に書き、
# news.json の releases はリリース commit で CHANGELOG から落とす(版に紐づくため)。
set -u
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
git rev-parse --git-dir >/dev/null 2>&1 || exit 0

NEWS=apps/desktop/src/data/news.json

# 実装の差分(docs / .claude / お知らせ自体は数えない)
impl=$(git diff HEAD --name-only -- crates apps/desktop/src apps/desktop/src-tauri 2>/dev/null \
  | grep -v "^$NEWS$")
[ -n "$impl" ] || exit 0

# --- 1. 保存データの形 ---------------------------------------------------------
added=$(git diff HEAD -- \
  crates/storage/src/character_repository.rs \
  apps/desktop/src/api/browserStore.ts \
  apps/desktop/src/api/transfer.ts \
  'crates/domain/src/*.rs' 2>/dev/null | grep '^+' | grep -v '^+++')
hits=""
printf '%s\n' "$added" | grep -qE 'const SCHEMA_VERSION: i64 = ' && hits="${hits}SQLite SCHEMA_VERSION / "
printf '%s\n' "$added" | grep -qE 'const SCHEMA_VERSION = [0-9]+' && hits="${hits}IndexedDB SCHEMA_VERSION / "
printf '%s\n' "$added" | grep -qE 'const FORMAT_VERSION = [0-9]+' && hits="${hits}書き出し FORMAT_VERSION / "
printf '%s\n' "$added" | grep -qE 'fn migrate_|ALTER TABLE|CREATE TABLE' && hits="${hits}migrate / テーブル定義 / "
printf '%s\n' "$added" | grep -qE 'serde\(default' && hits="${hits}JSON 列の欄追加(serde default) / "

msg=""
if [ -n "$hits" ]; then
  msg="保存データの形に触れています(${hits% / })。SQLite / IndexedDB / 書き出し JSON の 3 か所が揃っているか db-migration skill で確かめてください。外に出した版のユーザーデータが壊れます。"
fi

# --- 2. 変更内容が書き残されているか -------------------------------------------
# CHANGELOG は「## [未リリース]」節に行が増えたかを見る(節の見出しだけでは書いたことにしない)。
written=0
git diff HEAD --quiet -- "$NEWS" 2>/dev/null || written=1
if [ "$written" -eq 0 ]; then
  git diff HEAD -- CHANGELOG.md 2>/dev/null \
    | grep '^+' | grep -v '^+++' | grep -qE '^\+[-*] ' && written=1
fi

if [ "$written" -eq 0 ]; then
  count=$(printf '%s\n' "$impl" | wc -l | tr -d ' ')
  msg="${msg}実装を ${count} ファイル変えていますが、変更内容をどこにも書いていません。使う人に見える変化なら CHANGELOG.md の「## [未リリース]」に 1 件足してください(内部だけの変更なら、このまま進めて構いません)。"
fi

[ -n "$msg" ] || exit 0
printf '{"systemMessage":"%s"}\n' "$msg"
