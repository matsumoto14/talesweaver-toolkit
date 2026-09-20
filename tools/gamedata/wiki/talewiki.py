"""talewiki.com(EUC-JP の PukiWiki)から取るための層。

ここが取得の正本。`.claude/skills/talewiki-fetch/scripts/fetch_page.py` は
この `decode_euc_jp_with_nec` / `fetch_source` を呼ぶだけの薄い CLI。

## 3 つのエンドポイント

| 用途 | エンドポイント | 返るもの |
|---|---|---|
| ページ本文 | `?cmd=source&page=<EUC-JP percent>` | `<pre>` 内に PukiWiki 記法のソース |
| 全ページ名 | `?cmd=list` | HTML。`<div id="body">` 内の `href="./?<EUC-JP percent>"` が 1 ページ 1 個 |
| 更新日時 | `?RecentChanges` のソース | `-<日時> - [ … ] [[ページ名]]` が約 500 行(2024-01 まで遡れる) |

**更新検出に `?cmd=rss` は使わない**(実測 2026-09-20: 15 件しか返らず約 2 日分しかない。
RecentChanges は同じ日時を約 500 件持つ)。ただし RecentChanges には削除・改名が出ないので、
それは `?cmd=list` との突き合わせでしか分からない。
"""
from __future__ import annotations

import html
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timedelta, timezone

BASE = "https://talewiki.com/"
UA = "Mozilla/5.0"
JST = timezone(timedelta(hours=9))


def decode_euc_jp_with_nec(b: bytes) -> str:
    """EUC-JP を復号する。NEC 拡張文字(丸数字①等、先頭バイト 0xAD)を落とさない。

    素の `euc_jp` コーデックや `iconv -c` は 0xAD 区を読めず、そこから後ろが崩れる。
    バイト境界を守って走査し、0xAD 行だけ cp932 の対応位置(0x87xx)にマップする。
    """
    out: list[str] = []
    i, n = 0, len(b)
    buf = bytearray()

    def flush() -> None:
        if buf:
            out.append(buf.decode("euc_jp", errors="replace"))
            buf.clear()

    while i < n:
        c = b[i]
        if c < 0x80:
            buf.append(c); i += 1
        elif c == 0x8E:  # 半角カナ
            buf += b[i:i + 2]; i += 2
        elif c == 0x8F:  # JIS X 0212(3 バイト)
            buf += b[i:i + 3]; i += 3
        elif c == 0xAD and i + 1 < n:  # NEC 特殊文字(13 区)
            flush()
            cell = b[i + 1] - 0x80
            trail = cell + 0x1F if cell <= 0x5F else cell + 0x21
            out.append(bytes([0x87, trail]).decode("cp932", errors="replace"))
            i += 2
        else:
            buf += b[i:i + 2]; i += 2
    flush()
    return "".join(out)


def quote_page(page: str) -> str:
    return urllib.parse.quote(page.encode("euc_jp"))


def unquote_page(encoded: str) -> str:
    """`?cmd=list` の href を戻す。**`+` は空白**(この wiki の href は空白を `+` にする)。

    `Chapter/Secret+Chapter` の実名は `Chapter/Secret Chapter`。`+` のまま投げると
    `<pre>` が返らない。名前に本当に `+` を含むページは `%2B` で来るので取り違えない。
    """
    return urllib.parse.unquote_plus(encoded, encoding="euc_jp", errors="replace")


class Fetcher:
    """1 個だけ作って使い回す。取得間隔と再試行をここだけで持つ。"""

    def __init__(self, delay: float = 0.2, retries: int = 2, timeout: float = 30.0) -> None:
        self.delay = delay
        self.retries = retries
        self.timeout = timeout
        self._last = 0.0

    def _get(self, url: str) -> bytes:
        last_err: Exception | None = None
        for attempt in range(self.retries + 1):
            wait = self.delay - (time.monotonic() - self._last)
            if wait > 0:
                time.sleep(wait)
            req = urllib.request.Request(url, headers={"User-Agent": UA})
            try:
                with urllib.request.urlopen(req, timeout=self.timeout) as r:
                    return r.read()
            except (urllib.error.URLError, OSError) as e:
                last_err = e
                time.sleep(0.5 * (attempt + 1))
            finally:
                self._last = time.monotonic()
        raise RuntimeError(f"取得できない({last_err}): {url}")

    def text(self, url: str) -> str:
        return decode_euc_jp_with_nec(self._get(url))

    def source(self, page: str) -> str:
        """ページのソース。ページが無い / 名前が違うときは PageMissing を投げる。"""
        return parse_source(self.text(f"{BASE}?cmd=source&page={quote_page(page)}"), page)

    def page_names(self) -> list[str]:
        return parse_list(self.text(f"{BASE}?cmd=list"))

    def recent_changes(self) -> dict[str, str]:
        """ページ名 -> 更新日時(ISO8601 +09:00)。同名が複数行あるときは新しい方を残す。"""
        out: dict[str, str] = {}
        for name, dt in parse_recent_changes(self.source("RecentChanges")):
            iso = dt.isoformat()
            if name not in out or iso > out[name]:
                out[name] = iso
        return out


class PageMissing(Exception):
    """`<pre>` が無い = ページ名の誤り(大文字小文字・全角半角)か、ページが存在しない。"""


def parse_source(page_html: str, page: str) -> str:
    m = re.search(r"<pre[^>]*>(.*?)</pre>", page_html, re.S)
    if not m:
        raise PageMissing(page)
    return html.unescape(m.group(1))


def parse_list(list_html: str) -> list[str]:
    """`?cmd=list` の HTML からページ名を取る(実測 3,438 件)。

    ナビゲーションのリンクを拾わないよう `<div id="body">` 以降だけを見て、
    クエリを持つもの(`cmd=…` / `plugin=…`)を捨てる。
    """
    body = list_html.split('<div id="body"', 1)[-1]
    names: list[str] = []
    seen: set[str] = set()
    for enc in re.findall(r'href="\./\?([^"]+)"', body):
        if "=" in enc:
            continue
        name = unquote_page(enc)
        if name and name not in seen:
            seen.add(name)
            names.append(name)
    return names


_RC = re.compile(
    r"^-(\d{4})-(\d{2})-(\d{2}) \([^)]*\) (\d{2}):(\d{2}):(\d{2}) - .*?\[\[(.+?)\]\]\s*$"
)


def parse_recent_changes(source: str) -> list[tuple[str, datetime]]:
    """RecentChanges のソースから (ページ名, 更新日時) を新しい順に取る。

    1 行の形: `-2026-09-20 (日) 10:06:53 - [ &pageaction("X",diff); | … ] [[X]]`
    日時は JST(wiki の表示がそう)。`&pageaction(...)` 側ではなく `[[…]]` を名前とする。
    """
    out: list[tuple[str, datetime]] = []
    for line in source.splitlines():
        m = _RC.match(line.strip())
        if not m:
            continue
        y, mo, d, h, mi, s = (int(x) for x in m.groups()[:6])
        out.append((m.group(7), datetime(y, mo, d, h, mi, s, tzinfo=JST)))
    return out
