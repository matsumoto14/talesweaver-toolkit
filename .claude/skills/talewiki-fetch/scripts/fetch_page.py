"""talewiki.com のページソースを UTF-8 で 1 枚だけ取る CLI(調査用)。

使い方:
    python fetch_page.py <ページ名> [出力ファイル]
    python fetch_page.py ステータス <scratchpad>/status.txt

取得の実体(EUC-JP + NEC 拡張の復元、`<pre>` の抽出)は
`tools/gamedata/wiki/talewiki.py` が持つ。ここはその薄い口。
まとめて取り込むときは `python tools/gamedata/wiki/sync.py` を使う。
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tools" / "gamedata" / "wiki"))

from talewiki import Fetcher, PageMissing  # noqa: E402

if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    try:
        src = Fetcher(delay=0.0).source(sys.argv[1])
    except PageMissing as e:
        raise SystemExit(f"<pre> が見つからない(ページ名の誤りか、ページが存在しない): {e}")
    if len(sys.argv) >= 3:
        with open(sys.argv[2], "w", encoding="utf-8", newline="\n") as f:
            f.write(src)
        print(f"{len(src)} chars -> {sys.argv[2]}")
    else:
        sys.stdout.reconfigure(encoding="utf-8")
        print(src)
