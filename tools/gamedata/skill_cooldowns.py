# スキルのクールタイム(CT)を wiki から拾って Rust の表にする。
#
# 出典: wiki 各キャラの `Skill/<キャラ名>` ページ。
# 1. 「スキル性能一覧」の行 `|… [[極・スレイ>#Slay]]|…|` から (表示名 -> アンカー) を取る。
#    **アンカーは snake_case の id からは機械的に決まらない**(`極・連撃` -> `Streak` 等)ので
#    必ず一覧から引く。
# 2. `&aname(<Anchor>);` 直後のスキル詳細表の、ヘッダ末尾 `''CT''` 列を読む。
#    CT 列が無い = CT なし。`0` / `-` も CT なし。`10s` / `10分` を秒に直す。
# 3. 詳細表が `?`(未記入。イェフネンの新規スキル)のときだけ、一覧の `CT/消費` 列
#    (`CT10s` 形式)にフォールバックする。生成物のコメントに `[一覧]` と印を付ける。
# 4. id は `crates/gamedata/src/skills.rs` の `s("<character>", "<id>", "<名前>", …)` を
#    (キャラ, 日本語名)で突き合わせて決める。
#
# 入力は wiki に取りに行かず、ローカルミラー(`tools/gamedata/wiki/store.py` の `Store`)を読む。
#
# 使い方(リポジトリルートで):
#   python tools/gamedata/skill_cooldowns.py > crates/gamedata/src/skill_cooldowns.rs
import re
import sys
from pathlib import Path

# 生成物にもエラーにも日本語が出るので、cp932 の端末に落とされないようにする
sys.stdout.reconfigure(encoding="utf-8")
sys.stderr.reconfigure(encoding="utf-8")

sys.path.insert(0, str(Path(__file__).resolve().parent))
from wiki.store import Store  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
SKILLS_RS = ROOT / "crates" / "gamedata" / "src" / "skills.rs"
CHARACTERS_RS = ROOT / "crates" / "gamedata" / "src" / "characters.rs"

# 生成対象から外し、`skills.rs` 側に手書きの例外表として持つ技(理由は skills.rs のコメント)。
EXCLUDED = {
    "mira_crimson_shooter",  # Lv 別に CT が変わる(Lv1 だけ 120s、Lv2 以降 0)
    "roamini_mastary1_2",  # マスタリー選択で 30s / 60s / 90s に変わる
}


def characters() -> list[tuple[str, str]]:
    src = CHARACTERS_RS.read_text(encoding="utf-8")
    return re.findall(r'id: "(\w+)",\s*\n\s*name: "(.+?)",', src)


def known_skills() -> dict[tuple[str, str], str]:
    """(character_id, 表示名) -> skill id"""
    src = SKILLS_RS.read_text(encoding="utf-8")
    out = {}
    for character, sid, name in re.findall(r's\("(\w+)",\s*"(\w+)",\s*"(.+?)"', src):
        out[(character, name)] = sid
    return out


# --- wiki のソースを読む ---

SKIP_PREFIXES = ("//", "#region", "#endregion", "#areaedit", "#br", "#contents")


def is_noise(line: str) -> bool:
    return line.startswith(SKIP_PREFIXES)


def cells_of(line: str) -> list[str]:
    return [c.strip() for c in line.strip("|").split("|")]


def list_table(source: str) -> tuple[dict[str, str], dict[str, str]]:
    """スキル性能一覧から (表示名 -> アンカー) と (表示名 -> CT/消費 列) を取る。

    `~` は上の行を継承するので直前の値を持ち回る。継承の解決を誤ると CT が隣の技に漏れる。
    """
    anchors: dict[str, str] = {}
    listed: dict[str, str] = {}
    header: list[str] | None = None
    previous = ""
    for line in source.split("\n"):
        if is_noise(line):
            continue
        if line.startswith("|スキル|区分|依存|") and line.rstrip().endswith("h"):
            header = cells_of(line.rstrip().rstrip("h"))
            previous = ""
            continue
        if header is None or not line.startswith("|") or "[[" not in line:
            continue
        cells = cells_of(line)
        if len(cells) != len(header) or ">" in cells:
            continue
        name = re.search(r"\[\[(.+?)>#(\w+)\]\]", cells[0])
        if name is None:
            continue
        value = cells[header.index("CT/消費")] if "CT/消費" in header else "-"
        if value == "~":
            value = previous
        previous = value
        anchors.setdefault(name.group(1), name.group(2))
        listed.setdefault(name.group(1), value)
    return anchors, listed


SECONDS = re.compile(r"^(?:CT)?(?:(\d+(?:\.\d+)?)分)?(?:(\d+(?:\.\d+)?)s)?$")


def to_seconds(value: str) -> float | None | str:
    """CT 表記を秒にする。CT なしは `None`、読めない表記は元の文字列を返す。"""
    value = re.sub(r"(COLOR|BGCOLOR)\([^)]*\):", "", value).strip()
    if value in ("", "-", "0", "なし", "?"):
        return None
    m = SECONDS.match(value)
    if m and (m.group(1) or m.group(2)):
        return float(m.group(1) or 0) * 60 + float(m.group(2) or 0)
    return value


NO_COLUMN = object()
NO_ANCHOR = object()


