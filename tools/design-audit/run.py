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
    "R9": ("枠のある操作部品に border-radius が無い", "§04"),
    "R10": ("font-size が実寸スケールの外", "§05"),
    "R11": ("format.ts の外で toLocaleString / toFixed を直呼び", "§08"),
    "R12": ("ui/ の外に生の input / select / textarea", "§07"),
    "R13": ("fmtSigned / fmtPct を通らない符号付き・% の数値", "§08"),
    "R14": ("ページ側で面・未収録・主役数字を独自に作る", "§01 / §05 / §08"),
    "R15": ("--dur-* / DUR を通らない生の時間", "§10"),
    "R16": ("動きのクラスを JS で手で付け外し", "§10"),
    "R17": ("開閉ブロックを手書き(disclosurePane / disclosureCaret を使わず)", "§10"),
    "R18": ("<details> が開閉するのに動きが無い", "§00 / §10"),
}

# §05 の実寸スケール。v4 が使っている実寸で、役割トークン 4 段の外にもある。
# 密度は意識的な選択なので「役割トークンに寄せる」のではなく、この集合に収まるかを見る
FONT_SCALE = {44, 40, 27, 19, 17, 15, 14, 13, 12.5, 12, 11.5, 11, 10.5, 10, 9.5, 9, 8.5, 8}

# R11: 桁区切り・小数桁は format.ts だけが決める(§08「数値の 3 段」)。画面側の直呼びは違反
FORMAT_TS = SRC / "format.ts"
RAW_FORMAT_CALL = re.compile(r"\.(toLocaleString|toFixed)\s*\(")

# R12: 入力は ui/ の 5 形態(StatInput / TextField / Picker / StepSelect / ToggleRow)だけが生の要素を持つ(§07)。
# file / checkbox / radio / range は 5 形態の外(ファイル選択・ON/OFF)なので見ない
UI_DIR = SRC / "ui"
RAW_INPUT = re.compile(r"<(input|select|textarea)\b([^>]*)>", re.S)
INPUT_TYPE = re.compile(r'type\s*=\s*"([^"]*)"')
RAW_INPUT_TYPES = {"text", "number", "search"}

# R13: 符号と % は format.ts が付ける(§08)。手書きの符号(`+${…}` / 三項の "+")と Math.round(x * 100)}% は違反。
# 強化段のラベル(装備 +7 / Lv)は段の名前であって符号ではないので除く
RAW_PCT = re.compile(r"Math\.round\([^\n]*?\*\s*100\)\}%")
RAW_SIGN = re.compile(r"[+−]\$\{([^}]*)\}")
RAW_SIGN_TERNARY = re.compile(r"""\?\s*["']\+["']\s*:\s*["']["']\s*\}\$\{""")
STEP_LABEL = re.compile(r"enhance|level|\blv\b|\+ 1\b|\.join\(")

# R14: 面は app.css の .inset、未収録は .badge.unknown、18px 超の数字は役割トークン(§01 / §05 / §08)。
# ページ側(ui/ と app.css 以外)で塗り直したり作り直したりしない
SURFACE_INSET = re.compile(r"var\(--surface-inset\)")
UNKNOWN_NAMES = {"unknown", "unk", "missing", "unrecorded"}
DASHED_BORDER = re.compile(r"(?:^|[;\s])border(?:-style)?\s*:\s*[^;{}]*\bdashed\b")
MAX_BODY_FONT_PX = 18

