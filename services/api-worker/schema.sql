-- Tale Wiki 取込の D1 スキーマ(段階 0)。source_kind の docs/adr/013-wiki-import.md「本文は持たない」を担保する。
-- 正本は手元の wiki.sqlite。初回(と schema 変更時)は units.py が丸ごと再生成する(DELETE 全件 → INSERT)。
-- 以降は units.py --state が本番の現状(h 列)と比べて差分だけ書く(無料枠 1 日 10 万行に収めるため)。

-- 取込キャッシュの page(tools/gamedata/wiki/store.py)から source(本文)を除いたもの。
-- url は出典リンクの組み立て用(page 名の EUC-JP percent-encoding は units.py 側で作る。Worker は
-- page.url + '#' + anchor を連結するだけ)。
CREATE TABLE page (
  name       TEXT PRIMARY KEY,
  url        TEXT NOT NULL,
  mtime      TEXT,
  fetched_at TEXT,
  checked_at TEXT,
  status     TEXT NOT NULL,
  error      TEXT,
  h          TEXT                            -- 差分投入用。行の全列をまとめたハッシュ(units.py row_hash)
);

CREATE TABLE unit (
  id        TEXT PRIMARY KEY,             -- p:ページ/アンカー/連番  /  r:ページ/アンカー/表番号/キー
  kind      TEXT NOT NULL,                -- paragraph | row
  page      TEXT NOT NULL REFERENCES page(name),
  section   TEXT NOT NULL,                -- 見出し文。階層は " › " で連結
  anchor    TEXT NOT NULL,                -- 見出しのアンカー(出典 URL の #)
  ord       INTEGER NOT NULL,             -- ページ内の出現順。候補の自然順
  truncated INTEGER NOT NULL DEFAULT 0,   -- paragraph のみ。断片の先に続きがあるか(「続きを wiki で読む」)
  text      TEXT NOT NULL,                -- paragraph: 冒頭の断片(最大 3 文・200 字) / row: 「列名: 値 | …」(検索・表示用)
  table_idx INTEGER,                      -- row のみ。節内の表番号
  group_key TEXT,                         -- 塊のキー。表の行は page/anchor/table_idx、箇条書きの項目は page/anchor/list_idx。単独の段落は NULL
  row_key   TEXT,                         -- row のみ。主要列の値の組(進0-強1)か内容ハッシュ
  cells     TEXT,                         -- row のみ。JSON {"進化":"0","成功率":"100%",…} セル文字列そのまま
  nums      TEXT,                         -- row のみ。JSON {"進化":0,"Lv":[60,69]} 数値か [下限, 上限]。数値化できた列だけ
  h         TEXT                          -- 差分投入用。行の全列 + その unit の FTS terms をまとめたハッシュ
);
CREATE INDEX unit_page_ord ON unit(page, ord);
CREATE INDEX unit_table    ON unit(page, anchor, table_idx, ord);

CREATE TABLE wiki_table (                 -- 表 1 つ = 1 行
  page TEXT NOT NULL, anchor TEXT NOT NULL, table_idx INTEGER NOT NULL,
  caption TEXT,
  columns TEXT NOT NULL,                  -- JSON ["進化","強化","成功率",…]
  key_columns TEXT NOT NULL,              -- JSON ["進化","強化"]。row_key と「行は選ばれた列 + キー列」に使う
  default_columns TEXT NOT NULL,          -- JSON。先頭から最大 4 列
  row_count INTEGER NOT NULL,
  h TEXT,                                 -- 差分投入用。行の全列をまとめたハッシュ
  PRIMARY KEY (page, anchor, table_idx)
);

CREATE TABLE column_note (                -- 列名辞書。手書き(tools/gamedata/wiki/column_notes.json)から投入
  name TEXT PRIMARY KEY,
  note TEXT NOT NULL,                     -- "強化成功の確率"
  state_key TEXT                          -- "evolution" / "level" / NULL
);

CREATE TABLE alias (name TEXT NOT NULL, page TEXT NOT NULL, PRIMARY KEY (name, page));   -- 別名 → ページ名。ページ名自身も入れる。パスの末尾は複数ページで共有しうる
CREATE TABLE unit_link (unit_id TEXT NOT NULL, page TEXT NOT NULL, ord INTEGER NOT NULL);   -- [[リンク]] の出現順。「次の一手」のチップの材料
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);         -- synced_at / unit_count / schema_version / imported_at