def detail_ct(source: str, anchor: str, note: str = ""):
    """`&aname(<Anchor>);` 直後の詳細表の CT 列(先頭行)。

    CT 列そのものが無い(= CT を持たない技)ときは `NO_COLUMN`、アンカーが見つからない
    (ページの書き方が変わった)ときは `NO_ANCHOR` — 「CT なし」と混ぜない。
    2 行目以降に先頭行と違う CT 値が載っていたら(Lv 別に CT が変わる技)警告する。
    """
    start = source.find(f"&aname({anchor});")
    if start < 0:
        print(f"// アンカーが見つからない: {anchor} {note}", file=sys.stderr)
        return NO_ANCHOR
    rest = source[start + 1 :]
    end = rest.find("&aname(")
    section = rest[:end] if end >= 0 else rest

    index: int | None = None
    first: str | None = None
    for line in section.split("\n"):
        if is_noise(line) or not line.startswith("|"):
            continue
        stripped = line.rstrip()
        if stripped.endswith("|c"):  # 列幅の指定行
            continue
        if stripped.endswith("h"):  # ヘッダ行
            cells = cells_of(stripped.rstrip("h"))
            if "''CT''" in cells:
                index = cells.index("''CT''")
                width = len(cells)
            continue
        if index is None:
            continue
        cells = cells_of(stripped)
        if len(cells) != width or ">" in cells:
            continue
        if first is None:
            first = cells[index]
            continue
        # 2 行目以降(Lv 別の行)。`~` は上の行の継承。秒に直して**値が違うときだけ**
        # 知らせる(大文字小文字の揺れ `5S` や未記入 `?` は同じ扱い)
        value = cells[index].strip()
        if value and value != "~":
            head, tail = to_seconds(first.lower()), to_seconds(value.lower())
            if head != tail and not (head is None or tail is None):
                print(
                    f"// CT が行ごとに違う: {anchor} {note} 先頭行 {first.strip()!r} / "
                    f"別の行 {value!r}(先頭行を採用)",
                    file=sys.stderr,
                )
                break
    if first is None:
        return NO_COLUMN
    return first


def main() -> None:
    store = Store()
    skills = known_skills()
    rows: list[tuple[str, float, str, bool]] = []
    unreadable: list[str] = []
    for character_id, name in characters():
        page = store.db.execute(
            "SELECT source FROM page WHERE name = ?", (f"Skill/{name}",)
        ).fetchone()
        if page is None or page["source"] is None:
            print(f"// ミラーに無いページ: Skill/{name}", file=sys.stderr)
            continue
        source = page["source"]
        anchors, listed = list_table(source)
        for skill_name, anchor in anchors.items():
            sid = skills.get((character_id, skill_name))
            if sid is None:
                continue  # 攻撃表に無い行(補助・パッシブ)や取り込み対象外
            full_id = f"{character_id}_{sid}"
            if full_id in EXCLUDED:
                continue
            raw = detail_ct(source, anchor, f"{name} {skill_name}")
            fallback = False
            if raw is NO_COLUMN or raw is NO_ANCHOR:
                continue  # 詳細表に CT 列が無い = CT なし(アンカー無しは stderr に警告済み)
            if raw.strip().strip("?") == "":
                # 詳細表が未記入(イェフネンの新規スキル)。一覧の `CT/消費` 列だけが頼り。
                hit = re.search(r"CT(\d+(?:\.\d+)?)(分|s)", listed.get(skill_name, ""))
                if hit is None:
                    continue
                raw = hit.group(0)
                fallback = True
            seconds = to_seconds(raw)
            if isinstance(seconds, str):
                unreadable.append(f"{full_id} ({name} {skill_name}): {seconds}")
                continue
            if seconds is None:
                continue
            rows.append((full_id, seconds, f"{name} {skill_name}", fallback))

    print("//! スキルのクールタイム(CT、秒)。")
    print("//!")
    print("//! 出典: wiki 各キャラの `Skill/<キャラ名>` ページ。スキル詳細表(`&aname(…);` の直後)の")
    print("//! ヘッダ末尾 `CT` 列。CT 列が無い / `0` の技は CT なしなのでここに載せない。")
    print("//! `[一覧]` 印の行は詳細表が未記入(`?`)なのでスキル性能一覧の `CT/消費` 列から採った。")
    print("//! `tools/gamedata/skill_cooldowns.py` が生成する。手で編集しない。")
    print("//!")
    print("//! クライアント DB と食い違う技のうち DPS に効く 2 件は韓国公式スキル情報で裏取り済み(2026-09-21)。")
    print("//! いずれも wiki が正: `nocturne_magnetic_force` 재사용 대기 시간 1 분(ActionInfo/16_Nocturne/3004075.htm)、")
    print("//! `nocturne_satellite_canon` 30 초(同 3004076.htm、21.07.12 V820 で CT の誤記を修正済みとある)。")
    print("")
    print("#[rustfmt::skip]")
    print("pub(crate) const SKILL_COOLDOWNS: &[(&str, f64)] = &[")
    for sid, seconds, note, fallback in sorted(rows):
        mark = "[一覧] " if fallback else ""
        print(f'    ("{sid}", {seconds:.1f}),  // {mark}{note}')
    print("];")
    if unreadable:
        print(f"// CT が読めなかった行: {len(unreadable)} 件", file=sys.stderr)
        for m in unreadable:
            print(f"//   {m}", file=sys.stderr)


if __name__ == "__main__":
    main()