# R15: 動きの時間は役割で 9 段(§10「操作の応答」)。出どころは app.css の :root --dur-* と
# ui/motion.svelte.ts の DUR の 2 つだけで、値は一致していること。画面側に 0.18s / 220 を直接書かない。
# 例外は 0(動かさない指定)と、終わりのない回転(infinite)— 後者は「変化を見せる動き」ではない
MOTION_TS = SRC / "ui" / "motion.svelte.ts"
DUR_TOKEN = re.compile(r"--dur-([a-z]+)\s*:\s*([0-9.]+)(m?s)\s*;")
DUR_CONST = re.compile(r"^\s*([a-z]+):\s*(\d+),", re.M)
TIMED_DECL = re.compile(r"\b(transition|animation)(?:-duration)?\s*:\s*([^;{}\n]+)")
ZERO_TIME = {"0", "0s", "0ms", "0.01ms"}  # 0.01ms は reduced-motion の打ち消し
# Svelte の transition / animate に渡す時間。DUR.* を通らない数値リテラルが違反
JS_DURATION = re.compile(r"duration:\s*(\d+)")

# R16: 「一時的にクラスを付けて、少ししたら外す」は ui/motion.svelte.ts の action
# (bump / flash / swap / pulse / delta)がやること。画面側で毎回手で書くと、時間が
# --dur-* / DUR と二重に持たれてずれる(Workspace が 400ms・アニメーションは 260ms
# だった実例が動きの部品化のきっかけ)。app.css の動きのクラスが classList.add / remove /
# class: / setTimeout と同じ行に出てきたら、手書きの疑いが強いので候補に出す
MOTION_TOGGLE_CLASSES = ("badge-in", "open-in", "swap-in", "pane-in", "pop-in", "delta-in", "bump-up", "bump-down")
MOTION_CLASS_NAME = re.compile(r"\b(?:" + "|".join(MOTION_TOGGLE_CLASSES) + r")\b")
JS_CLASS_TOGGLE = re.compile(r"classList\.(?:add|remove)\(|class:[\w-]+|setTimeout\(")

# R17: 「開閉するブロック」の中身の面は ui/motion.svelte.ts の disclosurePane がやること
# (段階 2)。動的束縛 `class:open-in={...}` に加えて、`{#if 条件}` でマウント/アンマウント
# する形の直後に静的な `class="... open-in ..."`(`class:` ではない)を置くやり方も同じ
# 手書き(段階 2 のレビューで判明 — 動的束縛だけを見ていると、常に開いている面の入場と
# 区別がつかず、候補一覧のような「押すと {#if} で出し入れする」6 箇所を素通りしていた)。
# 常に開いている面の入場に使う固定の `class="... open-in"`(`{#if}` を伴わないもの)は
# 開閉ではないので、いまも対象外。
#
# キャレットの `class:rot={...}` は検出対象に含めない。回すキャレットは disclosurePane と
# 対のもの以外に、ポップオーバー(Picker / CalcPage の targetOpen・skillOpen。開閉は
# `.pop-in` + 重ねる面が別方式)や `transition:collapse` の折りたたみ(VersusPage。
# `{#if}` の出入りを Svelte transition が動かす、別方式)でも正しく使われている。
# `class:rot=` 単独で拾うと、それらの正しい使い方まで毎回候補に出て R17 が恒久的に
# 0 件にならない(段階 2 で確認 — 対象は disclosurePane と組む class:open-in だけ)。
DISCLOSURE_HANDWRITTEN = re.compile(r"class:open-in\s*=")
# {#if}(コメントを 1 個だけ挟んでもよい)の直後に静的な class="... open-in ..." が来る形
IF_STATIC_OPEN_IN = re.compile(
    r"\{#if\b[^}]*\}(?:\s*<!--.*?-->)?\s*<[a-zA-Z][^>]*\bclass=\"[^\"]*\bopen-in\b[^\"]*\"",
    re.S,
)

