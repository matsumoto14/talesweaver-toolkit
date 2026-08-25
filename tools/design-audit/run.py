#!/usr/bin/env python3
"""デザインシステム(docs/design-system.html)適合の機械監査。

**候補を出すだけで、判定はしない。**閾値での自動修正もしない。
出てきたものは goal 文書(docs/claude/goals/2026-08-25-design-conformance.md)の
「② 分類」にかけて、違反 / 規格の穴 / 決定待ち に振り分けること。

    python tools/design-audit/run.py            # 全ルール
    python tools/design-audit/run.py R2 R4      # ルールを絞る
    python tools/design-audit/run.py --list     # ルール一覧
"""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "apps" / "desktop" / "src"
APP_CSS = SRC / "app.css"

# --- 規格の定数(docs/design-system.html の各節) ---------------------------

RADIUS_STEPS = {"12px": "--r-window", "9px": "--r-panel", "6px": "--r-inset", "999px": "--r-pill"}
RADIUS_FREE = {"50%", "0", "0px", "inherit", "initial", "unset"}  # 段に数えない形
ICON_SIZES = {20, 28, 40, 64}
MAX_DURATION_S = 0.5

# §03 状態の 6 系統。bg / border の組で 1 系統。
STATE_SYSTEMS = [
    ("余裕・目標", "#dcebff", "#426dd6"),
    ("足りている", "#dff3e6", "#6fa98a"),
    ("ぎりぎり・操作待ち", "#fdf3de", "#c2a057"),
    ("届かない・危険", "#f6e8e5", "#b08480"),
    ("対象外・判定不能", "#eceef2", "#a9b4c4"),
    ("一時・チーム条件", "#efeef8", "#6d6aa8"),
]
STATE_OF: dict[str, tuple[int, str]] = {}
for _i, (_name, _bg, _bd) in enumerate(STATE_SYSTEMS):
    STATE_OF[_bg] = (_i, _name)
    STATE_OF[_bd] = (_i, _name)

RULES = {
    "R1": ("トークンがあるのに色を直書き", "§03 / §15"),
    "R2": ("4 段以外の border-radius", "§04"),
    "R3": ("--r-* を使わない border-radius の直書き", "§04"),
    "R4": ("数値書体に tabular-nums が無い", "§05"),
    "R5": ("アイコンサイズが 20 / 28 / 40 / 64 以外", "§06"),
    "R6": ("0.5s を超える transition / animation", "§10"),
    "R7": ("状態 6 系統をまたいだ色の組み合わせ", "§03"),
    "R8": ("TS / Svelte に色の実値を直書き", "§15"),
}

HEX = re.compile(r"#[0-9A-Fa-f]{3,8}\b")


def norm_hex(h: str) -> str:
    """#abc → #aabbcc、大文字小文字と #rrggbbff の ff を吸収して比較用に正規化する。"""
    s = h.lower()
    body = s[1:]
    if len(body) in (3, 4):
        body = "".join(c * 2 for c in body)
    if len(body) == 8 and body.endswith("ff"):
        body = body[:6]
    return "#" + body


@dataclass
class Finding:
    rule: str
    path: Path
    line: int
    value: str
    detail: str

    @property
    def where(self) -> str:
        return f"{self.path.relative_to(ROOT).as_posix()}:{self.line}"


# --- ソースの切り出し -------------------------------------------------------


@dataclass
class Chunk:
    """行番号を保ったままの部分文字列。CSS 相当のテキストを表す。"""

    text: str
    start_line: int
    path: Path
    kind: str  # "css" | "inline"

    def line_of(self, offset: int) -> int:
        return self.start_line + self.text.count("\n", 0, offset)


STYLE_BLOCK = re.compile(r"<style[^>]*>(.*?)</style>", re.S)
INLINE_STYLE = re.compile(r'style\s*=\s*"([^"]*)"', re.S)


def css_chunks(path: Path, text: str) -> list[Chunk]:
    """CSS ファイルは全体、Svelte は <style> ブロックと inline style 属性。"""
    if path.suffix == ".css":
        return [Chunk(text, 1, path, "css")]
    out: list[Chunk] = []
    for m in STYLE_BLOCK.finditer(text):
        out.append(Chunk(m.group(1), 1 + text.count("\n", 0, m.start(1)), path, "css"))
    for m in INLINE_STYLE.finditer(text):
        out.append(Chunk(m.group(1), 1 + text.count("\n", 0, m.start(1)), path, "inline"))
    return out


