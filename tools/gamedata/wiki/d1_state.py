r"""D1(tw-wiki)の現状を読み、差分投入(`units.py --state`)の入力になる state.json を書く。

    cd services/api-worker
    python ../../tools/gamedata/wiki/d1_state.py --out state.json           # 本番(--remote)
    python ../../tools/gamedata/wiki/d1_state.py --out state.json --local   # ローカル D1

wrangler.toml が services/api-worker にあるので、そこで実行すること(wrangler の慣習)。
本番(--remote)はあなたの手で流すこと。

unit は 9 万行あるので 1 回で読まず、LIMIT/OFFSET でページ分割する(既定 2 万行ずつ)。
他の表も同じ道で読む(小さければ 1 ページで終わる)。unit_fts は contentless でも rowid だけは
読めるので、rowid 一覧も読んで state に入れる(units.py --state が「unit にはあるのに unit_fts に
無い」「unit_fts にはあるのに unit に無い(孤児)」を検知して自己修復するのに使う)。
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

DB_NAME = "tw-wiki"
PAGE_SIZE = 20_000
_NPX = "npx.cmd" if os.name == "nt" else "npx"  # Windows は npx.cmd を直接叩く(CreateProcess が .cmd を素で実行できないため)


def run_query(sql: str, mode_flags: list[str]) -> list[dict]:
    """`wrangler d1 execute --json --command <sql>` を叩き、結果行のリストを返す。"""
    cmd = [_NPX, "wrangler", "d1", "execute", DB_NAME, *mode_flags, "--json", "--command", sql]
    # wrangler の出力(警告など)に端末の既定コードページで開けないバイトが混じることがあるので、
    # utf-8 を明示し、開けないバイトは落とす(JSON 本体は問題なく utf-8)。
    proc = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace",
                          check=True)
    data = json.loads(proc.stdout)
    return data[0]["results"]


def paginate(table: str, columns: list[str], order_by: str, mode_flags: list[str]) -> list[dict]:
    cols = ", ".join(columns)
    out: list[dict] = []
    offset = 0
    while True:
        rows = run_query(
            f"SELECT {cols} FROM {table} ORDER BY {order_by} LIMIT {PAGE_SIZE} OFFSET {offset}",
            mode_flags,
        )
        out.extend(rows)
        if len(rows) < PAGE_SIZE:
            break
        offset += PAGE_SIZE
    return out


def collect_state(mode_flags: list[str]) -> dict:
    return {
        "unit": paginate("unit", ["id", "rowid", "h"], "rowid", mode_flags),
        # unit_fts は contentless(terms を読み戻せない)だが rowid だけは読める。unit との
        # 食い違い(孤児・欠落)を検知するために別途持つ。
        "unit_fts_rowids": paginate("unit_fts", ["rowid"], "rowid", mode_flags),
        "page": paginate("page", ["name", "h"], "name", mode_flags),
        "wiki_table": paginate(
            "wiki_table", ["page", "anchor", "table_idx", "h"], "page, anchor, table_idx", mode_flags,
        ),
        "correction": paginate("correction", ["id", "h"], "id", mode_flags),
        "alias": paginate("alias", ["name", "page"], "name, page", mode_flags),
        "unit_link": paginate("unit_link", ["unit_id", "page", "ord"], "unit_id, page, ord", mode_flags),
        "column_note": paginate("column_note", ["name", "note", "state_key"], "name", mode_flags),
    }


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--out", type=Path, default=Path("state.json"), help="書き出す state.json の場所")
    p.add_argument("--local", action="store_true", help="ローカル D1(--local)を読む。既定は --remote")
    p.add_argument("--persist-to", type=str,
                   help="wrangler の --persist-to(ローカル D1 の保存先を変えたいとき。開発・確認用)")
    a = p.parse_args()

    mode_flags = ["--local" if a.local else "--remote"]
    if a.persist_to:
        mode_flags.append(f"--persist-to={a.persist_to}")
    state = collect_state(mode_flags)
    a.out.write_text(json.dumps(state, ensure_ascii=False), encoding="utf-8")

    for name, rows in state.items():
        print(f"{name}: {len(rows)}", file=sys.stderr)
    print(f"out: {a.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