# R18: ネイティブ <details> は既定で瞬時に開閉する(§00 04 / §10 型 6)。段階 2.5 で最初に
# 試した .details-anim(grid-template-rows: 0fr⇄1fr で本文を包む)は実機で動かなかった —
# ブラウザが本文を ::details-content という UA 生成ボックスに持たせていて、grid の行サイズの
# transition がその境界を越えられない(実測: 開始サンプルから最終値のまま、途中経過が無い)。
# 効くのは details::details-content 自体の block-size を動かす形だけで、包む要素は要らず
# 全 <details> に一律で効くので、この規則は「app.css のその 1 か所が生きているか」だけを見る。
# クラスの有無で判定すると grid 版のような『付いているのに動かない』実装にも 0 件を出してしまう
# (段階 2.5 やり直しで判明)ため、実際に効く CSS の形(本文の高さと --dur-open)を見る。
# **並び順に依存させない** — `transition: block-size var(--dur-open)` も
# `transition: var(--dur-open) block-size` も CSS として正しいので、
# 「transition の中に高さのプロパティと --dur-open が両方ある」だけを見る。
# 高さは block-size / height のどちらでもよい(横書き固定のこのアプリでは同義)
DETAILS_CONTENT_CLOSED = re.compile(r"details::details-content\s*\{([^}]*)\}", re.S)
DETAILS_CONTENT_OPEN = re.compile(r"details\[open\]::details-content\s*\{([^}]*)\}", re.S)
# transition 宣言の中に、高さのプロパティと --dur-open が(順不同で)両方あること
DETAILS_TRANSITION = re.compile(
    r"transition\s*:[^;{}]*(?:(?:\bblock-size\b|\bheight\b)[^;{}]*var\(--dur-open\)"
    r"|var\(--dur-open\)[^;{}]*(?:\bblock-size\b|\bheight\b))",
    re.S,
)

# R9 の対象。押す・打ち込む部品だけを見る(地や区切りまで見ると候補が溢れる)
CONTROL_SEL = re.compile(r"(?:^|[\s,>])(?:input|button|select|textarea)\b|\.(?:btn|chip|tab|field|check|toggle|max-btn|num-field|pill|badge)(?![\w-])")

HEX = re.compile(r"#[0-9A-Fa-f]{3,8}\b")
# 面としてべた塗りしている背景。グラデーションや影の中の色は「面」ではない
FLAT_BG = re.compile(r"background(?:-color)?\s*:\s*#[0-9A-Fa-f]{3,8}\b", re.I)


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
            # 白と黒は面のトークン(--bg-field)と同値になるが、対象は面としてべた塗りするときだけ。
            # 文字色の白、グラデーションの明側、box-shadow のハイライトは面ではないので拾わない
            if token and value in ("#ffffff", "#000000") and not FLAT_BG.search(line):
                continue
            if token:
                out.append(Finding("R1", path, i, m.group(0), f"var({token}) と同値"))
            elif is_script_file and i not in css_lines:
                out.append(Finding("R8", path, i, m.group(0), "CSS 変数に寄せられないか"))


def check_raw_format(path: Path, text: str, out: list[Finding]) -> None:
    """R11。TS / Svelte で数値書式を直接呼んでいる箇所。format.ts 自身は書式の実装なので除く。"""
    if path == FORMAT_TS or path.suffix not in (".ts", ".svelte"):
        return
    for i, line in enumerate(text.split("\n"), 1):
        for m in RAW_FORMAT_CALL.finditer(line):
            out.append(Finding("R11", path, i, m.group(1), "format.ts の fmtInt / fmtNum / fmtPct / fmtSigned / fmtRate に寄せる"))


def is_page_side(path: Path) -> bool:
    """ページ側 = ui/ と app.css の外。共有部品と土台は規格の実装そのものなので対象外。"""
    return path != APP_CSS and UI_DIR not in path.parents


def check_raw_inputs(path: Path, text: str, out: list[Finding]) -> None:
    """R12。ui/ の外の .svelte にある生の入力要素。"""
    if path.suffix != ".svelte" or not is_page_side(path):
        return
    for m in RAW_INPUT.finditer(text):
        tag, attrs = m.group(1), m.group(2)
        if tag == "input":
            t = INPUT_TYPE.search(attrs)
            kind = t.group(1) if t else "text"
            if kind not in RAW_INPUT_TYPES:
                continue
            value = f"<input type={kind}>"
        else:
            value = f"<{tag}>"
        out.append(Finding("R12", path, 1 + text.count("\n", 0, m.start()), value,
                           "ui/ の StatInput / TextField / Picker / StepSelect / ToggleRow に寄せる"))


