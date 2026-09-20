r"""取込キャッシュ(wiki-import-plan.md の層 [1])。派生データの正本。

素の SQLite で、拡張も JSON1 も使わない。**この先アプリ側がサーバー上で完結する形に
移るとき、Cloudflare D1 は SQLite なのでこのスキーマがそのまま載る**。だから今は
抽象化層を挟まず、置き場所だけ環境変数で差し替えられるようにしてある。

置き場所(リポ外。34 MB あり、再取得に 8 分かかるので使い捨てにしない):
- 既定 `%LOCALAPPDATA%\tw-context-wiki\wiki.sqlite`
- `TW_WIKI_CACHE` があればそれを使う
アプリ本体の DB(`%APPDATA%\dev.twcontext.app\tw-context.sqlite`)とは別物。混ぜない。
"""
from __future__ import annotations

import os
import sqlite3
from datetime import datetime, timezone
from pathlib import Path

SCHEMA = """
CREATE TABLE IF NOT EXISTS page (
    name       TEXT PRIMARY KEY,
    source     TEXT,           -- 最後に取得できた PukiWiki ソース。失敗時は上書きしない
    mtime      TEXT,           -- wiki 側の更新日時(RecentChanges 由来、ISO8601 +09:00)
    fetched_at TEXT,           -- source を取れた時刻(UTC ISO8601)
    checked_at TEXT,           -- 成否によらず最後に見に行った時刻(UTC ISO8601)
    status     TEXT NOT NULL,  -- new(一覧に居るが未取得) / ok / error / missing(ページが無い) / gone
    error      TEXT
);
CREATE INDEX IF NOT EXISTS page_status ON page(status);

CREATE TABLE IF NOT EXISTS sync_run (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    mode        TEXT NOT NULL,  -- full / recent
    started_at  TEXT NOT NULL,
    finished_at TEXT,
    listed      INTEGER,        -- ?cmd=list のページ数(full のときだけ)
    fetched     INTEGER,
    failed      INTEGER,
    gone        INTEGER
);
"""


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def default_path() -> Path:
    env = os.environ.get("TW_WIKI_CACHE")
    if env:
        return Path(env)
    base = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~/.cache")
    return Path(base) / "tw-context-wiki" / "wiki.sqlite"


