# スキルの対象指定(単体 / 範囲)を wiki から拾って Rust の表にする。
#
# 出典: wiki 各キャラの `Skill/<キャラ名>` ページ「スキル性能一覧」の **対象指定** 列。
# 表の `~` は「上の行と同じ」なので直前の値を引き継ぐ。値は
#   単体 / 範囲/自分中心 / 範囲/位置指定 / 範囲/方向指定 / …
# の形で、先頭が「範囲」なら Area、「単体」なら Single とする。
#
# 使い方(リポジトリルートで):
#   python tools/gamedata/skill_targets.py > crates/gamedata/src/skill_targets.rs
#
# id は crates/gamedata/src/skills.rs の `s("<character>", "<id>", "<名前>", …)` を
# 名前で突き合わせて決める(wiki のアンカーからは機械的に決まらない行があるため)。
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FETCH = ROOT / ".claude" / "skills" / "talewiki-fetch" / "scripts" / "fetch_page.py"
SKILLS_RS = ROOT / "crates" / "gamedata" / "src" / "skills.rs"
CHARACTERS_RS = ROOT / "crates" / "gamedata" / "src" / "characters.rs"


def characters() -> list[tuple[str, str]]:
    src = CHARACTERS_RS.read_text(encoding="utf-8")
    return re.findall(r'GameCharacter \{ id: "(\w+)", name: "(.+?)" \}', src)


def known_skills() -> dict[tuple[str, str], str]:
    """(character_id, 表示名) -> skill id"""
    src = SKILLS_RS.read_text(encoding="utf-8")
    out = {}
    for character, sid, name in re.findall(r's\("(\w+)",\s*"(\w+)",\s*"(.+?)"', src):
        out[(character, name)] = sid
    return out


def fetch(page: str, dest: Path) -> str:
    subprocess.run([sys.executable, str(FETCH), page, str(dest)], check=True, capture_output=True)
    return dest.read_text(encoding="utf-8")


def targets_of(source: str) -> dict[str, str]:
    """表示名 -> 対象指定(`~` は直前の行を引き継ぐ)"""
    out: dict[str, str] = {}
    header: list[str] | None = None
    previous = ""
    for line in source.split("\n"):
        if line.startswith("|スキル|区分|依存|"):
            header = [c.strip() for c in line.rstrip("h").strip("|").split("|")]
            previous = ""
            continue
        if header is None or not line.startswith("|") or "[[" not in line:
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) < len(header):
            continue
        name = re.search(r"\[\[(.+?)>", cells[0])
        if name is None:
            continue
        value = cells[header.index("対象指定")]
        if value == "~":
            value = previous
        previous = value
        out[name.group(1)] = value
    return out


AREA_HINTS = ("範囲", "位置指定", "方向指定", "設置", "連射", "自分中心")


def classify(value: str) -> str | None:
    """対象指定の表記から 単体 / 範囲 を決める。

    wiki の表記はゆれる(`指/位置指定&br;持続10s`、`設置&br;持続60s`、`連射/位置指定` 等)。
    **単体と書いてあるものだけ Single**、位置・方向・設置・連射のように複数に当たる形は Area。
    """
    if "単体" in value:
        return "Single"
    if any(h in value for h in AREA_HINTS):
        return "Area"
    return None


def main() -> None:
    tmp = ROOT / "target" / "wiki-skill-pages"
    tmp.mkdir(parents=True, exist_ok=True)
    skills = known_skills()
    rows: list[tuple[str, str, str]] = []
    missing: list[str] = []
    for character_id, name in characters():
        source = fetch(f"Skill/{name}", tmp / f"{character_id}.txt")
        found = targets_of(source)
        for skill_name, value in found.items():
            sid = skills.get((character_id, skill_name))
            if sid is None:
                continue  # 攻撃表に無いスキル(補助・パッシブ)や取り込み対象外の行
            kind = classify(value)
            if kind is None:
                missing.append(f"{character_id}/{skill_name}: {value}")
                continue
            rows.append((f"{character_id}_{sid}", kind, f"{name} {skill_name}"))

    print("//! スキルの対象指定(単体 / 範囲)。")
    print("//!")
    print("//! 出典: wiki 各キャラの `Skill/<キャラ名>` ページ「スキル性能一覧」の対象指定列。")
    print("//! `tools/gamedata/skill_targets.py` が生成する。手で編集しない。")
    print("")
    print("use domain::SkillTarget;")
    print("")
    print("#[rustfmt::skip]")
    print("pub(crate) const SKILL_TARGETS: &[(&str, SkillTarget)] = &[")
    for sid, kind, note in sorted(rows):
        print(f'    ("{sid}", SkillTarget::{kind}),  // {note}')
    print("];")
    if missing:
        print(f"// 対象指定が読めなかった行: {len(missing)} 件", file=sys.stderr)
        for m in missing[:20]:
            print(f"//   {m}", file=sys.stderr)


if __name__ == "__main__":
    main()