def check_raw_sign(path: Path, text: str, out: list[Finding]) -> None:
    """R13。.svelte で符号や % を手で組み立てている箇所。"""
    if path.suffix != ".svelte":
        return
    for i, line in enumerate(text.split("\n"), 1):
        if RAW_PCT.search(line):
            out.append(Finding("R13", path, i, "Math.round(x * 100)}%", "fmtPct(rate) に寄せる"))
        if RAW_SIGN_TERNARY.search(line):
            out.append(Finding("R13", path, i, '${x >= 0 ? "+" : ""}${…}', "fmtSigned / fmtSignedPct に寄せる"))
        for m in RAW_SIGN.finditer(line):
            if STEP_LABEL.search(m.group(1)):
                continue  # 強化段のラベル
            out.append(Finding("R13", path, i, m.group(0), "fmtSigned / fmtSignedPct に寄せる"))


def check_page_surfaces(chunk: Chunk, out: list[Finding]) -> None:
    """R14。ページ側の CSS で面・未収録・主役数字を独自に作っている箇所。"""
    if not is_page_side(chunk.path):
        return
    for m in SURFACE_INSET.finditer(chunk.text):
        out.append(Finding("R14", chunk.path, chunk.line_of(m.start()), "var(--surface-inset)",
                           "塗り直さず app.css の .inset を付ける"))
    for m in FONT_SIZE.finditer(chunk.text):
        if float(m.group(1)) > MAX_BODY_FONT_PX:
            out.append(Finding("R14", chunk.path, chunk.line_of(m.start(1)), f"{m.group(1)}px",
                               "18px 超は --t-result / --t-result-inline / --t-heading だけ"))
    if chunk.kind != "css":
        return
    for sel, body, offset in rule_blocks(chunk):
        names = sel_names(sel)
        if "badge" in names or not (names & UNKNOWN_NAMES) or not DASHED_BORDER.search(body):
            continue
        out.append(Finding("R14", chunk.path, chunk.line_of(offset), sel,
                           "未収録は .badge.unknown で出す(独自の破線 class を作らない)"))


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


SEL_NAMES = re.compile(r"[.#]?[A-Za-z][-\w]*")
# 枠も地も無い(リセットや透明化)ルールは角丸の対象にならない
NO_SURFACE = {"0", "none", "transparent", "0px", "inherit", "initial", "unset"}


def sel_names(sel: str) -> set[str]:
    """セレクタに出てくるクラス名・要素名の集合。疑似クラスと結合子は落とす。"""
    return {m.group(0).lstrip(".#") for m in SEL_NAMES.finditer(re.sub(r"::?[a-z-]+(\([^)]*\))?", " ", sel))}


FONT_SIZE = re.compile(r"font-size\s*:\s*([0-9.]+)px")


def check_font_scale(chunk: Chunk, out: list[Finding]) -> None:
    """§05 の実寸スケールの外にある font-size。calc() は §06 のアイコン比率なので見ない。"""
    for m in FONT_SIZE.finditer(chunk.text):
        size = float(m.group(1))
        if size not in FONT_SCALE:
            out.append(Finding("R10", chunk.path, chunk.line_of(m.start(1)), f"{m.group(1)}px",
                               "44 / 40 / 27 / 19 / 17 / 15 / 14 / 13 / 12.5 / 12 / 11.5 / 11 / 10.5 / 10 / 9.5 / 9 / 8.5 / 8 の外"))


