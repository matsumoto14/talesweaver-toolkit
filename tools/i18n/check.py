"""韓国語辞書の点検(docs/adr/022-i18n.md)。

画面の `t("…")` に渡している日本語の文言を集め、`apps/desktop/src/i18n/ko/*.json` と突き合わせる。

- 訳が無い文言(韓国語にすると日本語のまま出る)
- 使われていない訳(画面の文言を変えたのに辞書が古い)。Rust が返す表示名とゲームデータ名は
  `t(変数)` で引くので画面からは見えない — `rust.json` / `names.json` は数えない
- `{name}` の穴が日本語と韓国語で食い違う訳
- `t()` に通していない日本語が残っているファイル(段階 3 の進み具合。コメントは除く)

使い方: python tools/i18n/check.py [--untranslated]  (問題があれば終了コード 1)
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "apps" / "desktop" / "src"
KO = SRC / "i18n" / "ko"
# 画面からは見えない鍵(t(変数) で引く)を持つ辞書
DYNAMIC_FILES = {"rust.json", "names.json"}

JAPANESE = re.compile(r"[ぁ-んァ-ヶー一-龯]")
T_CALL = re.compile(r'\bt\(\s*"((?:[^"\\]|\\.)*)"')
# tc("文脈", "文言") は辞書の鍵 `文脈::文言`
TC_CALL = re.compile(r'\btc\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"')
PLACEHOLDER = re.compile(r"\{(\w+)\}")
COMMENTS = re.compile(r"//[^\n]*|/\*.*?\*/|<!--.*?-->", re.S)
STRING = re.compile(r'"(?:[^"\\\n]|\\.)*"|`(?:[^`\\]|\\.)*`')


def sources() -> list[Path]:
    return [
        p for p in SRC.rglob("*")
        if p.suffix in {".ts", ".svelte"} and "i18n" not in p.parts and p.name != "types.ts"
    ]


def untranslated_in(text: str) -> int:
    """t() の外に残っている日本語の文字数(コメントを除く)。"""
    body = COMMENTS.sub("", text)
    body = TC_CALL.sub("", T_CALL.sub("", body))
    return len(JAPANESE.findall(body))


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    used: dict[str, set[str]] = {}
    leftover: list[tuple[int, str]] = []
    for path in sources():
        text = path.read_text(encoding="utf-8")
        rel = path.relative_to(SRC).as_posix()
        for m in T_CALL.finditer(COMMENTS.sub("", text)):
            key = json.loads(f'"{m.group(1)}"')
            used.setdefault(key, set()).add(rel)
        for m in TC_CALL.finditer(COMMENTS.sub("", text)):
            key = json.loads(f'"{m.group(1)}::{m.group(2)}"')
            used.setdefault(key, set()).add(rel)
        n = untranslated_in(text)
        if n:
            leftover.append((n, rel))

    ko: dict[str, str] = {}
    static_keys: set[str] = set()
    for path in sorted(KO.glob("*.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        for key, value in data.items():
            if key in ko and ko[key] != value:
                print(f"訳が食い違う: {key!r} ({path.name})")
            ko[key] = value
            if path.name not in DYNAMIC_FILES:
                static_keys.add(key)

    missing = sorted(k for k in used if k not in ko)
    unused = sorted(k for k in static_keys if k not in used)
    holes = sorted(
        k for k in used
        if k in ko and set(PLACEHOLDER.findall(k)) != set(PLACEHOLDER.findall(ko[k]))
    )

    for key in missing:
        print(f"訳が無い: {key!r}  ({', '.join(sorted(used[key]))})")
    for key in unused:
        print(f"使われていない訳: {key!r}")
    for key in holes:
        print(f"穴が食い違う: {key!r} -> {ko[key]!r}")

    if "--untranslated" in sys.argv:
        for n, rel in sorted(leftover, reverse=True):
            print(f"{n:6d}  {rel}")
    total = sum(n for n, _ in leftover)
    print(
        f"t() の文言 {len(used)} / 訳が無い {len(missing)} / 使われていない {len(unused)} / "
        f"穴の食い違い {len(holes)} / t() の外の日本語 {total} 字({len(leftover)} ファイル)"
    )
    return 1 if missing or unused or holes else 0


if __name__ == "__main__":
    sys.exit(main())
