"""韓国語辞書の点検(docs/adr/022-i18n.md)。

画面の `t("…")` に渡している日本語の文言を集め、`apps/desktop/src/i18n/ko/*.json` と突き合わせる。

- 訳が無い文言(韓国語にすると日本語のまま出る)
- Rust が画面へ返す日本語(文字列リテラル。`format!` の文面は `{0}` `{1}`… の穴に置き換えた形)で
  `rust.json` に訳が無いもの。画面は Rust の文字列を `t(変数)` で引き、穴のある訳は照合で当てる(i18n.ts)
- 使われていない訳(画面・Rust の文言を変えたのに辞書が古い)。ゲームデータ名は `t(変数)` で引くので
  画面からは見えない — `names.json` は数えない
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
DYNAMIC_FILES = {"names.json", "notes.json"}
# 画面へ文字列を返す Rust(ゲームデータ名の crates/gamedata は names.json 側)
RUST_DIRS = [
    ROOT / "crates" / "domain" / "src",
    ROOT / "crates" / "commands" / "src",
    ROOT / "crates" / "storage" / "src",
    ROOT / "crates" / "web" / "src",
    ROOT / "apps" / "desktop" / "src-tauri" / "src",
]
RUST_COMMENT = re.compile(r"//[^\n]*")
RUST_STRING = re.compile(r'"((?:[^"\\\n]|\\.)*)"')
# 画面に届かない文字列(開発者向けの落ち方・検査)
RUST_INTERNAL = re.compile(r"\b(?:expect|panic!|unreachable!|assert!|assert_eq!|debug_assert!|todo!)\s*\(\s*$")
FORMAT_HOLE = re.compile(r"(?<!\{)\{[^{}]*\}(?!\})")

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


def rust_texts(known: set[str]) -> dict[str, set[str]]:
    """Rust が返しうる日本語の文字列(と、日本語を含まないが訳を持つ「AF」など)。
    `format!` の穴は出てくる順に `{0}` `{1}`…。"""
    found: dict[str, set[str]] = {}
    for base in RUST_DIRS:
        for path in base.rglob("*.rs"):
            if path.name in {"tests.rs", "args_check.rs"} or "tests" in path.relative_to(base).parts or "bin" in path.parts:
                continue
            text = path.read_text(encoding="utf-8")
            cut = text.find("#[cfg(test)]")
            if cut >= 0:
                text = text[:cut]
            text = RUST_COMMENT.sub("", text)
            for m in RUST_STRING.finditer(text):
                raw = m.group(1)
                if not (JAPANESE.search(raw) or raw in known) or RUST_INTERNAL.search(text[max(0, m.start() - 40):m.start()]):
                    continue
                counter = iter(range(100))
                key = FORMAT_HOLE.sub(lambda _: f"{{{next(counter)}}}", raw)
                key = key.replace("{{", "{").replace("}}", "}").replace('\\"', '"')
                found.setdefault(key, set()).add(path.relative_to(ROOT).as_posix())
    return found


def untranslated_in(text: str) -> int:
    """t() の外に残っている日本語の文字数(コメントを除く)。"""
    body = COMMENTS.sub("", text)
    body = TC_CALL.sub("", T_CALL.sub("", body))
    return len(JAPANESE.findall(body))


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    used: dict[str, set[str]] = {}
    broken: list[str] = []
    leftover: list[tuple[int, str]] = []
    for path in sources():
        text = path.read_text(encoding="utf-8")
        rel = path.relative_to(SRC).as_posix()
        for m in T_CALL.finditer(COMMENTS.sub("", text)):
            key = json.loads(f'"{m.group(1)}"', strict=False)
            if "\n" in key:
                broken.append(f"{rel}: {key[:40]!r}")
            used.setdefault(key, set()).add(rel)
        for m in TC_CALL.finditer(COMMENTS.sub("", text)):
            key = json.loads(f'"{m.group(1)}::{m.group(2)}"', strict=False)
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

    rust = rust_texts(set(ko))
    rust_missing = sorted(k for k in rust if k not in ko)
    used_anywhere = set(used) | set(rust)
    missing = sorted(k for k in used if k not in ko)
    unused = sorted(k for k in static_keys if k not in used_anywhere)
    holes = sorted(
        k for k in used_anywhere
        if k in ko and set(PLACEHOLDER.findall(k)) != set(PLACEHOLDER.findall(ko[k]))
    )

    for where in broken:
        print(f"t() の文字列に改行がある(JS として不正): {where}")
    for key in missing:
        print(f"訳が無い: {key!r}  ({', '.join(sorted(used[key]))})")
    for key in rust_missing:
        print(f"Rust の文言に訳が無い: {key!r}  ({', '.join(sorted(rust[key]))})")
    for key in unused:
        print(f"使われていない訳: {key!r}")
    for key in holes:
        print(f"穴が食い違う: {key!r} -> {ko[key]!r}")

    if "--untranslated" in sys.argv:
        for n, rel in sorted(leftover, reverse=True):
            print(f"{n:6d}  {rel}")
    total = sum(n for n, _ in leftover)
    print(
        f"t() の文言 {len(used)} / 訳が無い {len(missing)} / Rust の文言 {len(rust)}(訳が無い {len(rust_missing)}) / "
        f"使われていない {len(unused)} / "
        f"穴の食い違い {len(holes)} / t() の外の日本語 {total} 字({len(leftover)} ファイル)"
    )
    return 1 if broken or missing or rust_missing or unused or holes else 0


if __name__ == "__main__":
    sys.exit(main())