def check_control_radius(chunk: Chunk, out: list[Finding]) -> None:
    """押す・打ち込む部品に枠や地があるのに角丸が無い(§04 は 0 を段に数えない)。

    同じ面に対する別ルール(`:hover` や修飾クラス)で角丸が付いていれば違反ではないので、
    ファイル内で角丸を宣言しているセレクタの名前を先に集めて突き合わせる。
    """
    if chunk.kind != "css":
        return
    blocks = list(rule_blocks(chunk))
    rounded: set[str] = set()
    for sel, body, _ in blocks:
        if "border-radius" in body:
            rounded |= sel_names(sel)
    for sel, body, offset in blocks:
        if not CONTROL_SEL.search(sel) or "border-radius" in body:
            continue
        decls = DECL.findall(body)
        surface = [v.strip() for p, v in decls if p in ("border", "background", "background-color")]
        if not surface or all(v.split()[0] in NO_SURFACE for v in surface):
            continue
        if sel_names(sel) & rounded:
            continue
        out.append(Finding("R9", chunk.path, chunk.line_of(offset), sel, "角丸が 4 段のどれでもない(0)"))


DURATION = re.compile(r"(?<![-\w.])(\d*\.?\d+)(ms|s)(?![-\w])")


def check_duration(chunk: Chunk, out: list[Finding]) -> None:
    for m in re.finditer(r"\b(transition|animation)(?:-duration)?\s*:\s*([^;{}\n]+)", chunk.text):
        for d in DURATION.finditer(m.group(2)):
            seconds = float(d.group(1)) / (1000 if d.group(2) == "ms" else 1)
            if seconds > MAX_DURATION_S:
                out.append(Finding("R6", chunk.path, chunk.line_of(m.start(2)), m.group(0).strip(),
                                   f"{seconds}s > {MAX_DURATION_S}s"))


def check_duration_tokens(chunk: Chunk, out: list[Finding]) -> None:
    """R15(CSS 側)。transition / animation の時間が --dur-* を通っていない。"""
    for m in TIMED_DECL.finditer(chunk.text):
        value = m.group(2)
        if "infinite" in value:
            continue
        for d in DURATION.finditer(value):
            raw = d.group(0)
            if raw in ZERO_TIME:
                continue
            out.append(Finding("R15", chunk.path, chunk.line_of(m.start(2)), f"{m.group(1)}: {value.strip()}",
                               f"{raw} の直書き。var(--dur-*) から取る"))
            break


def check_duration_js(path: Path, text: str, out: list[Finding]) -> None:
    """R15(JS 側)。Svelte の transition / animate に数値リテラルを渡している。"""
    if path == MOTION_TS:
        return
    for m in JS_DURATION.finditer(text):
        if m.group(1) == "0":  # 動かさない指定(主要部位など)は段の外
            continue
        out.append(Finding("R15", path, 1 + text.count("\n", 0, m.start()), f"duration: {m.group(1)}",
                           "ui/motion.svelte.ts の DUR.* から取る"))


def check_manual_motion_class(path: Path, text: str, out: list[Finding]) -> None:
    """R16。動きのクラスの付け外しを画面側が手で書いている疑いのある行。

    ui/motion.svelte.ts 自身はこのクラスの実装なので対象外。co-occurrence だけを見る
    素朴な検出なので、同じ行にクラス名と操作(classList.add/remove・class:・setTimeout)
    が両方出ているかだけを見て、意味までは判定しない。
    """
    if path.suffix not in (".svelte", ".ts") or path == MOTION_TS:
        return
    for i, line in enumerate(text.split("\n"), 1):
        m = MOTION_CLASS_NAME.search(line)
        if m and JS_CLASS_TOGGLE.search(line):
            out.append(Finding("R16", path, i, m.group(0),
                               "ui/motion.svelte.ts の action(bump / flash / swap / pulse / delta)に寄せる"))


