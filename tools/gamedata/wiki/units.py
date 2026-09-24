r"""wiki.sqlite の PukiWiki ソースを D1 の unit / wiki_table / alias / unit_link / column_note /
meta 行に切り、`tools/gamedata/wiki/out/units.sql` を書き出す。

    python tools/gamedata/wiki/units.py [--cache path] [--out dir] [--segment-cli path]
                                        [--limit N] [--pages 名前,名前,...]

本文を持たない方針(docs/adr/013-wiki-import.md)の担保はここで作る:
- unit.text は段落の冒頭の断片(最大 3 文・200 字)、または表の行の「列名: 値」だけ
- 全文は分かち書きの結果(語の集合)だけが unit_fts に入る。SQL に本文そのものは出さない

段階 0 の契約は同じディレクトリの scratchpad/design/stage0-spec.md(司令塔が決めた)を正とする。
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

from store import Store  # noqa: E402
from talewiki import quote_page  # noqa: E402

HERE = Path(__file__).resolve().parent
BASE_URL = "https://talewiki.com/?"

# --- 除外ページ(コメント系。索引にも入れない。ADR-013) ------------------------------------

# 除外規則はここ 1 か所にまとめる(コメント・wiki 自身の運用ページ・システムページ)。
# `Link/*` `TWFAQ/*` `MAP/*` はドメインのページなので除外しない。
_EXACT_EXCLUDED = {
    "MenuBar", "SideBar", "RecentChanges", "RecentDeleted", "InterWikiName",
    "Help", "FormattingRules",
    "SandBox", "img", "InterWiki", "InterWikiSandBox", "InterWikiテクニカル",
    "WikiName", "WikiEngines", "YukiWiki", "PHP", "dev-talewiki", "AutoTicketLinkName",
    "提案１", "多目的用ページ", "更新依頼用", "導入済みプラグイン", "整形ルール", "掲示板一覧",
    "ヘルプ", "1LineBBS",
}
_PREFIX_EXCLUDED = (
    "コメント/", "PukiWiki/", ":config/", "Comments/", "Z_", "練習用ページ", "公式告知",
    "MenuBar", "Menubar", "Menu2",  # MenuBar/Item/武器 のようなナビ用の下位ページも含む
)
_SUFFIX_EXCLUDED = ("/1LineBBS", "/コメント")
_EXACT_SUFFIX_EXCLUDED = (":RenameLog",)


def is_excluded_page(name: str) -> bool:
    """コメント・wiki 編集・システムページ(unit にしない。page 表には載せる)。"""
    if name in _EXACT_EXCLUDED:
        return True
    if name.startswith(_PREFIX_EXCLUDED):
        return True
    if name.endswith(_SUFFIX_EXCLUDED):
        return True
    if any(name.endswith(s) for s in _EXACT_SUFFIX_EXCLUDED):
        return True
    return False


# --- インライン装飾 ------------------------------------------------------------------------

_LINK = re.compile(r"\[\[(.+?)\]\]")
# インラインプラグイン `&name(args){body};` は末尾の `;` まで記法の一部(残すと本文に `;` が散る)
_COLOR = re.compile(r"&color\([^)]*\)\{([^}]*)\};?")
_SIZE = re.compile(r"&size\([^)]*\)\{([^}]*)\};?")
# `&ref(...)` の引数(画像ファイル名)自体に丸括弧が入ることがある
# (`&ref(ぼのぼのの森のお土産(アーミングソード).png,nolink);`)。1 段までのネストを許して
# 拾う(段階 3 spec 9 実測: これを許さないと横結合の名前行から `.png,nolink); ` が漏れる)。
_REF = re.compile(r"&ref\((?:[^()]|\([^()]*\))*\)\s*;?")
# `&aname(識別子);` はアンカーを打つだけの見えないプラグイン(装備表の行頭でよく使われる)。
_ANAME = re.compile(r"&aname\([^)]*\)\s*;?")
_ENTITY = re.compile(r"&(?:[a-zA-Z]+|#\d+);")
_BOLD3 = re.compile(r"'''(.*?)'''")
_BOLD2 = re.compile(r"''(.*?)''")
_STRIKE = re.compile(r"%%(.*?)%%")
# 位置・色の接頭辞は行頭だけでなくセル・文の途中にも出る(`貰えるもの：COLOR(blue):マナP`)
_PREFIX = re.compile(r"(?:SIZE\(\d+\):|COLOR\([^)]*\):|BGCOLOR\([^)]*\):|CENTER:|RIGHT:|LEFT:)")
# `COLOR(blue){自身・味方}` のように `&` の無いブロック形もある(列名に出る)
_COLOR_BLOCK = re.compile(r"(?:COLOR|SIZE|BGCOLOR)\([^)]*\)\{([^}]*)\}")
_MEANINGFUL_TEXT = re.compile(r"[一-鿿゠-ヿぁ-ゟa-zA-Z0-9]")


def _split_link(inner: str) -> tuple[str, str]:
    """`[[...]]` の中身を (表示, ページ名) に。`表示>ページ#anchor` / `表示:URL` / `ページ`。

    表示部分は `[雑貨店]` のように角括弧を含むことがある(`[[[雑貨店]>Shop/…]]`)。
    """
    if ">" in inner:
        display, target = inner.split(">", 1)
    elif ":" in inner and "://" in inner:
        display, target = inner.split(":", 1)
    else:
        display = target = inner
    display = display.strip().strip("[]").strip()
    target = target.split("#", 1)[0].strip()
    return display, target


def extract_links(text: str) -> list[str]:
    """`[[名前]]` / `[[表示>名前]]` のページ名部分を出現順で返す(アンカーと URL は落とす)。"""
    out = []
    for m in _LINK.finditer(text):
        _, target = _split_link(m.group(1))
        if target and "://" not in target:
            out.append(target)
    return out


def strip_decorations(text: str) -> str:
    """インライン装飾を落として素の文にする(最小で足りる範囲)。"""
    out = text.replace("&br;", "\n")
    out = _LINK.sub(lambda m: _split_link(m.group(1))[0], out)
    out = _COLOR.sub(lambda m: m.group(1), out)
    out = _SIZE.sub(lambda m: m.group(1), out)
    out = _REF.sub("", out)
    out = _ANAME.sub("", out)
    out = _ENTITY.sub(" ", out)
    out = _BOLD3.sub(lambda m: m.group(1), out)
    out = _BOLD2.sub(lambda m: m.group(1), out)
    out = _STRIKE.sub(lambda m: m.group(1), out)
    out = _COLOR_BLOCK.sub(lambda m: m.group(1), out)
    out = _PREFIX.sub("", out)
    return out.strip()


def has_meaningful_text(text: str) -> bool:
    """記号・罫線だけの断片(`-`、`&nbsp;`、`----`)を unit にしない。"""
    return bool(_MEANINGFUL_TEXT.search(text))


# --- 段落の断片(冒頭 3 文・200 字) ---------------------------------------------------------

_SENTENCE_END = re.compile("[。!！?？\n]")  # 半角・全角の ! ? と改行
_MAX_SENTENCES = 3
_MAX_CHARS = 200


def fragment(text: str) -> tuple[str, bool]:
    """段落の冒頭の断片と、続きがあるか(truncated)。"""
    text = text.strip()
    if not text:
        return "", False
    sentences: list[str] = []
    last = 0
    for m in _SENTENCE_END.finditer(text):
        end = m.end()
        piece = text[last:end].strip(" \t")
        if piece.strip():
            sentences.append(piece)
        last = end
        if len(sentences) >= _MAX_SENTENCES:
            break
    remainder = text[last:].strip()
    if len(sentences) < _MAX_SENTENCES and remainder:
        sentences.append(remainder)
        remainder = ""
    # 文の境界(。や改行)は元のまま残し、改行だけ空白にする(「こと2) …」と繋がらないように)
    kept = " ".join("".join(sentences).split())
    truncated = bool(remainder) or len(kept) > _MAX_CHARS
    if len(kept) > _MAX_CHARS:
        kept = kept[:_MAX_CHARS]
    return kept, truncated


# --- 数値化 --------------------------------------------------------------------------------

_NUM = r"[+-]?[0-9０-９,、]+"
_RANGE = re.compile(rf"^({_NUM})\s*[~〜\-ー]\s*({_NUM})$")
_LV_UP = re.compile(rf"^Lv\s*({_NUM})\s*以上$")
_PLAIN = re.compile(rf"^({_NUM})\s*(?:%|％|パーセント)?$")


def _to_int(raw: str) -> int | None:
    s = raw.translate(str.maketrans("０１２３４５６７８９", "0123456789"))
    s = s.replace(",", "").replace("、", "")
    try:
        return int(s)
    except ValueError:
        return None


def numify(raw: str) -> int | list[int] | None:
    """セルを数値化する。`1,234` `100%` `+20` は数値、範囲は [下限, 上限]、`Lv60以上` は [60, 9999]。"""
    s = strip_decorations(raw).strip()
    if not s:
        return None
    m = _RANGE.match(s)
    if m:
        lo, hi = _to_int(m.group(1)), _to_int(m.group(2))
        if lo is not None and hi is not None:
            return [lo, hi]
    m = _LV_UP.match(s)
    if m:
        lo = _to_int(m.group(1))
        if lo is not None:
            return [lo, 9999]
    m = _PLAIN.match(s)
    if m:
        n = _to_int(m.group(1))
        if n is not None:
            return n
    return None


# --- ブロック分解(見出し・段落・箇条書き・表) ------------------------------------------------

@dataclass
class Heading:
    level: int
    text: str
    anchor: str | None


@dataclass
class Paragraph:
    text: str


@dataclass
class ListItem:
    text: str
    run_id: int  # 連続する箇条書きの塊の通し番号(ページ内)


@dataclass
class Table:
    header: list[str] | None
    rows: list[list[str]]  # 生セル(未解決の ~ / > を含む)


Block = Heading | Paragraph | ListItem | Table

_HEADING = re.compile(r"^(\*{1,3})\s*(.*)$")
_ANCHOR = re.compile(r"\[#([0-9a-zA-Z_-]+)\]\s*$")
_LIST = re.compile(r"^([+-]{1,3})\s*(.*)$")
_TABLE_ROW = re.compile(r"^\|(.*)\|(h|c|f)?$")


def parse_blocks(source: str) -> list[Block]:
    lines = source.replace("\r\n", "\n").replace("\r", "\n").split("\n")
    blocks: list[Block] = []
    para_buf: list[str] = []
    table_buf: list[str] = []
    list_run = 0
    in_list_run = False

    def flush_para() -> None:
        nonlocal para_buf
        if para_buf:
            text = "\n".join(para_buf).strip()
            if text:
                blocks.append(Paragraph(strip_decorations(text)))
            para_buf = []

    def flush_table() -> None:
        nonlocal table_buf
        if table_buf:
            blocks.append(parse_table(table_buf))
            table_buf = []

    for raw_line in lines:
        line = raw_line.rstrip()

        if line.strip() == "":
            flush_para()
            flush_table()
            in_list_run = False
            continue

        if line.startswith("//") or re.fullmatch(r"-{4,}", line.strip()):
            continue

        m = _TABLE_ROW.match(line)
        if m:
            flush_para()
            in_list_run = False
            table_buf.append(line)
            continue
        flush_table()

        m = _HEADING.match(line)
        if m:
            flush_para()
            in_list_run = False
            level = len(m.group(1))
            rest = m.group(2)
            anchor_m = _ANCHOR.search(rest)
            anchor = anchor_m.group(1) if anchor_m else None
            title = _ANCHOR.sub("", rest).strip()
            blocks.append(Heading(level, strip_decorations(title), anchor))
            continue

        m = _LIST.match(line)
        if m:
            flush_para()
            if not in_list_run:
                list_run += 1
                in_list_run = True
            blocks.append(ListItem(strip_decorations(m.group(2)), list_run))
            continue
        in_list_run = False

        if line.startswith("#"):
            flush_para()
            continue

        if line.startswith(">"):
            flush_para()
            continue

        if line.startswith(" ") or line.startswith("　"):
            flush_para()
            continue

        if line.startswith("~"):
            flush_para()
            para_buf.append(line[1:])
            continue

        para_buf.append(line)

    flush_para()
    flush_table()
    return blocks


# --- 表 -------------------------------------------------------------------------------------

def parse_table(lines: list[str]) -> Table:
    header: list[str] | None = None
    rows: list[list[str]] = []
    for line in lines:
        m = _TABLE_ROW.match(line)
        if not m:
            continue
        body, kind = m.group(1), m.group(2)
        cells = [c.strip() for c in body.split("|")]
        if kind == "h":
            # 見出しセルの `~`(見出し扱いの印)は列名に含めない。`>` は右の見出しと結合、
            # 空・記号だけの見出しは 列N にする(列名が `:` や `>` にならないように)
            names = [strip_decorations(c.lstrip("~")) for c in cells]
            for i in range(len(names) - 1, -1, -1):
                if names[i] == ">" and i + 1 < len(names):
                    names[i] = names[i + 1]
            header = [n if has_meaningful_text(n) else f"列{i + 1}" for i, n in enumerate(names)]
        elif kind in ("c", "f"):
            continue
        else:
            rows.append(cells)
    return Table(header, rows)


def resolve_merges(table: Table) -> list[list[str]]:
    """`~`(上のセルと同じ)と `>`(右のセルと同じ)を解決した、装飾抜きのセル値。"""
    resolved: list[list[str]] = []
    prev: list[str] | None = None
    width = len(table.header) if table.header else (len(table.rows[0]) if table.rows else 0)
    for raw in table.rows:
        row = list(raw) + [""] * (width - len(raw))
        for i in range(len(row) - 1, -1, -1):
            if row[i].strip() == ">" and i + 1 < len(row):
                row[i] = row[i + 1]
        for i, cell in enumerate(row):
            if cell.strip() == "~" and prev is not None and i < len(prev):
                row[i] = prev[i]
        # セル内の改行(`&br;`)は 1 行の値に潰す(row_key・「列名: 値」の 1 行表示のため)
        row = [" ".join(strip_decorations(c).split()) for c in row]
        resolved.append(row)
        prev = row
    return resolved


def columns_for(table: Table, width: int) -> list[str]:
    if table.header:
        return table.header
    return [f"列{i + 1}" for i in range(width)]


def detect_repeat_group(columns: list[str]) -> tuple[list[int], int, int] | None:
    """空の区切り列を除いたあと、同名列が group_size ずつ 2 回以上繰り返すか。

    戻り値は (非空列の元 index リスト, group_size, 繰り返し数)。列名は 2 文字以上のときだけ対象。
    """
    kept_idx = [i for i, c in enumerate(columns) if c.strip()]
    kept = [columns[i] for i in kept_idx]
    n = len(kept)
    for group_size in range(2, n // 2 + 1):
        if n % group_size != 0:
            continue
        count = n // group_size
        if count < 2:
            continue
        groups = [kept[i * group_size:(i + 1) * group_size] for i in range(count)]
        if all(g == groups[0] for g in groups) and all(len(c) >= 2 for c in groups[0]):
            return kept_idx, group_size, count
    return None


@dataclass
class UnwoundTable:
    columns: list[str]
    rows: list[list[str]]  # 各行は columns と同じ長さ
    # 「名前」列を足したとき、各行が名前行の直後(=比較対象の基本値の行)かどうか。
    # 段階 3 spec 9〜10。名前列を足さない表では None(全行 True 扱い)。
    first_of_name: list[bool] | None = None


KEY_VALUE_COLUMNS = ["項目", "内容"]


def is_key_value_table(table: Table) -> bool:
    """`|~条件|Lv35以上|` のように先頭セルが `~見出し` の行が半分以上ある表(クエストの条件・報酬など)。

    列名を持たない縦見出しの表で、行 = 列名の組にならない。「項目 | 内容」の行に組み替える。
    """
    rows = [r for r in table.rows if any(c.strip() and c.strip() != ">" for c in r)]
    if not rows:
        return False
    headed = sum(1 for r in rows if r and r[0].strip().startswith("~"))
    return headed * 2 >= len(rows)


def unwind_key_value_table(table: Table) -> UnwoundTable:
    """縦見出しの表を「項目 | 内容」に。`|~|続き|` は上の項目に続く行、`|>||` の空行は捨てる。"""
    out_rows: list[list[str]] = []
    key = ""
    for raw in table.rows:
        cells = [c.strip() for c in raw]
        if not cells:
            continue
        head = cells[0]
        if head.startswith("~"):
            new_key = " ".join(strip_decorations(head[1:]).split())
            if new_key:
                key = new_key
        values = [" ".join(strip_decorations(c).split()) for c in cells[1:] if c.strip() not in ("", ">", "~")]
        value = " ".join(v for v in values if v)
        if not value or not key:
            continue
        if out_rows and out_rows[-1][0] == key:
            out_rows[-1][1] = f"{out_rows[-1][1]} / {value}"
        else:
            out_rows.append([key, value])
    return UnwoundTable(list(KEY_VALUE_COLUMNS), out_rows)


def promote_decorated_header(table: Table) -> Table:
    """列名が `列N`(`|h` 行が無い)表で、`~` 装飾のセルが並ぶ行を列名に昇格する(段階 3 spec 9)。

    `Item/武器/*` `Item/防具/*` は表テンプレートに `|h` を付けていないため、列名がすべて
    `列N` になる。実際の列名は本文側に `|~取得場所|~価&br;格|...|` のような、セルの大半が
    `~`(見出し扱いの印)で始まる行として書かれている。表全体でその行を数え、最も `~` 装飾
    セルが多い行(幅の半分以上)を列名として抜き取り、行からは外す。`table.header` が
    既にある表には触らない。
    """
    if table.header is not None or not table.rows:
        return table
    width = max(len(r) for r in table.rows)
    best_idx, best_count = None, 0
    for i, row in enumerate(table.rows):
        count = sum(1 for c in row if strip_decorations(c).startswith("~"))
        if count > best_count:
            best_idx, best_count = i, count
    if best_idx is None or best_count < max(2, width // 2):
        return table
    # 列名は `~突&br;き` のように 1 文字ずつ改行で折り返して書かれている(狭い列の見出し)。
    # `&br;` は strip_decorations で改行になるので、空白を挟まずに詰めて 1 語に戻す。
    names = ["".join(strip_decorations(c).lstrip("~").split()) for c in table.rows[best_idx]]
    names += [""] * (width - len(names))
    for i in range(len(names) - 1, -1, -1):
        if names[i] == ">" and i + 1 < len(names):
            names[i] = names[i + 1]
    header = [n if has_meaningful_text(n) else f"列{i + 1}" for i, n in enumerate(names)]
    rows = table.rows[:best_idx] + table.rows[best_idx + 1:]
    return Table(header=header, rows=rows)


def _row_dominant_value(row: list[str]) -> str | None:
    """横結合で名前だけ書いた行の名前を取り出す。

    `>`(右のセルと同じ)の連続で、行は「名前」×k セル + 空欄 or 「備考」×m セルのような
    連続した同値の塊(colspan の跡)に分かれる。名前は先頭の塊で、後ろに空欄・備考の塊が
    続くだけ(非空の塊が高々 2 つ)なら名前行とみなし、先頭の塊の値を返す。
    非空の値が散らばって 3 塊以上になる行(実際のステータス行は列ごとに値が違うので
    ほぼ必ずこうなる)は名前行にしない。`~`(装飾行の見出しセルがそのまま残ったもの)の
    塊は無視する(列見出しの直前の「名称 / 備考」キャプション行を誤認しないため)。
    幅の狭い表(進化段階×成功率のような 2〜3 列)は対象にしない。
    """
    width = len(row)
    if width < 6:
        return None
    runs: list[tuple[str, int]] = []
    i = 0
    while i < width:
        v = row[i].strip()
        j = i
        while j < width and row[j].strip() == v:
            j += 1
        runs.append((v, j - i))
        i = j
    meaningful = [(v, n) for v, n in runs if v and not v.startswith("~")]
    if not meaningful or len(meaningful) > 2:
        return None
    first_value, first_len = meaningful[0]
    if first_len < 2:
        return None
    return first_value


def propagate_name_rows(
    columns: list[str], rows: list[list[str]],
) -> tuple[list[str], list[list[str]], list[bool]]:
    """横結合で 1 行に名前だけ書いた行(セルの半分より多くが同じ値)を「見出し行」とみなし、
    その値を後続の行に `名前` 列として付ける(次の名前行まで。段階 3 spec 9)。

    名前行そのものは行から外す。名前行の直後の行(基本値の行)だけ `first_of_name=True`
    にする — 同じ名前のまま続く行(上限行・追加のレベル行など)は比較対象にしない
    (段階 3 spec 10 の突合が上限値を基本値と誤って比べないため)。
    名前行が 1 つも見つからない表は素通しする(columns・rows は変えない)。
    """
    if not rows:
        return columns, rows, []
    out_rows: list[list[str]] = []
    out_first: list[bool] = []
    current_name: str | None = None
    is_first_after_name = False
    saw_name_row = False
    for row in rows:
        name = _row_dominant_value(row)
        if name is not None:
            current_name = name
            is_first_after_name = True
            saw_name_row = True
            continue
        # 名前は先頭列に置く(キー列 = 先頭列。画面はキー列を常に出し、row_key もここから作る)
        out_rows.append([current_name or ""] + row)
        out_first.append(is_first_after_name)
        is_first_after_name = False
    if not saw_name_row:
        return columns, rows, [True] * len(rows)
    return ["名前"] + columns, out_rows, out_first


# 名前行の伝播・`~` 見出しの昇格は装備カタログの表だけに掛ける(段階 3 spec 9)。
# 他のページ(Quest・Event の報酬表など)にも同じ「横結合の見出し行」記法が出るが、
# そこまで対象にすると本来の値(装飾の取りこぼしを含む)が `名前` 列や `text` に混ざり、
# ゼリッピの答えの候補表示を汚す。実測(2026-09-22): Item/武器・Item/防具・Item/アクセサリの
# 65 ページで一様に効く(表構造が揃っている)。他の見つけたページは対象にしない。
_ITEM_CATALOG_PAGE_PREFIXES = ("Item/武器/", "Item/防具/", "Item/アクセサリ/")


def is_item_catalog_page(page: str) -> bool:
    return page.startswith(_ITEM_CATALOG_PAGE_PREFIXES)


def unwind_table(table: Table, normalize_item_catalog: bool = False) -> UnwoundTable:
    if normalize_item_catalog:
        table = promote_decorated_header(table)
    if is_key_value_table(table):
        return unwind_key_value_table(table)
    width = len(table.header) if table.header else (max((len(r) for r in table.rows), default=0))
    columns = columns_for(table, width)
    # 値が 1 つも無い行(`|>||` の区切りや装飾だけの行)は行にしない
    resolved = [r for r in resolve_merges(table) if any(has_meaningful_text(c) for c in r)]
    repeat = detect_repeat_group(columns)
    if repeat is None:
        if normalize_item_catalog:
            columns, resolved, first_of_name = propagate_name_rows(columns, resolved)
            return UnwoundTable(columns, resolved, first_of_name)
        return UnwoundTable(columns, resolved)
    kept_idx, group_size, count = repeat
    kept_columns = [columns[i] for i in kept_idx]
    out_columns = kept_columns[:group_size]
    out_rows: list[list[str]] = []
    for row in resolved:
        kept_row = [row[i] if i < len(row) else "" for i in kept_idx]
        for g in range(count):
            piece = kept_row[g * group_size:(g + 1) * group_size]
            if any(v.strip() for v in piece):
                out_rows.append(piece)
    return UnwoundTable(out_columns, out_rows)


def choose_key_columns(columns: list[str], rows: list[list[str]]) -> list[str]:
    """先頭列で一意なら先頭列だけ、だめなら先頭 2 列。それでも駄目なら空(呼び出し側がハッシュを使う)。"""
    if not columns or not rows:
        return []
    first = [r[0] if r else "" for r in rows]
    if len(set(first)) == len(first):
        return columns[:1]
    if len(columns) >= 2:
        pair = [(r[0] if len(r) > 0 else "", r[1] if len(r) > 1 else "") for r in rows]
        if len(set(pair)) == len(pair):
            return columns[:2]
    return []


def make_row_key(row: list[str], columns: list[str], key_columns: list[str]) -> str:
    if key_columns:
        idxs = [columns.index(c) for c in key_columns]
        parts = [(row[i] if i < len(row) else "")[:20] for i in idxs]
        return "-".join(parts)
    content = "|".join(row)
    return hashlib.sha1(content.encode("utf-8")).hexdigest()[:8]


# --- ユニット組み立て -------------------------------------------------------------------------

@dataclass
class Unit:
    id: str
    kind: str
    page: str
    section: str
    anchor: str
    ord: int
    truncated: int
    text: str
    table_idx: int | None
    group_key: str | None
    row_key: str | None
    cells: dict[str, str] | None
    nums: dict[str, object] | None
    # DB には書かない(unit_rows の列リストに含めない)。段階 3 spec 10 の突合専用:
    # 「名前」列を持つ行のうち、名前行の直後(=基本値の行)かどうか。名前列が無い行は True。
    is_first_of_name: bool = True


@dataclass
class TableInfo:
    page: str
    anchor: str
    table_idx: int
    columns: list[str]
    key_columns: list[str]
    default_columns: list[str]
    row_count: int


@dataclass
class PageUnits:
    units: list[Unit] = field(default_factory=list)
    tables: list[TableInfo] = field(default_factory=list)
    links: list[tuple[str, str, int]] = field(default_factory=list)  # (unit_id, page, ord)


def build_units(page: str, source: str) -> PageUnits:
    blocks = parse_blocks(source)
    out = PageUnits()

    section_stack: list[str] = []
    anchor = "top"
    section = ""
    level_fallback_counts: dict[int, int] = {}

    ord_counter = 0
    n_in_section = 0
    table_idx_in_section = 0
    current_list_run: int | None = None
    seen_anchors: dict[str, int] = {}

    link_ord = 0

    def add_link_source(unit_id: str, text: str) -> None:
        nonlocal link_ord
        for name in extract_links(text):
            out.links.append((unit_id, name, link_ord))
            link_ord += 1

    for block in blocks:
        if isinstance(block, Heading):
            level = block.level
            del section_stack[level - 1:]
            while len(section_stack) < level - 1:
                section_stack.append("")
            section_stack.append(block.text)
            section = " › ".join(t for t in section_stack if t)
            if block.anchor:
                anchor = block.anchor
            else:
                level_fallback_counts[level] = level_fallback_counts.get(level, 0) + 1
                anchor = f"content_{level}_{level_fallback_counts[level]}"
            # wiki 側が同じ明示アンカーを複数回使うことがある(実測)。id・wiki_table の主キーの
            # 衝突を避けるため、ページ内で見た回数を足して一意にする。
            seen_anchors[anchor] = seen_anchors.get(anchor, 0) + 1
            if seen_anchors[anchor] > 1:
                anchor = f"{anchor}_{seen_anchors[anchor]}"
            n_in_section = 0
            table_idx_in_section = 0
            current_list_run = None
            continue

        if isinstance(block, Paragraph):
            if not has_meaningful_text(block.text):
                continue
            n_in_section += 1
            ord_counter += 1
            text, truncated = fragment(block.text)
            unit_id = f"p:{page}/{anchor}/{n_in_section}"
            out.units.append(Unit(
                id=unit_id, kind="paragraph", page=page, section=section, anchor=anchor,
                ord=ord_counter, truncated=1 if truncated else 0, text=text,
                table_idx=None, group_key=None, row_key=None, cells=None, nums=None,
            ))
            add_link_source(unit_id, block.text)
            continue

        if isinstance(block, ListItem):
            if not has_meaningful_text(block.text):
                continue
            n_in_section += 1
            ord_counter += 1
            text, truncated = fragment(block.text)
            unit_id = f"p:{page}/{anchor}/{n_in_section}"
            group_key = f"{page}/{anchor}/l{block.run_id}"
            out.units.append(Unit(
                id=unit_id, kind="paragraph", page=page, section=section, anchor=anchor,
                ord=ord_counter, truncated=1 if truncated else 0, text=text,
                table_idx=None, group_key=group_key, row_key=None, cells=None, nums=None,
            ))
            add_link_source(unit_id, block.text)
            continue

        if isinstance(block, Table):
            unwound = unwind_table(block, normalize_item_catalog=is_item_catalog_page(page))
            if not unwound.columns or not unwound.rows:
                continue  # ほどけない・空の表は段落にも表にもせず捨てる(最小)
            table_idx = table_idx_in_section
            table_idx_in_section += 1
            key_columns = choose_key_columns(unwound.columns, unwound.rows)
            default_columns = unwound.columns[:4]
            group_key = f"{page}/{anchor}/{table_idx}"
            for row_i, row in enumerate(unwound.rows):
                row_key = make_row_key(row, unwound.columns, key_columns)
                cells = {c: (row[i] if i < len(row) else "") for i, c in enumerate(unwound.columns)}
                nums: dict[str, object] = {}
                for c, v in cells.items():
                    n = numify(v)
                    if n is not None:
                        nums[c] = n
                ord_counter += 1
                unit_id = f"r:{page}/{anchor}/{table_idx}/{row_key}"
                is_first = unwound.first_of_name[row_i] if unwound.first_of_name is not None else True
                out.units.append(Unit(
                    id=unit_id, kind="row", page=page, section=section, anchor=anchor,
                    ord=ord_counter, truncated=0,
                    text=" | ".join(f"{c}: {v}" for c, v in cells.items()),
                    table_idx=table_idx, group_key=group_key, row_key=row_key,
                    cells=cells, nums=nums if nums else None, is_first_of_name=is_first,
                ))
            out.tables.append(TableInfo(
                page=page, anchor=anchor, table_idx=table_idx, columns=unwound.columns,
                key_columns=key_columns, default_columns=default_columns,
                row_count=len(unwound.rows),
            ))
            continue

    _dedupe_ids(out.units)
    return out


def _dedupe_ids(units: list[Unit]) -> None:
    """id が衝突したら通し番号を足して一意にする。

    (a) wiki 側の見出しが同じ明示アンカーを複数回使っている、(b) 表の行が内容ごと重複していて
    row_key(先頭列 / 先頭 2 列 / 内容ハッシュ)まで一致する、のどちらでも起こりうる。
    unit.id が主キーなので、最小の保険として起きたときだけ番号を足す。
    """
    seen: dict[str, int] = {}
    for u in units:
        count = seen.get(u.id, 0) + 1
        seen[u.id] = count
        if count > 1:
            u.id = f"{u.id}~{count}"


# --- alias -----------------------------------------------------------------------------------

def page_aliases(name: str, manual: dict[str, str]) -> list[tuple[str, str]]:
    """(別名, ページ名)。(a) ページ名自身 (b) パスの末尾 (c) 手書き別名(呼び出し側でまとめて処理)。"""
    out = [(name, name)]
    if "/" in name:
        tail = name.rsplit("/", 1)[-1]
        if tail and tail != name:
            out.append((tail, name))
    return out


_USEFUL_ALIAS = re.compile(r"[一-鿿゠-ヿa-zA-Z0-9]")


def _is_useful_alias(name: str) -> bool:
    """1 文字や記号だけの別名を弾く(検索の雑音になるだけで、辞書語の最長一致も食う)。"""
    return len(name) >= 2 and bool(_USEFUL_ALIAS.search(name))


MAX_PAGES_PER_TAIL = 3


def build_aliases(page_names: list[str], manual: dict[str, str]) -> dict[str, list[str]]:
    """全ページの別名 → ページ名の一覧。

    パスの末尾は複数ページで衝突してよい(「エクリプスダンジョン」は Dungeon/ と ミニゲーム/ の
    2 ページ)。衝突を落とすと、辞書からも消えて分かち書きが壊れる(「マキシミン」→ マキシ/ミン)。
    ただし「共通」「navi」のように多数のページに付く末尾は別名としての意味が無いので、
    MAX_PAGES_PER_TAIL を超えたら入れない。
    """
    alias_to_pages: dict[str, list[str]] = {}
    tail_pages: dict[str, list[str]] = {}
    for name in page_names:
        alias_to_pages.setdefault(name, []).append(name)
        if "/" in name:
            tail = name.rsplit("/", 1)[-1]
            if tail and tail != name and _is_useful_alias(tail):
                tail_pages.setdefault(tail, []).append(name)
    for tail, pages in tail_pages.items():
        if len(pages) > MAX_PAGES_PER_TAIL:
            continue
        for page in pages:
            lst = alias_to_pages.setdefault(tail, [])
            if page not in lst:
                lst.append(page)
    for alias, target in manual.items():
        if target in page_names:
            lst = alias_to_pages.setdefault(alias, [])
            if target not in lst:
                lst.append(target)
    return alias_to_pages


# --- SQL 書き出し -------------------------------------------------------------------------------

_MAX_STMT_BYTES = 90_000


def sql_str(value: str | None) -> str:
    if value is None:
        return "NULL"
    return "'" + value.replace("'", "''") + "'"


def sql_int(value: int | None) -> str:
    return "NULL" if value is None else str(value)


def sql_json(value: object) -> str:
    return sql_str(json.dumps(value, ensure_ascii=False, separators=(",", ":")))


def batched_insert(table: str, columns: list[str], rows: list[list[str]],
                    verb: str = "INSERT") -> list[str]:
    """1 文が 100 KB を超えないよう、複数行 INSERT に割る。

    `verb="INSERT OR REPLACE"` で差分投入の upsert に使う(主キー一致なら置き換え)。
    """
    if not rows:
        return []
    head = f"{table}({', '.join(columns)})" if columns else table
    prefix = f"{verb} INTO {head} VALUES\n"
    stmts: list[str] = []
    chunk: list[str] = []
    size = len(prefix.encode("utf-8"))
    for row in rows:
        value = "(" + ", ".join(row) + ")"
        value_bytes = len(value.encode("utf-8")) + 2
        if chunk and size + value_bytes > _MAX_STMT_BYTES:
            stmts.append(prefix + ",\n".join(chunk) + ";")
            chunk = []
            size = len(prefix.encode("utf-8"))
        chunk.append(value)
        size += value_bytes
    if chunk:
        stmts.append(prefix + ",\n".join(chunk) + ";")
    return stmts


# --- 訂正(corrections.json) --------------------------------------------------------------

def load_corrections(path: Path) -> list[dict]:
    """`export_corrections` が書き出した confirmed 訂正。無ければ空。"""
    if not path.exists():
        return []
    return json.loads(path.read_text(encoding="utf-8"))


_VARIANT = re.compile(r"【[^】]+】")


def resolve_correction_unit(correction: dict, units: list[Unit]) -> str | None:
    """`page` の row ユニットのうち訂正の対象になる行の id。無ければ None。

    順位: (1) 行の名前(row_key。スキル名の列)に `match` があり、`subject` の変種名(【暴言】)も
    本文にある行 → (2) row_key に `match` がある行 → (3) 本文に `match` と変種名がある行 →
    (4) 本文に `match` がある最初の行。説明文の中で他のスキル名に触れている行(「フェイクの後に…」)に
    付かないよう、名前の列を先に見る。マスタリー変種は基本効果の行とは別の行に書かれているので、
    変種名でも絞る(2026-09-22 実測)。
    """
    match = correction.get("match") or ""
    variants = _VARIANT.findall(correction.get("subject") or "")
    rows = [u for u in units if u.kind == "row" and u.page == correction["page"] and match in u.text]
    named = [u for u in rows if match in (u.row_key or "")]
    for pool in (named, rows):
        if variants:
            for u in pool:
                if all(v in u.text for v in variants):
                    return u.id
        if pool:
            return pool[0].id
    return None


def correction_rows(corrections: list[dict], units: list[Unit]) -> list[list[str]]:
    rows: list[list[str]] = []
    for c in corrections:
        unit_id = resolve_correction_unit(c, units)
        # 同じ行・同じ列に基本とマスタリー変種の訂正が並ぶ(カース・ペンジュラム)ので、subject で区別する
        cid = f"c:{unit_id or c['page']}/{c['col']}/{c['subject']}"
        rows.append([
            sql_str(cid), sql_str(c["subject"]), sql_str(unit_id), sql_str(c["col"]),
            sql_str(c["value"]), sql_str(c.get("grade", "confirmed")), sql_str(c["source_kind"]),
            sql_str(c["source_title"]), sql_str(c.get("source_url")), "NULL",
        ])
    return rows


# --- 静的データだけの項目(app_data.json) -------------------------------------------------

def load_app_data(path: Path) -> list[dict]:
    """`export_app_data` が書き出した静的データだけの項目。無ければ空(段階 3 spec B)。"""
    if not path.exists():
        return []
    return json.loads(path.read_text(encoding="utf-8"))


def app_data_correction_rows(app_data: list[dict]) -> list[list[str]]:
    """`app_data.json` の 1 セルにつき correction 行を 1 本作る。

    wiki の行とは結び付けない(`unit_id` は常に NULL。Worker が完全一致でだけ拾う、段階 3 spec B 7)。
    `source_kind` は `source_title` の文言("クライアント DB 由来" / "wiki 由来")から機械で作る
    (二重管理にしない)。`section` は Worker が疑似候補を組み立てるときに使う(correction 表の
    唯一の section 由来。wiki 訂正の行は `unit_id` から `page`/`section` を引けるので NULL のまま)。
    """
    rows: list[list[str]] = []
    for item in app_data:
        subject = item["subject"]
        section = item["section"]
        source_title = item["source_title"]
        source_kind = "client_db" if "クライアント DB 由来" in source_title else "gamedata"
        for col, value in item["cells"].items():
            cid = f"c:app/{subject}/{col}"
            rows.append([
                sql_str(cid), sql_str(subject), "NULL", sql_str(col), sql_str(str(value)),
                sql_str("confirmed"), sql_str(source_kind), sql_str(source_title), "NULL",
                sql_str(section),
            ])
    return rows


# --- 自動検出の訂正(apparent) -------------------------------------------------------------

# wiki の列名(実表で確認済み)→ app_data.json の列名。同じ日本語名の列は素通し、
# `Item/アクセサリ/レリック/*` だけ「クリ」と綴る(段階 3 spec 10)。
STAT_COLUMNS = ["突き", "斬り", "物防", "魔攻", "魔防", "命中", "回避", "敏捷", "Cri補正"]
_STAT_COLUMN_ALIASES = {"クリ": "Cri補正"}

# 誤検出(表記ゆれ・上限行の混入など)が半分を超えたら、司令塔の判断が要る。
# 実測(2026-09-22): 有効。詳細は implementer の報告を参照。
EMIT_APPARENT = True


def _numify_plain(raw: str) -> int | None:
    """`numify` のうち、範囲・`Lv60以上` を弾いて単一の整数だけ返す(段階 3 spec 10)。"""
    n = numify(raw)
    return n if isinstance(n, int) else None


def apparent_equipment_corrections(app_data: list[dict], units: list[Unit]) -> list[list[str]]:
    """wiki の装備行(名前列が app_data の subject に一致)と 9 値を比べ、数値化できて
    値が違う列だけ `grade='apparent'` の correction を出す(段階 3 spec 10)。

    比較対象は「名前行の直後の行」(`is_first_of_name`)だけ — 同じ名前のまま続く上限行・
    追加のレベル行を基本値と誤って比べない。`-` / `MAX` / `%` / 範囲(`numify` がリストや
    None を返す値)は比べない。app_data 側は `client_db` 由来の装備だけ(wiki 由来はそもそも
    wiki と比べる意味がない)。
    """
    if not EMIT_APPARENT:
        return []
    by_subject: dict[str, dict[str, str]] = {}
    for item in app_data:
        if item.get("kind") != "equipment":
            continue
        if "クライアント DB 由来" not in item.get("source_title", ""):
            continue
        by_subject[item["subject"]] = item["cells"]

    rows: list[list[str]] = []
    for u in units:
        if u.kind != "row" or not u.cells or not u.is_first_of_name:
            continue
        subject = u.cells.get("名前")
        if not subject or subject not in by_subject:
            continue
        app_cells = by_subject[subject]
        for wiki_col, wiki_val in u.cells.items():
            col = _STAT_COLUMN_ALIASES.get(wiki_col, wiki_col)
            if col not in STAT_COLUMNS or col not in app_cells:
                continue
            wiki_num = _numify_plain(wiki_val)
            app_num = _numify_plain(app_cells[col])
            if wiki_num is None or app_num is None or wiki_num == app_num:
                continue
            cid = f"c:apparent/{u.id}/{col}"
            rows.append([
                sql_str(cid), sql_str(subject), sql_str(u.id), sql_str(col),
                sql_str(str(app_num)), sql_str("apparent"), sql_str("client_db"),
                sql_str("アプリのデータ(クライアント DB 由来)"), "NULL", "NULL",
            ])
    return rows


# --- 分かち書き(segment-cli 呼び出し) --------------------------------------------------------

def run_segment_cli(cli_path: Path, dict_path: Path, texts: list[str]) -> list[list[str]]:
    """`node --experimental-strip-types <cli_path> --dict <dict_path>` に全テキストを流す。

    行数は入力と必ず一致する(segment-cli.ts の契約)。改行は空白に潰してから渡す。
    """
    if not texts:
        return []
    payload = "\n".join(t.replace("\n", " ").replace("\r", " ") for t in texts) + "\n"
    proc = subprocess.run(
        ["node", "--experimental-strip-types", str(cli_path), "--dict", str(dict_path)],
        input=payload, capture_output=True, text=True, encoding="utf-8", check=True,
    )
    lines = proc.stdout.split("\n")
    if lines and lines[-1] == "":
        lines = lines[:-1]
    if len(lines) != len(texts):
        raise RuntimeError(f"segment-cli の行数が入力と一致しません: {len(lines)} != {len(texts)}")
    return [line.split(" ") if line else [] for line in lines]


# --- メイン ---------------------------------------------------------------------------------

def unit_search_text(unit: Unit, page_aliases: dict[str, list[str]]) -> str:
    """検索用テキスト。ページ名だけでなく、そのページの別名も並べる。

    「エタ」→「エタの意志」のような短い別名は辞書の最長一致で丸ごと 1 語になり、
    ページ名(長い形)の索引に「エタ」単体の語が残らない。別名を素材として並べておけば、
    短い別名で聞いてもそのページのユニットが引ける(受け入れ条件の「エタ解放…」)。
    """
    names = page_aliases.get(unit.page, [unit.page])
    prefix = " ".join(names)
    if unit.section:
        prefix = f"{prefix} › {unit.section}"
    return f"{prefix} {unit.text}"


# units.sql が先頭で空にして入れ直す表(wiki と同梱データから作るもの)。利用者から届いたもの
# (reaction・ask_log・ask_call)は入れない — 再投入のたびに消えてしまう。
REBUILT_TABLES = ("correction", "unit_link", "alias", "column_note", "wiki_table", "unit", "page", "meta")


# --- 差分投入 ---------------------------------------------------------------------------------
#
# 全件投入(state=None)は REBUILT_TABLES を DELETE してから INSERT し直す(今まで通り)。
# 差分投入(state あり)は本番の現状(d1_state.py が書いた state.json)と比べ、消えた行は DELETE・
# 変わった行は INSERT OR REPLACE・新しい行は INSERT だけを出す。meta だけは毎回書き直す。

def load_state(path: Path) -> dict:
    """本番(または対象の D1)の現状。`d1_state.py` が書いた state.json。"""
    return json.loads(path.read_text(encoding="utf-8"))


def row_hash(values: list[str]) -> str:
    """SQL 値文字列(sql_str/sql_int/sql_json 済み)の列を 1 つのハッシュにする(sha256 先頭 16 hex)。"""
    return hashlib.sha256("\x1f".join(values).encode("utf-8")).hexdigest()[:16]


def diff_keyed(old: dict, new: dict) -> tuple[list, list]:
    """key→h の現状(old)と新しい内容(new)を比べる。戻り値は (消えた key のリスト, upsert する key のリスト)。

    key は文字列でもタプル(複合キー)でもよい。値が変わらない key はどちらにも出ない。
    """
    to_delete = [k for k in old if k not in new]
    to_upsert = [k for k in new if old.get(k) != new[k]]
    return to_delete, to_upsert


def diff_set(old: set, new: set) -> tuple[list, list]:
    """行全体で比べる小さな表(alias・unit_link・column_note)用。戻り値は (消えた行, 増えた行)。"""
    return sorted(old - new), sorted(new - old)


def split_by_bytes(parts: list[str], overhead: int) -> list[list[str]]:
    """文 1 つが `_MAX_STMT_BYTES` を超えないよう、`, ` で繋ぐ部品を割る(D1 の 1 文上限は 100 KB)。
    `overhead` は部品以外(`DELETE FROM … IN (` など)のバイト数。"""
    out: list[list[str]] = []
    chunk: list[str] = []
    size = overhead
    for part in parts:
        part_bytes = len(part.encode("utf-8")) + 2
        if chunk and size + part_bytes > _MAX_STMT_BYTES:
            out.append(chunk)
            chunk = []
            size = overhead
        chunk.append(part)
        size += part_bytes
    if chunk:
        out.append(chunk)
    return out


def delete_in(table: str, column: str, values: list[str]) -> list[str]:
    """`DELETE FROM t WHERE col IN (...)`。values は sql_str/sql_int 済み。"""
    head = f"DELETE FROM {table} WHERE {column} IN ("
    return [f"{head}{', '.join(chunk)});" for chunk in split_by_bytes(values, len(head.encode("utf-8")) + 2)]


def delete_composite_in(table: str, columns: list[str], tuples: list[list[str]]) -> list[str]:
    """複合キーの `DELETE FROM t WHERE (a, b) IN (VALUES (x,y), ...)`。tuples の要素は sql_str/sql_int 済み。"""
    head = f"DELETE FROM {table} WHERE ({', '.join(columns)}) IN (VALUES "
    parts = ["(" + ", ".join(t) + ")" for t in tuples]
    return [f"{head}{', '.join(chunk)});" for chunk in split_by_bytes(parts, len(head.encode("utf-8")) + 2)]


def plan_unit_diff(all_units: list[Unit], unit_h: list[str],
                    old_unit: dict[str, tuple[int, str]],
                    old_fts_rowids: set[int]) -> dict:
    """unit の新規/変化/消滅の分類と rowid の割当て(ネットワーク・SQL に触らない純粋関数)。

    `old_unit` は本番の現状 {id: (rowid, h)}、`old_fts_rowids` は本番の unit_fts に実在する
    rowid の集合(contentless でも rowid だけは読める。d1_state.py が読む)。rowid は既存 id は
    そのまま維持し、新しい id は本番の現状にある最大 rowid + 1 から連番で割り当てる。
    **消えた rowid が永久に封印されるわけではない**(state は残っている行しか持たないので、
    末尾の行が消えれば次の実行では最大値が下がり、同じ番号がまた振られうる。unit と unit_fts の
    両方から同時に消えるので実害は無い)。`unit_h[i]` は `all_units[i]` に対応する h(row_hash 済み)。

    `unit` の INSERT/REPLACE が途中で失敗しても、unit_fts だけ古い/無いままにならないよう、
    「unit にはあるのに unit_fts に無い rowid」も upsert 対象にする(fts だけ入れ直す。h が
    同じでも構わない — 次回また同じ判定になるだけで、実害は無い)。「unit_fts にはあるのに
    unit に無い rowid」(孤児。前回 unit_fts だけ書けて unit が書けなかった、等)は
    `orphan_fts_rowids` として別に返す(呼び出し側は他の何より先に消す)。
    """
    new_id_set = {u.id for u in all_units}
    to_delete_ids = [uid for uid in old_unit if uid not in new_id_set]

    next_rowid = max((rowid for rowid, _ in old_unit.values()), default=0) + 1
    id_to_rowid: dict[str, int] = {}
    for u in all_units:
        if u.id in old_unit:
            id_to_rowid[u.id] = old_unit[u.id][0]
        else:
            id_to_rowid[u.id] = next_rowid
            next_rowid += 1

    old_unit_rowids = {rowid for rowid, _ in old_unit.values()}
    orphan_fts_rowids = sorted(old_fts_rowids - old_unit_rowids)

    upsert_idx: list[int] = []
    fts_delete_rowids: list[int] = []  # 既存 unit の fts を消してから入れ直す分(孤児は含まない)
    for i, u in enumerate(all_units):
        if u.id not in old_unit:
            upsert_idx.append(i)  # 新規: fts は INSERT のみ(消す物が無い)
            continue
        rowid = id_to_rowid[u.id]
        content_changed = old_unit[u.id][1] != unit_h[i]
        fts_missing = rowid not in old_fts_rowids
        if content_changed or fts_missing:
            upsert_idx.append(i)
            if not fts_missing:
                fts_delete_rowids.append(rowid)
    for uid in to_delete_ids:
        rowid = old_unit[uid][0]
        if rowid in old_fts_rowids:
            fts_delete_rowids.append(rowid)

    upsert_set = set(upsert_idx)
    unchanged_idx = [i for i in range(len(all_units)) if i not in upsert_set]

    return {
        "to_delete_ids": to_delete_ids,
        "id_to_rowid": id_to_rowid,
        "upsert_idx": upsert_idx,
        "fts_delete_rowids": fts_delete_rowids,
        "orphan_fts_rowids": orphan_fts_rowids,
        "unchanged_idx": unchanged_idx,
    }


def generate(store: Store, out_dir: Path, segment_cli: Path, aliases_manual: dict[str, str],
            column_notes: dict[str, dict], limit: int | None, only_pages: list[str] | None,
            corrections: list[dict] | None = None, app_data: list[dict] | None = None,
            state: dict | None = None,
            ) -> dict[str, Any]:
    """`out_dir/units.sql` を書く。`state` が None なら全件(DELETE 全件 → INSERT)、
    あれば本番の現状と比べた差分だけを書く(d1_state.py が書いた state.json の中身)。
    """
    all_pages = list(store.db.execute("SELECT * FROM page"))
    page_names_all = [r["name"] for r in all_pages]

    ok_rows = [r for r in all_pages if r["status"] == "ok" and not is_excluded_page(r["name"])]
    if only_pages:
        wanted = set(only_pages)
        # --pages は必ず含め、残りは limit で埋める(開発用の的当てを楽にする)
        head = [r for r in ok_rows if r["name"] in wanted]
        tail = [r for r in ok_rows if r["name"] not in wanted]
        ok_rows = head + tail
    if limit:
        ok_rows = ok_rows[:limit]

    all_units: list[Unit] = []
    all_tables: list[TableInfo] = []
    all_links: list[tuple[str, str, int]] = []
    for row in ok_rows:
        page_units = build_units(row["name"], row["source"] or "")
        all_units.extend(page_units.units)
        all_tables.extend(page_units.tables)
        all_links.extend(page_units.links)

    alias_map = build_aliases(page_names_all, aliases_manual)

    out_dir.mkdir(parents=True, exist_ok=True)
    dict_path = out_dir / "dict.txt"
    # 辞書 = 別名 + 静的データの名前(称号・装備)+ 単独の訂正の対象名。Worker 側(loadAliasIndex)も
    # 同じ 2 系統(alias と correction.subject)から辞書を組むので、索引と質問の切れ目が揃う
    dict_words = set(alias_map.keys())
    dict_words.update(item["subject"] for item in (app_data or []))
    dict_words.update(c["subject"] for c in (corrections or []))
    dict_path.write_text("\n".join(sorted(dict_words)) + "\n", encoding="utf-8")

    page_aliases: dict[str, list[str]] = {}
    for alias_name, target_pages in alias_map.items():
        for target_page in target_pages:
            page_aliases.setdefault(target_page, []).append(alias_name)

    texts = [unit_search_text(u, page_aliases) for u in all_units]
    terms_per_unit = run_segment_cli(segment_cli, dict_path, texts)

    # --- 各表の本体(h を除く値)を先に組み立てる。全件・差分どちらのモードでも同じ ---------------

    # unit: rowid・h を除いた列の値と、その unit の検索語(重複を落として空白区切り)。
    unit_bodies: list[list[str]] = []
    unit_terms_str: list[str] = []
    for u, terms in zip(all_units, terms_per_unit):
        unit_bodies.append([
            sql_str(u.id), sql_str(u.kind), sql_str(u.page), sql_str(u.section),
            sql_str(u.anchor), sql_int(u.ord), sql_int(u.truncated), sql_str(u.text),
            sql_int(u.table_idx), sql_str(u.group_key), sql_str(u.row_key),
            sql_json(u.cells) if u.cells is not None else "NULL",
            sql_json(u.nums) if u.nums is not None else "NULL",
        ])
        unit_terms_str.append(" ".join(sorted(set(terms))))
    unit_h = [row_hash(body + [t]) for body, t in zip(unit_bodies, unit_terms_str)]

    page_bodies: dict[str, list[str]] = {
        r["name"]: [
            sql_str(r["name"]), sql_str(f"{BASE_URL}{quote_page(r['name'])}"),
            sql_str(r["mtime"]), sql_str(r["fetched_at"]), sql_str(r["checked_at"]),
            sql_str(r["status"]), sql_str(r["error"]),
        ]
        for r in all_pages
    }
    page_h = {name: row_hash(body) for name, body in page_bodies.items()}

    table_bodies: dict[tuple, list[str]] = {
        (t.page, t.anchor, t.table_idx): [
            sql_str(t.page), sql_str(t.anchor), sql_int(t.table_idx), "NULL",
            sql_json(t.columns), sql_json(t.key_columns), sql_json(t.default_columns),
            sql_int(t.row_count),
        ]
        for t in all_tables
    }
    table_h = {k: row_hash(body) for k, body in table_bodies.items()}

    note_bodies: list[list[str]] = [
        [sql_str(name), sql_str(note["note"]), sql_str(note.get("state_key"))]
        for name, note in column_notes.items()
    ]
    note_set = {(name, note["note"], note.get("state_key")) for name, note in column_notes.items()}

    alias_set = {(name, page) for name, pages in alias_map.items() for page in pages}
    link_set = {(uid, page, ord_) for uid, page, ord_ in all_links}

    correction_rows_ = correction_rows(corrections or [], all_units)
    app_data_rows_ = app_data_correction_rows(app_data or [])
    apparent_rows_ = apparent_equipment_corrections(app_data or [], all_units)
    all_correction_rows = correction_rows_ + app_data_rows_ + apparent_rows_
    # key は id 列(row[0]、sql_str 済みの '...' そのもの)。生の id に戻さない — state 側も
    # 同じ sql_str(id) で揃えるので、変換の往復を要らなくする。
    correction_bodies: dict[str, list[str]] = {r[0]: r for r in all_correction_rows}
    # 全件投入は同じ id の 2 行で PRIMARY KEY 違反になって止まる。差分でも黙って片方を捨てず、同じく止める
    if len(correction_bodies) != len(all_correction_rows):
        seen: set[str] = set()
        dup = sorted({r[0] for r in all_correction_rows if r[0] in seen or seen.add(r[0])})
        raise ValueError(f"correction の id が重複しています: {dup[:5]}")
    correction_h = {k: row_hash(body) for k, body in correction_bodies.items()}

    from datetime import datetime, timezone
    imported_at = datetime.now(timezone.utc).isoformat(timespec="seconds")
    synced_at = max((r["fetched_at"] for r in ok_rows if r["fetched_at"]), default=imported_at)
    meta_rows = [
        [sql_str("synced_at"), sql_str(synced_at)],
        [sql_str("unit_count"), sql_str(str(len(all_units)))],
        [sql_str("schema_version"), sql_str("1")],
        [sql_str("imported_at"), sql_str(imported_at)],
    ]

    writes: dict[str, int] | None = None
    if state is None:
        lines = _generate_full(
            page_bodies, page_h, unit_bodies, unit_h, unit_terms_str,
            table_bodies, table_h, note_bodies, alias_set, link_set,
            all_correction_rows, meta_rows,
        )
    else:
        lines, writes = _generate_diff(
            state, page_bodies, page_h, all_units, unit_bodies, unit_h, unit_terms_str,
            table_bodies, table_h, note_set, alias_set, link_set,
            correction_bodies, correction_h, meta_rows,
        )

    out_dir.joinpath("units.sql").write_text("\n".join(lines) + "\n", encoding="utf-8")

    return {
        "pages": len(ok_rows),
        "units": len(all_units),
        "aliases": sum(len(v) for v in alias_map.values()),
        "tables": len(all_tables),
        "corrections": len(all_correction_rows),
        "corrections_matched": sum(1 for r in correction_rows_ if r[2] != "NULL"),
        "corrections_confirmed_notice": len(correction_rows_),
        "app_data_corrections": len(app_data_rows_),
        "apparent_corrections": len(apparent_rows_),
        "writes": writes,
    }


def _generate_full(page_bodies, page_h, unit_bodies, unit_h, unit_terms_str,
                    table_bodies, table_h, note_bodies, alias_set, link_set,
                    all_correction_rows, meta_rows) -> list[str]:
    """今まで通りの全件投入(DELETE 全件 → INSERT)。初回の migrations/004 直後や、
    スキーマ・分かち書き辞書を丸ごと作り直すときに使う。"""
    lines: list[str] = [*(f"DELETE FROM {t};" for t in REBUILT_TABLES), "DROP TABLE IF EXISTS unit_fts;"]

    page_rows = [body + [sql_str(page_h[name])] for name, body in page_bodies.items()]
    lines.extend(batched_insert(
        "page", ["name", "url", "mtime", "fetched_at", "checked_at", "status", "error", "h"],
        page_rows,
    ))

    # rowid を明示して unit_fts の rowid と揃える(1 始まりの連番)。
    unit_rows = [
        [str(i)] + body + [sql_str(h)]
        for i, (body, h) in enumerate(zip(unit_bodies, unit_h), start=1)
    ]
    lines.extend(batched_insert(
        "unit",
        ["rowid", "id", "kind", "page", "section", "anchor", "ord", "truncated", "text",
         "table_idx", "group_key", "row_key", "cells", "nums", "h"],
        unit_rows,
    ))

    table_rows = [body + [sql_str(table_h[key])] for key, body in table_bodies.items()]
    lines.extend(batched_insert(
        "wiki_table",
        ["page", "anchor", "table_idx", "caption", "columns", "key_columns", "default_columns",
         "row_count", "h"],
        table_rows,
    ))

    lines.extend(batched_insert("column_note", ["name", "note", "state_key"], note_bodies))

    alias_rows = [[sql_str(name), sql_str(page)] for name, page in sorted(alias_set)]
    lines.extend(batched_insert("alias", ["name", "page"], alias_rows))

    link_rows = [[sql_str(uid), sql_str(page), sql_int(ord_)] for uid, page, ord_ in sorted(link_set)]
    lines.extend(batched_insert("unit_link", ["unit_id", "page", "ord"], link_rows))

    correction_rows_with_h = [
        row + [sql_str(row_hash(row))] for row in all_correction_rows
    ]
    lines.extend(batched_insert(
        "correction",
        ["id", "subject", "unit_id", "col", "value", "grade", "source_kind", "source_title",
         "source_url", "section", "h"],
        correction_rows_with_h,
    ))

    lines.extend(batched_insert("meta", ["key", "value"], meta_rows))

    lines.append(
        "CREATE VIRTUAL TABLE unit_fts USING fts5(terms, content='', tokenize='unicode61', "
        "detail='full', contentless_delete=1);"
    )
    # rowid を unit と揃える(1 始まりの連番)。terms は重複を落として順不同(ソート)で入れる。
    fts_rows = [[str(i), sql_str(t)] for i, t in enumerate(unit_terms_str, start=1)]
    lines.extend(batched_insert("unit_fts(rowid, terms)", [], fts_rows))

    return lines


def _generate_diff(state, page_bodies, page_h, all_units, unit_bodies, unit_h, unit_terms_str,
                    table_bodies, table_h, note_set, alias_set, link_set,
                    correction_bodies, correction_h, meta_rows) -> tuple[list[str], dict[str, int]]:
    """本番の現状(state)と比べた差分だけを書く。REPLACE できる主キー表は INSERT OR REPLACE、
    alias/unit_link/column_note は行全体の集合差で比べる。meta は毎回書き直す。

    unit/unit_fts は書き込み順が壊れても自己修復できるよう、**unit_fts の操作(孤児の削除・
    消す・入れ直す)を先に全部出し、unit(削除・REPLACE)を後に出す**。`--file` の投入が
    unit_fts の後・unit の前で止まっても、次回は「unit にあるのに unit_fts に無い」として
    plan_unit_diff が再検知して入れ直す(plan_unit_diff の docstring 参照)。
    """
    lines: list[str] = []
    writes: dict[str, int] = {}

    # --- unit + unit_fts(rowid の保持・自己修復が要るので専用ロジック plan_unit_diff を使う) ----
    old_unit = {r["id"]: (r["rowid"], r["h"]) for r in state["unit"]}
    old_fts_rowids = {r["rowid"] for r in state["unit_fts_rowids"]}
    plan = plan_unit_diff(all_units, unit_h, old_unit, old_fts_rowids)
    to_delete_ids = plan["to_delete_ids"]
    id_to_rowid = plan["id_to_rowid"]
    upsert_idx = plan["upsert_idx"]
    fts_delete_rowids = plan["fts_delete_rowids"]
    orphan_fts_rowids = plan["orphan_fts_rowids"]

    # 1) unit_fts の DELETE(孤児 → 消える unit の分 → 変わる/補充する既存 unit の分)。
    #    孤児の削除は、rowid が使い回されうる新規 INSERT より必ず先に出す。
    to_delete_fts_rowids = [old_unit[x][0] for x in to_delete_ids if old_unit[x][0] in old_fts_rowids]
    all_fts_delete_rowids = orphan_fts_rowids + to_delete_fts_rowids + fts_delete_rowids
    if all_fts_delete_rowids:
        lines.extend(delete_in("unit_fts", "rowid", [sql_int(r) for r in all_fts_delete_rowids]))

    # 2) unit_fts の INSERT(新規 + 変わった/補充する既存)。unit 側の書き込みより必ず先。
    fts_upsert_rows = [
        [sql_int(id_to_rowid[all_units[i].id]), sql_str(unit_terms_str[i])] for i in upsert_idx
    ]
    lines.extend(batched_insert("unit_fts(rowid, terms)", [], fts_upsert_rows))

    # 3) unit の DELETE → INSERT OR REPLACE(unit_fts が先に揃った後)。
    if to_delete_ids:
        lines.extend(delete_in("unit", "id", [sql_str(x) for x in to_delete_ids]))
    unit_upsert_rows = [
        [sql_int(id_to_rowid[all_units[i].id])] + unit_bodies[i] + [sql_str(unit_h[i])]
        for i in upsert_idx
    ]
    lines.extend(batched_insert(
        "unit",
        ["rowid", "id", "kind", "page", "section", "anchor", "ord", "truncated", "text",
         "table_idx", "group_key", "row_key", "cells", "nums", "h"],
        unit_upsert_rows, verb="INSERT OR REPLACE",
    ))

    writes["unit"] = len(to_delete_ids) + len(upsert_idx)
    writes["unit_fts"] = len(all_fts_delete_rowids) + len(fts_upsert_rows)

    # --- page ---------------------------------------------------------------------------------
    old_page = {r["name"]: r["h"] for r in state["page"]}
    to_delete_page, to_upsert_page = diff_keyed(old_page, page_h)
    if to_delete_page:
        lines.extend(delete_in("page", "name", [sql_str(n) for n in to_delete_page]))
    page_rows = [page_bodies[n] + [sql_str(page_h[n])] for n in to_upsert_page]
    lines.extend(batched_insert(
        "page", ["name", "url", "mtime", "fetched_at", "checked_at", "status", "error", "h"],
        page_rows, verb="INSERT OR REPLACE",
    ))
    writes["page"] = len(to_delete_page) + len(to_upsert_page)

    # --- wiki_table(複合キー) -------------------------------------------------------------------
    old_table = {(r["page"], r["anchor"], r["table_idx"]): r["h"] for r in state["wiki_table"]}
    to_delete_table, to_upsert_table = diff_keyed(old_table, table_h)
    if to_delete_table:
        lines.extend(delete_composite_in(
            "wiki_table", ["page", "anchor", "table_idx"],
            [[sql_str(p), sql_str(a), sql_int(i)] for p, a, i in to_delete_table],
        ))
    table_rows = [table_bodies[k] + [sql_str(table_h[k])] for k in to_upsert_table]
    lines.extend(batched_insert(
        "wiki_table",
        ["page", "anchor", "table_idx", "caption", "columns", "key_columns", "default_columns",
         "row_count", "h"],
        table_rows, verb="INSERT OR REPLACE",
    ))
    writes["wiki_table"] = len(to_delete_table) + len(to_upsert_table)

    # --- correction -----------------------------------------------------------------------------
    # key は correction_bodies と同じく sql_str(id) そのもの(state 側もここで揃える)。
    old_corr = {sql_str(r["id"]): r["h"] for r in state["correction"]}
    to_delete_corr, to_upsert_corr = diff_keyed(old_corr, correction_h)
    if to_delete_corr:
        lines.extend(delete_in("correction", "id", to_delete_corr))
    corr_rows = [correction_bodies[k] + [sql_str(correction_h[k])] for k in to_upsert_corr]
    lines.extend(batched_insert(
        "correction",
        ["id", "subject", "unit_id", "col", "value", "grade", "source_kind", "source_title",
         "source_url", "section", "h"],
        corr_rows, verb="INSERT OR REPLACE",
    ))
    writes["correction"] = len(to_delete_corr) + len(to_upsert_corr)

    # --- alias / unit_link / column_note(小さいので行全体の集合差) --------------------------------
    old_alias = {(r["name"], r["page"]) for r in state["alias"]}
    del_alias, add_alias = diff_set(old_alias, alias_set)
    if del_alias:
        lines.extend(delete_composite_in("alias", ["name", "page"],
                                          [[sql_str(n), sql_str(p)] for n, p in del_alias]))
    lines.extend(batched_insert("alias", ["name", "page"],
                                 [[sql_str(n), sql_str(p)] for n, p in add_alias],
                                 verb="INSERT OR REPLACE"))
    writes["alias"] = len(del_alias) + len(add_alias)

    old_link = {(r["unit_id"], r["page"], r["ord"]) for r in state["unit_link"]}
    del_link, add_link = diff_set(old_link, link_set)
    if del_link:
        lines.extend(delete_composite_in(
            "unit_link", ["unit_id", "page", "ord"],
            [[sql_str(u), sql_str(p), sql_int(o)] for u, p, o in del_link],
        ))
    lines.extend(batched_insert("unit_link", ["unit_id", "page", "ord"],
                                 [[sql_str(u), sql_str(p), sql_int(o)] for u, p, o in add_link]))
    writes["unit_link"] = len(del_link) + len(add_link)

    old_note = {(r["name"], r["note"], r.get("state_key")) for r in state["column_note"]}
    del_note, add_note = diff_set(old_note, note_set)
    if del_note:
        lines.extend(delete_in("column_note", "name", [sql_str(n) for n, _, _ in del_note]))
    lines.extend(batched_insert("column_note", ["name", "note", "state_key"],
                                 [[sql_str(n), sql_str(note), sql_str(sk)] for n, note, sk in add_note],
                                 verb="INSERT OR REPLACE"))
    writes["column_note"] = len(del_note) + len(add_note)

    # --- meta(毎回書き直す。消してから入れると、その間に止まったとき /health が索引なしを返すので置き換える)
    lines.extend(batched_insert("meta", ["key", "value"], meta_rows, verb="INSERT OR REPLACE"))
    writes["meta"] = len(meta_rows)

    return lines, writes


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--cache", type=Path, help="wiki.sqlite の場所(既定は store.default_path())")
    p.add_argument("--out", type=Path, default=HERE / "out", help="出力先ディレクトリ")
    p.add_argument("--segment-cli", type=Path, default=HERE.parent.parent.parent
                   / "services" / "api-worker" / "tools" / "segment-cli.ts",
                   help="segment-cli.ts の場所")
    p.add_argument("--limit", type=int, help="ページ数を絞る(開発用)")
    p.add_argument("--pages", type=str, help="カンマ区切りのページ名。必ず含める(開発用)")
    p.add_argument("--state", type=Path,
                   help="d1_state.py が書いた state.json。指定すると本番の現状と比べた差分だけを書く")
    a = p.parse_args()

    store = Store(a.cache)
    print(f"cache: {store.path}")

    aliases_manual = json.loads((HERE / "aliases.json").read_text(encoding="utf-8"))
    column_notes = json.loads((HERE / "column_notes.json").read_text(encoding="utf-8"))
    corrections = load_corrections(HERE / "corrections.json")
    app_data = load_app_data(HERE / "app_data.json")
    only_pages = [s.strip() for s in a.pages.split(",")] if a.pages else None
    state = load_state(a.state) if a.state else None

    stats = generate(store, a.out, a.segment_cli, aliases_manual, column_notes, a.limit, only_pages,
                     corrections, app_data, state)
    store.close()

    print(f"pages: {stats['pages']}")
    print(f"units: {stats['units']}")
    print(f"tables: {stats['tables']}")
    print(f"aliases: {stats['aliases']}")
    print(
        f"corrections: {stats['corrections']} "
        f"(confirmed: notice={stats['corrections_confirmed_notice']}"
        f"(matched: {stats['corrections_matched']}) + app_data={stats['app_data_corrections']}, "
        f"apparent={stats['apparent_corrections']})"
    )
    print(f"out: {a.out / 'units.sql'}")

    writes = stats["writes"]
    if writes is not None:
        total = sum(writes.values())
        # FTS5 の内部索引(unicode61 のトークン単位の書き込み)はここに入らない。ここでの「行」は
        # unit/unit_fts などの表に対して書く SQL 文の行数(見積り)で、D1 の実際の課金行数の近似。
        print("write rows (見積り。FTS5 の内部索引の書き込みは含まない):", file=sys.stderr)
        for name, n in writes.items():
            print(f"  {name}: {n}", file=sys.stderr)
        print(f"  合計: {total}", file=sys.stderr)
        if total > 80_000:
            print(
                f"警告: 合計 {total} 行は無料枠(1 日 10 万行)に近い/超えます。"
                "--file で流す前に確認してください。",
                file=sys.stderr,
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
