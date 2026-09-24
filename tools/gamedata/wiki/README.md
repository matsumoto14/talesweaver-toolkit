# Tale Wiki 取込パイプライン

docs/wiki-import-plan.md の **層 [1](取込キャッシュ)**。派生データの正本で、
この先の構造化抽出・索引・整形表示は全部ここから作る。

```
python tools/gamedata/wiki/sync.py --full    # 初回。3,439 ページ・約 9 分
python tools/gamedata/wiki/sync.py           # 以降。RecentChanges の更新分だけ
python tools/gamedata/wiki/sync.py --status  # 取らずに中身を見る
python -m unittest discover -s tools/gamedata/wiki -t tools/gamedata/wiki
```

| ファイル | 役目 |
|---|---|
| `talewiki.py` | 取得と解析。EUC-JP + NEC 拡張の復元、`?cmd=source` / `?cmd=list` / RecentChanges |
| `store.py` | SQLite(`page` / `sync_run`)。最後に成功した版を必ず残す |
| `sync.py` | 1 コマンド。差分 / 全件 / 状態表示 |
| `units.py` | `wiki.sqlite` → `out/units.sql`(D1 投入用)。全件・差分(`--state`)の両方 |
| `d1_state.py` | 本番(または対象の D1)の現状を読み、`units.py --state` の入力(state.json)を書く |

`.claude/skills/talewiki-fetch/scripts/fetch_page.py` は 1 枚だけ見たいとき用の薄い口で、
中身は `talewiki.py` を呼ぶ。復号の実装をここと二重に持たない。

## 置き場所

既定 `%LOCALAPPDATA%\tw-context-wiki\wiki.sqlite`、`TW_WIKI_CACHE` か `--cache` で変えられる。
**リポには入れない**(約 27 MB)。アプリ本体の `%APPDATA%\dev.twcontext.app\tw-context.sqlite`
とは別物。素の SQLite で拡張を使わないので、この先サーバー側に移すとき
Cloudflare D1 にそのまま載る。

## 実測で分かったこと(2026-09-20)

- **更新検出に `?cmd=rss` は使えない**。15 件(約 2 日分)しか返らない。
  RecentChanges のソースは同じ日時を **約 500 件**持ち 2024-01 まで遡れるので、そちらを使う
- **`?cmd=list` の href は空白を `+` にする**。`Chapter/Secret+Chapter` の実名は
  `Chapter/Secret Chapter` で、`+` のまま `?cmd=source` に投げると `<pre>` が返らない。
  空白を含むページは 66 件あるので、`unquote_plus` で戻さないとその全部を取りこぼす
- ページ数は **3,438**。`?cmd=list` の body 内 href は 3,443 個だが、
  ナビ 4 個と重複 1 個を含む(計画の「3,443」はこれを数えていた)
- **`?cmd=list` に載らない生きたページがある**。`FrontPage` が実際に載らない。だから一覧に無いだけで
  消えたとは決めず、取得で確かめて `<pre>` が返らないときだけ `gone` にする。取込対象は
  一覧 + RecentChanges の和で、本文があるページは **3,439**(一覧 3,438 + `FrontPage`)
- **RecentChanges には wiki 側が壊れた名前で記録した行がある**(`???á?ó?È/1LineBBS`、2024-11-15)。
  こういう `missing` / `gone` は mtime が動くまで取りに行かない(毎回失敗し続けるため)
- **ソース総量は約 15 MB**(SQLite のファイルは約 27 MB)。計画の「34 MB」は 25 ページの平均 10 KB からの
  外挿で、実際の平均は 4.4 KB だった
- 削除・改名は RecentChanges に出ない。`--full` の `?cmd=list` 突き合わせでしか分からず、
  消えたページは `status = 'gone'` にして**本文は残す**(改名なら新名で入り直す)

## status の意味

| 値 | 意味 |
|---|---|
| `new` | 一覧に居るが本文はまだ取っていない(`gone` から戻ってきた場合も含む) |
| `ok` | 本文がある |
| `error` | 取得に失敗した。`source` は最後に成功した版のまま |
| `missing` | `?cmd=source` が `<pre>` を返さない(ページ名が違う / 存在しない) |
| `gone` | `?cmd=list` から消えた(削除か改名) |

## 再取得の規則

差分実行は RecentChanges の更新分に加えて、**`new`(未取得)と `error`(一時的な失敗)も必ず拾う**。
`--full` を途中で止めても次の差分実行が残りを埋め、`ok` だったページが通信で落ちても次回戻る。