def script_text(path: Path, text: str) -> str:
    """色の直書き検査から <style> を除いた本体を返す(CSS 側は R1 が別に見る)。"""
    if path.suffix == ".ts":
        return text
    return STYLE_BLOCK.sub(lambda m: "\n" * m.group(0).count("\n"), text)


def rule_blocks(chunk: Chunk):
    """CSS の { … } を素朴に対応付けて (セレクタ, 本体, 本体の開始 offset) を返す。"""
    text = chunk.text
    depth = 0
    sel_start = 0
    body_start = 0
    for i, ch in enumerate(text):
        if ch == "{":
            depth += 1
            if depth == 1:
                body_start = i + 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                yield text[sel_start:body_start - 1].strip(), text[body_start:i], body_start
                sel_start = i + 1


DECL = re.compile(r"([-a-zA-Z]+)\s*:\s*([^;{}]+)")


# --- トークン表 -------------------------------------------------------------


def load_tokens() -> dict[str, str]:
    """app.css の :root から「値 → トークン名」を作る。同値が複数なら最初のもの。"""
    text = APP_CSS.read_text(encoding="utf-8")
    root = text.split(":root", 1)[1]
    root = root[root.index("{") + 1 : root.index("\n}")]
    table: dict[str, str] = {}
    for name, value in re.findall(r"(--[-a-z0-9]+)\s*:\s*([^;]+);", root):
        v = value.strip()
        if HEX.fullmatch(v):
            table.setdefault(norm_hex(v), name)
    return table


# --- ルール -----------------------------------------------------------------


def check_colors(path: Path, text: str, tokens: dict[str, str], out: list[Finding]) -> None:
    """R1(トークン同値の直書き)と R8(TS / Svelte への色直書き)。"""
    is_script_file = path.suffix in (".ts", ".svelte")
    css_lines: set[int] = set()
    for chunk in css_chunks(path, text):
        if chunk.kind == "css":
            css_lines.update(range(chunk.start_line, chunk.start_line + chunk.text.count("\n") + 1))

    for i, line in enumerate(text.split("\n"), 1):
        if path == APP_CSS and line.lstrip().startswith("--"):
            continue  # トークン定義そのもの
        for m in HEX.finditer(line):
            value = norm_hex(m.group(0))
            token = tokens.get(value)
            # 白と黒は面のトークン(--bg-field)と同値になるが、文字色の白は別物。地に使うときだけ拾う
            if token and value in ("#ffffff", "#000000") and "background" not in line:
                continue
            if token:
                out.append(Finding("R1", path, i, m.group(0), f"var({token}) と同値"))
            elif is_script_file and i not in css_lines:
                out.append(Finding("R8", path, i, m.group(0), "CSS 変数に寄せられないか"))


def check_radius(chunk: Chunk, out: list[Finding]) -> None:
    for m in re.finditer(r"border-radius\s*:\s*([^;{}\n]+)", chunk.text):
        raw = m.group(1).strip()
        if "var(--r-" in raw:
            continue
        line = chunk.line_of(m.start(1))
        for value in raw.split():
            v = value.strip().rstrip("!important").strip()
            if not v or v in RADIUS_FREE or v.startswith("var("):
                continue
            if v in RADIUS_STEPS:
                out.append(Finding("R3", chunk.path, line, raw, f"var({RADIUS_STEPS[v]}) と同値"))
            else:
                out.append(Finding("R2", chunk.path, line, raw, "4 段(12 / 9 / 6 / 999)の外"))
            break


def uses_num_font(decls: str) -> bool:
    """font-family(font 略記)で数値書体を当てているか。--font-num の定義そのものは除く。"""
    for prop, value in DECL.findall(decls):
        if prop in ("font-family", "font") and ("--font-num" in value or "monospace" in value):
            return True
    return False


def check_tabular(chunk: Chunk, out: list[Finding]) -> None:
    if chunk.kind == "inline":
        if uses_num_font(chunk.text) and "tabular-nums" not in chunk.text:
            out.append(Finding("R4", chunk.path, chunk.start_line, "inline style",
                               "数値書体を当てているが tabular-nums が無い"))
        return
    for sel, body, offset in rule_blocks(chunk):
        if not uses_num_font(body):
            continue
        if "font-variant-numeric" in body or "tabular-nums" in body:
            continue
        out.append(Finding("R4", chunk.path, chunk.line_of(offset), sel or "(rule)",
                           "数値書体だが tabular-nums が無い"))


