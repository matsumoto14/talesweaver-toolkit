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