def check_manual_disclosure(path: Path, text: str, out: list[Finding]) -> None:
    """R17。開閉ブロックを disclosurePane / disclosureCaret を使わず手書きしている箇所。

    ui/motion.svelte.ts 自身は実装なので対象外。動的束縛(class:open-in=)は行単位、
    「{#if} の直後に静的な class="... open-in"」はテンプレート全体を見る(2 行にまたがる
    ことがあるため)。
    """
    if path.suffix != ".svelte":
        return
    for i, line in enumerate(text.split("\n"), 1):
        m = DISCLOSURE_HANDWRITTEN.search(line)
        if m:
            out.append(Finding("R17", path, i, m.group(0),
                               "ui/motion.svelte.ts の disclosurePane / disclosureCaret を使う"))
    for m in IF_STATIC_OPEN_IN.finditer(text):
        line = 1 + text.count("\n", 0, m.start())
        out.append(Finding("R17", path, line, "{#if} + 静的 open-in",
                           "ui/motion.svelte.ts の disclosurePane を使う"))


def check_details_motion(out: list[Finding]) -> None:
    """R18(app.css そのもの)。<details> の本文を動かす仕組みが app.css で生きているか。

    全 <details> に一律で効く仕組みなので、ファイルごとではなく app.css を 1 回だけ見る。
    閉じた側の transition に「本文の高さ(block-size / height)」と `--dur-open` が両方あること、
    開いた側が高さを auto に戻していることの 2 つを見る。どちらか欠けると、閉じたまま /
    開いたまま瞬時に切り替わるだけになる。並び順・改行・プロパティ名の選び方には依存させない。
    """
    css = APP_CSS.read_text(encoding="utf-8")
    closed = DETAILS_CONTENT_CLOSED.search(css)
    opened = DETAILS_CONTENT_OPEN.search(css)
    closed_ok = bool(closed and DETAILS_TRANSITION.search(closed.group(1)))
    opened_ok = bool(opened and re.search(r"\b(?:block-size|height)\s*:\s*auto\b", opened.group(1)))
    if not (closed_ok and opened_ok):
        out.append(Finding("R18", APP_CSS, 1, "details::details-content",
                           "block-size を var(--dur-open) で動かす transition が app.css に無い"
                           "(grid-template-rows は ::details-content の境界を越えられず効かない)"))


def check_duration_scale(out: list[Finding]) -> None:
    """R15(段そのもの)。app.css の --dur-* と motion.svelte.ts の DUR が食い違っていないか。"""
    css: dict[str, int] = {}
    for name, num, unit in DUR_TOKEN.findall(APP_CSS.read_text(encoding="utf-8")):
        css[name] = round(float(num) * (1 if unit == "ms" else 1000))
    parts = MOTION_TS.read_text(encoding="utf-8").split("export const DUR = {", 1)
    ts = {k: int(v) for k, v in DUR_CONST.findall(parts[1].split("} as const;", 1)[0])} if len(parts) > 1 else {}
    for name in sorted(set(css) | set(ts)):
        if css.get(name) != ts.get(name):
            out.append(Finding("R15", MOTION_TS, 1, f"--dur-{name}",
                               f"app.css {css.get(name)}ms / DUR {ts.get(name)}ms — 段が 2 つに割れている"))


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
        check_raw_format(path, text, out)
        check_raw_inputs(path, text, out)
        check_raw_sign(path, text, out)
        check_duration_js(path, text, out)
        check_manual_motion_class(path, text, out)
        check_manual_disclosure(path, text, out)
        for chunk in css_chunks(path, text):
            check_page_surfaces(chunk, out)
            check_radius(chunk, out)
            check_tabular(chunk, out)
            check_icon_size(chunk, out)
            check_duration(chunk, out)
            check_state_pairs(chunk, out)
            check_control_radius(chunk, out)
            check_font_scale(chunk, out)
            check_duration_tokens(chunk, out)
    check_duration_scale(out)
    check_details_motion(out)
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
