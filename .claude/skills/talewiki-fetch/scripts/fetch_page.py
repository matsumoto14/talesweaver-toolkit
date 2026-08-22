"""talewiki.com(EUC-JP PukiWiki)のページソースを UTF-8 で取得する。

使い方:
    python fetch_page.py <ページ名> [出力ファイル]
    python fetch_page.py ステータス <scratchpad>/status.txt

- `?cmd=source&page=<EUC-JP URL エンコード>` で wiki ソースを取得し、<pre> 内を抽出して HTML エンティティを解除する
- NEC 拡張文字(丸数字①②等、EUC-JP の先頭バイト 0xAD)は euc_jp コーデックで読めないため、
  バイト境界を守って走査し、0xAD 行だけ cp932 の対応位置(0x87xx)にマップして復号する
"""
import html
import re
import sys
import urllib.parse
import urllib.request


def decode_euc_jp_with_nec(b: bytes) -> str:
    out = []
    i, n = 0, len(b)
    buf = bytearray()

    def flush():
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


def fetch_source(page: str) -> str:
    enc = urllib.parse.quote(page.encode("euc_jp"))
    url = f"https://talewiki.com/?cmd=source&page={enc}"
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req, timeout=30) as r:
        raw = r.read()
    text = decode_euc_jp_with_nec(raw)
    m = re.search(r"<pre[^>]*>(.*?)</pre>", text, re.S)
    if not m:
        raise SystemExit(f"<pre> が見つからない(ページ名の誤りか、ページが存在しない): {url}")
    return html.unescape(m.group(1))


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    src = fetch_source(sys.argv[1])
    if len(sys.argv) >= 3:
        with open(sys.argv[2], "w", encoding="utf-8", newline="\n") as f:
            f.write(src)
        print(f"{len(src)} chars -> {sys.argv[2]}")
    else:
        sys.stdout.reconfigure(encoding="utf-8")
        print(src)