def check_icon_size(chunk: Chunk, out: list[Finding]) -> None:
    if chunk.kind != "css":
        return
    for sel, body, offset in rule_blocks(chunk):
        if "icon" not in sel.lower():
            continue
        for prop, value in DECL.findall(body):
            if prop not in ("width", "height", "min-width", "min-height", "font-size"):
                continue
            for num in re.findall(r"(\d+(?:\.\d+)?)px", value):
                if prop == "font-size":
                    continue
                if float(num) not in ICON_SIZES:
                    out.append(Finding("R5", chunk.path, chunk.line_of(offset), f"{sel} {{ {prop}: {value.strip()} }}",
                                       "4 段(20 / 28 / 40 / 64)の外"))


DURATION = re.compile(r"(?<![-\w.])(\d*\.?\d+)(ms|s)(?![-\w])")


def check_duration(chunk: Chunk, out: list[Finding]) -> None:
    for m in re.finditer(r"\b(transition|animation)(?:-duration)?\s*:\s*([^;{}\n]+)", chunk.text):
        for d in DURATION.finditer(m.group(2)):
            seconds = float(d.group(1)) / (1000 if d.group(2) == "ms" else 1)
            if seconds > MAX_DURATION_S:
                out.append(Finding("R6", chunk.path, chunk.line_of(m.start(2)), m.group(0).strip(),
                                   f"{seconds}s > {MAX_DURATION_S}s"))


def check_state_pairs(chunk: Chunk, out: list[Finding]) -> None:
    """背景と枠が別々の系統から来ていないか。"""
    def scan(decls: str, line: int) -> None:
        found: dict[str, tuple[int, str]] = {}
        for prop, value in DECL.findall(decls):
            if prop not in ("background", "background-color", "border", "border-color"):
                continue
            for h in HEX.finditer(value):
                sysinfo = STATE_OF.get(norm_hex(h.group(0)))
                if sysinfo:
                    found["bg" if prop.startswith("background") else "bd"] = sysinfo
        if len(found) == 2 and found["bg"][0] != found["bd"][0]:
            out.append(Finding("R7", chunk.path, line,
                               f'{found["bg"][1]} の地 + {found["bd"][1]} の枠',
                               "6 系統をまたいでいる"))

    if chunk.kind == "inline":
        scan(chunk.text, chunk.start_line)
    else:
        for _sel, body, offset in rule_blocks(chunk):
            scan(body, chunk.line_of(offset))


# --- 実行 -------------------------------------------------------------------


def collect() -> list[Finding]:
    tokens = load_tokens()
    out: list[Finding] = []
    files = sorted(p for p in SRC.rglob("*") if p.suffix in (".css", ".svelte", ".ts"))
    for path in files:
        text = path.read_text(encoding="utf-8")
        check_colors(path, text, tokens, out)
        for chunk in css_chunks(path, text):
            check_radius(chunk, out)
            check_tabular(chunk, out)
            check_icon_size(chunk, out)
            check_duration(chunk, out)
            check_state_pairs(chunk, out)
    return out


def main(argv: list[str]) -> int:
    # Windows のコンソール既定(cp932)だと日本語の一部が落ちるので UTF-8 に固定する
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8", errors="replace")
    if "--list" in argv:
        for rid, (title, section) in RULES.items():
            print(f"{rid}  {section:10s} {title}")
        return 0
    wanted = {a for a in argv if a in RULES} or set(RULES)

    findings = [f for f in collect() if f.rule in wanted]
    by_rule: dict[str, list[Finding]] = defaultdict(list)
    for f in findings:
        by_rule[f.rule].append(f)

    for rid in RULES:
        if rid not in wanted:
            continue
        title, section = RULES[rid]
        items = by_rule.get(rid, [])
        print(f"\n=== {rid} {title}  ({section})  {len(items)} 件 ===")
        if not items:
            print("  なし")
            continue
        groups: dict[tuple[str, str], list[Finding]] = defaultdict(list)
        for f in items:
            groups[(f.value, f.detail)].append(f)
        for (value, detail), fs in sorted(groups.items(), key=lambda kv: -len(kv[1])):
            print(f"  {value}  — {detail}  ({len(fs)} 箇所)")
            for f in fs:
                print(f"      {f.where}")

    print(f"\n--- 合計 {len(findings)} 件。これは候補であって違反ではない。goal の「② 分類」にかけること ---")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