`missing` / `gone` だけは mtime が動くまで取りに行かない(壊れた名前を永久に叩き続けないため)。
ただし **`--full` で `?cmd=list` に載っていたページは実在が確かめられている**ので、`missing` でも取り直す
(こちら側の取得・復号が原因の失敗から戻れるように)。

`gone` から一覧に戻ったページは古い `source` を持ったまま `new` になるので、
mtime が取れなくても必ず取り直す。

## unit / correction の書き出し(units.py)

```
python tools/gamedata/wiki/units.py [--pages 名前,名前,...] [--limit N] [--state state.json]
```

`wiki.sqlite` の `page` を段落・表の行・訂正のユニットに切り、`out/units.sql` を書き出す
(D1 への流し込みは `wrangler d1 execute` に渡す)。`--pages` は開発用に対象ページを先頭に寄せる
だけで、`--limit` を付けないと全ページ(約 3,400)を処理する。

**`--state` を付けない(既定)と全件(DELETE 全件 → INSERT)。** D1 の無料枠(書き込み 1 日 10 万行)を
必ず超えるので、初回とスキーマ変更時だけ使う。

**`--state <state.json>` を付けると、本番(または対象の D1)の現状と比べた差分だけを書く。**
`state.json` は `d1_state.py` が本番から読んで書く(下記)。全ページを今どおり再計算し(分かち書き
辞書と訂正表は全体依存なので、ページ単位の差分計算では足りない)、最終生成物を state と比べて
消えた行は `DELETE`、変わった行は `INSERT OR REPLACE`、新しい行は `INSERT` だけを出す。`meta` だけは
毎回書き直す。`unit` の rowid は既存 id をそのまま維持し、新しい id は state の最大 rowid + 1 から
連番(state は残っている行しか持たないので、末尾の行が消えれば次回は同じ番号がまた振られうる。
`unit` と `unit_fts` の両方から同時に消えるので実害は無い)。

`--file` の投入が unit_fts と unit の間で止まっても、次回の差分生成で自己修復する: `unit_fts` の
DELETE/INSERT を必ず `unit` の DELETE/REPLACE より先に出す(`unit` が書けずに止まれば、次回は
h の不一致として再検知され、`unit_fts` を消して入れ直すだけで済む)。`d1_state.py` は
`unit_fts` の rowid 一覧も読み、`unit` に無い rowid(孤児。前回 unit_fts だけ書けて unit が
書けなかった、等)を最初に消し、`unit` にはあるのに `unit_fts` に無い rowid(前回 unit だけ
書けて unit_fts が書けなかった)を入れ直す。`unit_fts` は `contentless_delete=1` — 1 行だけの
`DELETE FROM unit_fts WHERE rowid=?` が効く。標準エラーに表ごと・合計の書き込み見積りを出し
(FTS5 の内部索引の書き込みは含まない)、80,000 行を超えたら警告する(流す前に人が見て
止められるように)。

```
cd services/api-worker
python ../../tools/gamedata/wiki/d1_state.py --out state.json           # 本番(--remote)の現状
python ../../tools/gamedata/wiki/d1_state.py --out state.json --local   # ローカル D1 で確認するとき
```

`d1_state.py` は `wrangler.toml` がある `services/api-worker` で実行すること(wrangler の慣習)。
`unit`(9 万行強)は 1 回で読まず、`LIMIT`/`OFFSET` で分割して読む(既定 2 万行ずつ)。本番
(`--remote`)への読み取りはあなたの手で流すこと。

**訂正(`correction`)**は `corrections.json`(git 管理、生成物)から読む。値は
`crates/gamedata` が持つ `CharacterSkillDef` / `BuffDefinition` の効果値から機械で作るので、
gamedata を直した後は必ず作り直す:

```
cargo run -p gamedata --bin export_corrections > tools/gamedata/wiki/corrections.json
```

`units.py` は各訂正について `page` のユニットのうち `text` に `match` を含む**最初の row**を
`unit_id` にする(無ければ NULL のまま、訂正は対象ユニットを持たず単独で候補になる)。

**静的データだけの項目(`app_data.json`、生成物)**は称号・装備カタログのうち wiki に無い・
wiki より新しい値(9 値・上限)を持つ。gamedata を直した後は必ず作り直す(ADR-021):

```
cargo run -p gamedata --bin export_app_data > tools/gamedata/wiki/app_data.json
```

`units.py` はセル 1 つにつき `correction` 行を 1 本作る(`unit_id` は常に NULL。wiki の行とは
結び付けない)。Worker は質問の語が `subject` に完全一致したときだけ、これを 1 件の疑似候補として
候補に足す(段階 3 spec B)。