class Store:
    def __init__(self, path: Path | None = None) -> None:
        self.path = Path(path) if path else default_path()
        if str(self.path) != ":memory:":
            self.path.parent.mkdir(parents=True, exist_ok=True)
        self.db = sqlite3.connect(self.path)
        self.db.row_factory = sqlite3.Row
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.executescript(SCHEMA)
        self.db.commit()

    def close(self) -> None:
        self.db.close()

    # --- 読む ---

    def mtimes(self) -> dict[str, str | None]:
        """ページ名 -> 記録済みの mtime。未取得のページは入らない。"""
        return {r["name"]: r["mtime"] for r in self.db.execute("SELECT name, mtime FROM page")}

    def needs_fetch(self, name: str, mtime: str | None, retry_missing: bool = False) -> bool:
        """取り直すべきか。

        - 知らないページ / `new`(一覧に居るだけ): 取る。**`gone` から一覧に戻ってきたページは
          古い `source` を持ったまま `new` に戻るので、mtime を見ずに必ず取り直す**
        - `error`(たぶん一時的な失敗): 取る
        - `missing`(`<pre>` が返らない)/ `gone`(消えた): **mtime が動くまで取らない**。
          RecentChanges には wiki 側が壊れた名前で記録した行が実際にあり(`???á?ó?È/1LineBBS`、
          2024-11-15)、毎回取りに行っても永久に失敗する。
          ただし `retry_missing`(= `--full` で `?cmd=list` に載っていた = 実在が確かめられた)なら取る
        - `ok`: wiki 側の mtime が記録より新しいときだけ
        """
        row = self.db.execute(
            "SELECT source, mtime, status FROM page WHERE name = ?", (name,)
        ).fetchone()
        if row is None:
            return True
        if row["status"] in ("missing", "gone"):
            if retry_missing:
                return True
            return mtime is not None and (row["mtime"] is None or mtime > row["mtime"])
        if row["status"] in ("new", "error") or row["source"] is None:
            return True
        if mtime is None:
            return False
        return row["mtime"] is None or mtime > row["mtime"]

    def unfinished(self) -> list[str]:
        """本文をまだ持っていない / 一時的に失敗したページ。差分実行でも拾い直す。

        `--full` を途中で止めた場合と、`ok` だったページが通信で落ちた場合の両方を含む。
        `missing` / `gone` は入れない(mtime が動くまで取らないため)。
        """
        return [
            r["name"]
            for r in self.db.execute(
                """SELECT name FROM page
                   WHERE status IN ('new', 'error') OR (source IS NULL AND status = 'ok')"""
            )
        ]

    def status(self, name: str) -> str | None:
        row = self.db.execute("SELECT status FROM page WHERE name = ?", (name,)).fetchone()
        return row["status"] if row else None

    def counts(self) -> dict[str, int]:
        return {
            r["status"]: r["n"]
            for r in self.db.execute("SELECT status, COUNT(*) n FROM page GROUP BY status")
        }

    # --- 書く ---

    def seen(self, name: str, mtime: str | None) -> None:
        """一覧に居ることだけを記録する(本文はまだ取らない)。"""
        self.db.execute(
            """INSERT INTO page(name, mtime, status) VALUES(?, ?, 'new')
               ON CONFLICT(name) DO UPDATE SET
                 mtime = COALESCE(excluded.mtime, page.mtime),
                 status = CASE WHEN page.status = 'gone' THEN 'new' ELSE page.status END""",
            (name, mtime),
        )

    def save(self, name: str, source: str, mtime: str | None) -> None:
        self.db.execute(
            """INSERT INTO page(name, source, mtime, fetched_at, checked_at, status, error)
               VALUES(?, ?, ?, ?, ?, 'ok', NULL)
               ON CONFLICT(name) DO UPDATE SET
                 source = excluded.source, mtime = COALESCE(excluded.mtime, page.mtime),
                 fetched_at = excluded.fetched_at, checked_at = excluded.checked_at,
                 status = 'ok', error = NULL""",
            (name, source, mtime, now(), now()),
        )

    def fail(self, name: str, error: str, status: str = "error",
             mtime: str | None = None) -> None:
        """**最後に成功した版は消さない**。status と error だけ塗り替える。

        mtime も記録する。`missing` を「この版では失敗した」として覚え、同じ版を
        毎回取りに行かないため(`needs_fetch`)。
        """
        self.db.execute(
            """INSERT INTO page(name, mtime, checked_at, status, error) VALUES(?, ?, ?, ?, ?)
               ON CONFLICT(name) DO UPDATE SET
                 mtime = COALESCE(excluded.mtime, page.mtime),
                 checked_at = excluded.checked_at, status = excluded.status,
                 error = excluded.error""",
            (name, mtime, now(), status, error[:500]),
        )

    def unlisted(self, listed: set[str]) -> list[str]:
        """一覧に無い既知のページ。**まだ gone とは決めない**。

        `?cmd=list` は生きているページを全部載せるとは限らない(`FrontPage` が実際に載らない)。
        消えたと決める前に取得で確かめる。
        """
        return [
            r["name"]
            for r in self.db.execute("SELECT name FROM page WHERE status <> 'gone'")
            if r["name"] not in listed
        ]

    def mark_gone(self, name: str) -> None:
        """取得でも確かめたうえで消えたとする(削除か改名)。source は残す。"""
        self.db.execute(
            "UPDATE page SET status = 'gone', checked_at = ? WHERE name = ?", (now(), name)
        )

    # --- 実行の記録 ---

    def start_run(self, mode: str) -> int:
        cur = self.db.execute(
            "INSERT INTO sync_run(mode, started_at) VALUES(?, ?)", (mode, now())
        )
        self.db.commit()
        return int(cur.lastrowid)

    def finish_run(self, run_id: int, **counts: int | None) -> None:
        self.db.execute(
            """UPDATE sync_run SET finished_at = ?, listed = ?, fetched = ?, failed = ?, gone = ?
               WHERE id = ?""",
            (now(), counts.get("listed"), counts.get("fetched"), counts.get("failed"),
             counts.get("gone"), run_id),
        )
        self.db.commit()
