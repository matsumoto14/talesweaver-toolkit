"""wiki を取り込む 1 コマンド。

    python tools/gamedata/wiki/sync.py            # 差分(RecentChanges の更新分だけ)
    python tools/gamedata/wiki/sync.py --full     # 全件。?cmd=list と突き合わせて改名・削除も検出
    python tools/gamedata/wiki/sync.py --status    # 取らずに今の中身だけ見る

初回は `--full` を使う(3,439 ページ・約 9 分)。以降は差分だけで足りる。
取得失敗はページ単位で記録して先に進み、最後に成功した版を残す。
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from store import Store  # noqa: E402
from talewiki import Fetcher, PageMissing  # noqa: E402


def fetch_one(store: Store, fetch: Fetcher, name: str, mtime: str | None) -> bool:
    """1 ページ取って保存する。**1 ページの失敗で全体を止めない**。"""
    try:
        store.save(name, fetch.source(name), mtime)
        return True
    except PageMissing:
        store.fail(name, "<pre> が無い(存在しないか名前が違う)", status="missing", mtime=mtime)
    except Exception as e:
        store.fail(name, str(e), mtime=mtime)
    return False


def plan_targets(store: Store, mtimes: dict[str, str],
                 listed: set[str] | None) -> list[str]:
    """取得を検討するページ名。重複を除いた順序付きリスト。

    - `--full`(`listed` あり): 一覧の全部 + RecentChanges にしか出ないもの
      (`FrontPage` は `?cmd=list` に載らない)
    - 差分(`listed` なし): RecentChanges に出たもの
    - どちらでも **`store.unfinished()`**(未取得・一時的に失敗)を足す。
      `--full` を途中で止めても次回が残りを埋め、通信で落ちたページも次回拾い直す
    """
    out: list[str] = []
    seen: set[str] = set()
    for group in ((sorted(listed) if listed else []), mtimes, store.unfinished()):
        for name in group:
            if name not in seen:
                seen.add(name)
                out.append(name)
    return out


def sync(full: bool, limit: int | None, delay: float, cache: Path | None) -> int:
    store = Store(cache)
    fetch = Fetcher(delay=delay)
    mode = "full" if full else "recent"
    run = store.start_run(mode)
    print(f"cache: {store.path}")

    # RecentChanges は更新日時の出どころ。全件のときも使う(どのページが新しいか分かる)
    mtimes = fetch.recent_changes()
    print(f"RecentChanges: {len(mtimes)} 件")

    listed: set[str] | None = None
    if full:
        names = fetch.page_names()
        listed = set(names)
        print(f"?cmd=list: {len(names)} ページ")
        for name in names:
            store.seen(name, mtimes.get(name))
        store.db.commit()

    targets = plan_targets(store, mtimes, listed)
    todo = [
        n for n in targets
        # `--full` で一覧に載っていたページは実在が確かめられているので、
        # `missing` でも取り直す(こちら側の取得・復号が原因の失敗から戻れるようにする)
        if store.needs_fetch(n, mtimes.get(n), retry_missing=listed is not None and n in listed)
    ]
    if limit:
        todo = todo[:limit]
    print(f"取得対象: {len(todo)} / {len(targets)}")

    fetched = failed = 0
    for i, name in enumerate(todo, 1):
        if fetch_one(store, fetch, name, mtimes.get(name)):
            fetched += 1
        else:
            failed += 1
        if i % 100 == 0 or i == len(todo):
            store.db.commit()
            print(f"  {i}/{len(todo)} (失敗 {failed})", flush=True)
    store.db.commit()

    gone: list[str] = []
    if listed is not None:
        # 一覧に無いものは取得で確かめる。取れたら生きている(`FrontPage` は ?cmd=list に載らない)。
        # `<pre>` が返らないときだけ消えた(削除か改名)とみなす
        unlisted = store.unlisted(listed)
        if limit:
            unlisted = unlisted[:limit]
        if unlisted:
            print(f"一覧に無い: {len(unlisted)} — 取得で確かめる")
        for name in unlisted:
            fetch_one(store, fetch, name, mtimes.get(name))
            if store.status(name) == "missing":
                store.mark_gone(name)
                gone.append(name)
        store.db.commit()
        if gone:
            print(f"消えた(削除か改名): {len(gone)} — {gone[:10]}")

    store.finish_run(run, listed=len(listed) if listed else None,
                     fetched=fetched, failed=failed, gone=len(gone))
    print(f"完了: 取得 {fetched} / 失敗 {failed} / gone {len(gone)}")
    print(f"状態: {store.counts()}")
    store.close()
    return 1 if failed and not fetched else 0


def show_status(cache: Path | None) -> int:
    store = Store(cache)
    print(f"cache: {store.path}")
    print(f"状態: {store.counts()}")
    for r in store.db.execute(
        "SELECT * FROM sync_run ORDER BY id DESC LIMIT 5"
    ):
        print(f"  #{r['id']} {r['mode']} {r['started_at']} → {r['finished_at']} "
              f"listed={r['listed']} fetched={r['fetched']} failed={r['failed']} gone={r['gone']}")
    for r in store.db.execute(
        "SELECT name, status, error FROM page WHERE status IN ('error','missing') LIMIT 20"
    ):
        print(f"  ! {r['name']}: {r['status']} {r['error']}")
    store.close()
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--full", action="store_true", help="?cmd=list 全件。改名・削除を検出する")
    p.add_argument("--status", action="store_true", help="取得せず今の中身を出す")
    p.add_argument("--limit", type=int, help="取得するページ数の上限(試すとき用)")
    p.add_argument("--delay", type=float, default=0.2, help="1 ページごとの最短間隔(秒)")
    p.add_argument("--cache", type=Path, help="SQLite の置き場所(既定は TW_WIKI_CACHE)")
    a = p.parse_args()
    if a.status:
        return show_status(a.cache)
    return sync(a.full, a.limit, a.delay, a.cache)


if __name__ == "__main__":
    raise SystemExit(main())