CREATE TABLE correction (                 -- wiki の外から来る訂正。段階 0 では空のまま(表だけ作る)
  id          TEXT PRIMARY KEY,           -- c:対象ユニットの ID/列名。対象が無い項目は c:名前/列名
  subject     TEXT NOT NULL,              -- スキル名/バフ名。get_app_data(name) は subject = name OR unit.page = name
  unit_id     TEXT,                       -- 対象の unit.id。wiki に項目が無ければ NULL(単独で候補になる)
  col         TEXT NOT NULL,
  value       TEXT NOT NULL,              -- 訂正後の値(表示用の文字列)
  grade       TEXT NOT NULL,              -- confirmed | apparent(自動検出で未確認。初期は confirmed だけを入れる)
  source_kind TEXT NOT NULL,              -- notice | client_db | gamedata
  source_title TEXT NOT NULL,
  source_url  TEXT,                       -- 公式お知らせは必須。静的データは NULL でよい
  section     TEXT,                       -- unit_id を持たない行(app_data)の疑似候補用の節名。他は NULL(unit.section を引ける)
  h           TEXT,                       -- 差分投入用。行の全列をまとめたハッシュ
  CHECK (source_kind <> 'notice' OR source_url IS NOT NULL)
);
CREATE INDEX correction_unit ON correction(unit_id);

CREATE TABLE reaction (                   -- ユーザーのリアクション。段階 0 では空のまま(表だけ作る)
  id         INTEGER PRIMARY KEY,
  day        TEXT NOT NULL,               -- 日付まで。時刻は持たない
  answer_id  TEXT NOT NULL,
  kind       TEXT NOT NULL,               -- helpful | wrong | value_wrong
  reason     TEXT,                        -- wrong のチップ: off_topic | outdated | unclear
  unit_ids   TEXT NOT NULL,               -- JSON。答えに出ていたユニット
  unit_id    TEXT, col TEXT,              -- value_wrong のみ
  claim      TEXT, note TEXT,             -- value_wrong のみ。正しいと思う値と根拠(任意、長さ上限つき)
  question   TEXT,                        -- 「質問文も一緒に送る」を選んだときだけ
  status     TEXT NOT NULL DEFAULT 'new'  -- new | confirmed | rejected(メンテナが付ける)
);

CREATE TABLE ask_log (                    -- /ask の記録(質問と答え。運営が改善と費用の把握に使う)
  id         INTEGER PRIMARY KEY,
  at         TEXT NOT NULL,               -- ISO 時刻(UTC)
  user       TEXT NOT NULL,               -- 端末 ID のハッシュ(auth.ts userHash)。端末を跨いで同じ人かは分からない
  question   TEXT NOT NULL,
  qkey       TEXT,                        -- 答えのキャッシュのキー(cache.ts questionKey、語の集合)。続きの質問は NULL
  prev_page  TEXT,                        -- 続きの質問なら直前のページ
  state      TEXT NOT NULL,               -- JSON(level / evolution。装備の中身は来ない)
  kind       TEXT NOT NULL,               -- answer | none | error
  reason     TEXT,                        -- none の理由 / error の文
  route      TEXT,                        -- cheap | loop | cheap_then_loop
  answer_id  TEXT,                        -- reaction.answer_id と突き合わせる
  lead       TEXT,                        -- 結論文(LLM が書いた 1 文)
  body       TEXT NOT NULL,               -- 応答 JSON の全文
  ms         INTEGER NOT NULL,            -- 所要時間
  understanding TEXT,                     -- 理解(1 回目)の出力 JSON(管理画面専用。migrations/003)
  candidates TEXT                         -- LLM に見せた候補 [{id,page,section}](管理画面専用。migrations/003)
);
CREATE INDEX ask_log_user ON ask_log(user, at);

CREATE TABLE ask_call (                   -- /ask 1 回につき、LLM への往復ごとに 1 行(管理画面専用。migrations/003)
  id                    INTEGER PRIMARY KEY,
  ask_log_id            INTEGER NOT NULL,
  seq                   INTEGER NOT NULL,
  kind                  TEXT NOT NULL,     -- understand | select | loop
  model                 TEXT NOT NULL,
  input_tokens          INTEGER NOT NULL,
  cache_read_tokens     INTEGER NOT NULL,
  cache_creation_tokens INTEGER NOT NULL,
  output_tokens         INTEGER NOT NULL,
  ms                    INTEGER NOT NULL
);
CREATE INDEX ask_call_ask_log_id ON ask_call(ask_log_id);

-- 検索用の索引。contentless(本文を保持しない。ADR-013)、detail='full'。terms は取込時に重複を落としてソートした語の集合なので、位置情報があっても本文(語順)は復元できない。detail='none' / 'column' では bm25 が常に 0 で順位が付かない(実測 2026-09-22)。
-- terms は「ページ名 › 節名」+ 本文を正規化・分かち書きした語を、重複を落として空白区切りにしたもの。
-- contentless_delete=1: 差分投入で unit 1 行だけ消す/差し替えるときに `DELETE FROM unit_fts WHERE rowid=?` が効く
-- ようにする(ローカル D1 で確認済み。消した行は MATCH に出ない)。unit.rowid と常に一致させる(retrieve.ts の JOIN)。
CREATE VIRTUAL TABLE unit_fts USING fts5(
  terms,
  content='',
  tokenize='unicode61',
  detail='full',
  contentless_delete=1
);
